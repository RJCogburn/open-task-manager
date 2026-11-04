//! Watching for slowdowns that come and go: the Slowdown check looks once,
//! over a second, and misses a stutter every few seconds or a program that
//! spikes and settles. This samples every second for a while (30 seconds
//! from the page), then says what spiked, when, and who was busy then.
//!
//! Stalls come from the kernel's running totals in /proc/pressure (time in
//! microseconds some program spent waiting), so each second's share is
//! exact rather than a moving average.

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::ThreadKind;
use tauri::State;

use super::slowdown::{Check, Hog, check, end_action, endable, swap_pages};
use crate::AppState;

static STOP: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);

/// One second of watching.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    /// Share of the second some program waited, in percent.
    cpu_wait: Option<f64>,
    memory_wait: Option<f64>,
    io_wait: Option<f64>,
    /// How busy the processor was, in percent of all of it.
    cpu: f32,
    /// Pages swapped in and out.
    swapped: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchReport {
    /// Unix time in milliseconds when watching began.
    started: u64,
    points: Vec<Point>,
    checks: Vec<Check>,
}

/// A program's readings over the watch, one per second (zero while it wasn't running).
#[derive(Default, Clone)]
struct Track {
    name: String,
    /// Percent of one core.
    cpu: Vec<f32>,
    /// Bytes read and written per second.
    disk: Vec<f64>,
    first_memory: u64,
    last_memory: u64,
}

/// The kernel's running total of microseconds some program waited.
fn stall_total(kind: &str) -> Option<u64> {
    let text = fs::read_to_string(format!("/proc/pressure/{kind}")).ok()?;
    text.lines()
        .find(|l| l.starts_with("some"))?
        .split_whitespace()
        .find_map(|f| f.strip_prefix("total="))?
        .parse()
        .ok()
}

/// Watches for `seconds` (5 to 120), then reports. Stops early when asked.
#[tauri::command]
pub async fn slowdown_watch(state: State<'_, AppState>, seconds: u32) -> Result<WatchReport, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Already watching.".into());
    }
    STOP.store(false, Ordering::SeqCst);
    let shared = state.shared.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let seconds = seconds.clamp(5, 120) as usize;
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let kinds = ["cpu", "memory", "io"];
        let mut totals: Vec<Option<u64>> = kinds.iter().map(|k| stall_total(k)).collect();
        let mut swap = swap_pages();
        let mut at = Instant::now();
        shared.lock().unwrap().refresh_processes();
        let mut points = Vec::new();
        let mut tracks: HashMap<u32, Track> = HashMap::new();
        let mut cores = 1;
        for i in 0..seconds {
            thread::sleep(Duration::from_secs(1));
            if STOP.load(Ordering::SeqCst) {
                break;
            }
            let micros = at.elapsed().as_micros().max(1) as f64;
            at = Instant::now();
            let now: Vec<Option<u64>> = kinds.iter().map(|k| stall_total(k)).collect();
            let share = |j: usize| match (totals[j], now[j]) {
                (Some(a), Some(b)) => Some((b.saturating_sub(a) as f64 / micros * 100.0).min(100.0)),
                _ => None,
            };
            let swap_now = swap_pages();
            let mut s = shared.lock().unwrap();
            s.refresh_processes();
            s.sys.refresh_cpu_usage();
            cores = s.sys.cpus().len().max(1);
            let elapsed = s.elapsed;
            points.push(Point {
                cpu_wait: share(0),
                memory_wait: share(1),
                io_wait: share(2),
                cpu: s.sys.global_cpu_usage(),
                swapped: swap_now.saturating_sub(swap),
            });
            for (pid, p) in s.sys.processes() {
                if p.thread_kind() == Some(ThreadKind::Userland) {
                    continue;
                }
                let t = tracks.entry(pid.as_u32()).or_insert_with(|| Track {
                    name: p.name().to_string_lossy().into_owned(),
                    cpu: vec![0.0; i],
                    disk: vec![0.0; i],
                    first_memory: p.memory(),
                    ..Default::default()
                });
                let du = p.disk_usage();
                t.cpu.resize(i, 0.0);
                t.disk.resize(i, 0.0);
                t.cpu.push(p.cpu_usage());
                t.disk.push((du.read_bytes + du.written_bytes) as f64 / elapsed);
                t.last_memory = p.memory();
            }
            drop(s);
            totals = now;
            swap = swap_now;
        }
        let n = points.len();
        for t in tracks.values_mut() {
            t.cpu.resize(n, 0.0);
            t.disk.resize(n, 0.0);
        }
        if n < 3 {
            return Err("Stopped.".to_string());
        }
        let checks = analyse(&points, &tracks, cores);
        Ok(WatchReport { started, points, checks })
    })
    .await
    .map_err(|e| e.to_string());
    RUNNING.store(false, Ordering::SeqCst);
    result?
}

#[tauri::command]
pub fn slowdown_watch_stop() {
    STOP.store(true, Ordering::SeqCst);
}

/// "0:07": seconds into the watch.
fn mark(i: usize) -> String {
    format!("{}:{:02}", (i + 1) / 60, (i + 1) % 60)
}

fn rate(bytes: f64) -> String {
    if bytes >= 1e9 {
        format!("{:.1} GB/s", bytes / 1e9)
    } else if bytes >= 1e6 {
        format!("{:.0} MB/s", bytes / 1e6)
    } else {
        format!("{:.0} KB/s", bytes / 1e3)
    }
}

fn hog(pid: u32, t: &Track, cpu: f32, disk: f64) -> Hog {
    Hog { pid, name: t.name.clone(), cpu, memory: t.last_memory, disk: disk as u64 }
}

/// Who was busiest across some seconds, by a reading of each second, most first.
fn busiest<'a, F: Fn(&Track, usize) -> f64>(
    tracks: &'a HashMap<u32, Track>,
    seconds: &[usize],
    reading: F,
) -> Vec<(u32, &'a Track, f64)> {
    let mut v: Vec<(u32, &Track, f64)> = tracks
        .iter()
        .map(|(pid, t)| (*pid, t, seconds.iter().map(|&i| reading(t, i)).sum::<f64>() / seconds.len().max(1) as f64))
        .filter(|x| x.2 > 0.0)
        .collect();
    v.sort_by(|a, b| b.2.total_cmp(&a.2));
    v
}

fn analyse(points: &[Point], tracks: &HashMap<u32, Track>, cores: usize) -> Vec<Check> {
    let n = points.len();
    let mut checks = Vec::new();
    let worst = |f: &dyn Fn(&Point) -> Option<f64>| {
        points.iter().enumerate().filter_map(|(i, p)| f(p).map(|v| (i, v))).max_by(|a, b| a.1.total_cmp(&b.1))
    };

    // Waiting on the disks.
    let io: Vec<usize> = (0..n).filter(|&i| points[i].io_wait.unwrap_or(0.0) >= 10.0).collect();
    if !io.is_empty() {
        let (at, peak) = worst(&|p| p.io_wait).unwrap_or((0, 0.0));
        let who = busiest(tracks, &io, |t, i| t.disk[i]);
        let names: Vec<String> = who.iter().take(3).map(|(_, t, b)| format!("{} ({})", t.name, rate(*b))).collect();
        let status = if peak >= 30.0 || io.len() >= 5 { "bad" } else { "warn" };
        let mut c = check(
            "watch-io",
            status,
            format!("Programs waited on the disks in {} of {n} seconds", io.len()),
            format!(
                "Worst at {}: {peak:.0}% of that second.{}",
                mark(at),
                if names.is_empty() {
                    String::new()
                } else {
                    format!(" Busiest with the disks then: {}.", names.join(", "))
                }
            ),
        )
        .page("disks")
        .more([
            "While programs wait on a disk, whatever they do pauses: windows stop responding, games hitch, \
             files open slowly. Brief waits now and then are normal; repeated ones mean something is keeping \
             the disk busy."
                .to_string(),
            "A program reading or writing a lot at those moments is the likely cause: an update, a backup, a \
             file indexer, a browser's cache, or a game loading. If it's one you didn't expect, ending it stops \
             the waiting right away."
                .to_string(),
        ]);
        for (pid, t, b) in who.iter().take(2) {
            let h = hog(*pid, t, 0.0, *b);
            if *b >= 1e6 && endable(&h) {
                c = c.action(end_action(&h));
            }
        }
        checks.push(c);
    }

    // Waiting for the processor.
    let cpu: Vec<usize> = (0..n).filter(|&i| points[i].cpu_wait.unwrap_or(0.0) >= 20.0).collect();
    if !cpu.is_empty() {
        let (at, peak) = worst(&|p| p.cpu_wait).unwrap_or((0, 0.0));
        let who = busiest(tracks, &cpu, |t, i| t.cpu[i] as f64);
        let names: Vec<String> =
            who.iter().take(3).map(|(_, t, c)| format!("{} ({c:.0}% of a core)", t.name)).collect();
        let status = if peak >= 50.0 || cpu.len() >= 5 { "bad" } else { "warn" };
        let mut c = check(
            "watch-cpu",
            status,
            format!("Programs waited for the processor in {} of {n} seconds", cpu.len()),
            format!(
                "Worst at {}: {peak:.0}% of that second.{}",
                mark(at),
                if names.is_empty() { String::new() } else { format!(" Busiest then: {}.", names.join(", ")) }
            ),
        )
        .page("processes")
        .more([format!(
            "Your processor has {cores} threads. When programs want more than that, they take turns, and \
             everything slows down a little. Short bursts are normal; repeated ones while you're working feel \
             like stutter."
        )]);
        for (pid, t, cp) in who.iter().take(2) {
            let h = hog(*pid, t, *cp as f32, 0.0);
            if *cp >= 50.0 && endable(&h) {
                c = c.action(end_action(&h));
            }
        }
        checks.push(c);
    }

    // Waiting for memory.
    let mem: Vec<usize> = (0..n).filter(|&i| points[i].memory_wait.unwrap_or(0.0) >= 5.0).collect();
    if !mem.is_empty() {
        let (at, peak) = worst(&|p| p.memory_wait).unwrap_or((0, 0.0));
        let mut grew: Vec<(&Track, i64)> =
            tracks.values().map(|t| (t, t.last_memory as i64 - t.first_memory as i64)).filter(|x| x.1 > 0).collect();
        grew.sort_by_key(|x| std::cmp::Reverse(x.1));
        let names: Vec<String> =
            grew.iter().take(3).map(|(t, g)| format!("{} (+{:.0} MB)", t.name, *g as f64 / 1e6)).collect();
        checks.push(
            check(
                "watch-memory",
                if peak >= 20.0 || mem.len() >= 5 { "bad" } else { "warn" },
                format!("Programs waited for memory in {} of {n} seconds", mem.len()),
                format!(
                    "Worst at {}: {peak:.0}% of that second.{}",
                    mark(at),
                    if names.is_empty() { String::new() } else { format!(" Grew the most: {}.", names.join(", ")) }
                ),
            )
            .page("memory")
            .more([
                "Waiting for memory means Linux had to free some first, by dropping cached files or swapping, \
                 before a program could carry on. It shows as pauses across the whole desktop."
                    .to_string(),
            ]),
        );
    }

    // Swapping.
    let swapped: Vec<usize> = (0..n).filter(|&i| points[i].swapped >= 256).collect();
    if !swapped.is_empty() {
        let pages: u64 = points.iter().map(|p| p.swapped).sum();
        checks.push(
            check(
                "watch-swap",
                if swapped.len() >= 5 { "bad" } else { "warn" },
                format!("Swapping in {} of {n} seconds", swapped.len()),
                format!("{:.0} MB moved between memory and swap while watching.", pages as f64 * 4096.0 / 1e6),
            )
            .page("memory")
            .more(["Swapping now and then is fine. Swapping in many seconds means memory is short, and \
                 programs pause while their memory is brought back."
                .to_string()]),
        );
    }

    // Programs that came and went: a spike well above their usual.
    let mut spiky: Vec<(u32, &Track, usize, f32)> = tracks
        .iter()
        .filter_map(|(pid, t)| {
            let spikes = t.cpu.iter().filter(|&&c| c >= 80.0).count();
            let peak = t.cpu.iter().cloned().fold(0.0f32, f32::max);
            let usual = t.cpu.iter().sum::<f32>() / n as f32;
            (spikes > 0 && spikes <= n / 2 && usual < peak / 3.0).then_some((*pid, t, spikes, peak))
        })
        .collect();
    spiky.sort_by(|a, b| b.3.total_cmp(&a.3));
    if !spiky.is_empty() {
        let any_wait = !cpu.is_empty();
        let names: Vec<String> = spiky
            .iter()
            .take(4)
            .map(|(_, t, k, p)| {
                format!("{} (up to {p:.0}% of a core, {k} {})", t.name, if *k == 1 { "time" } else { "times" })
            })
            .collect();
        let mut c = check(
            "watch-spikes",
            if any_wait { "warn" } else { "ok" },
            if spiky.len() == 1 {
                format!("{} jumped up and settled again", spiky[0].1.name)
            } else {
                format!("{} programs jumped up and settled again", spiky.len())
            },
            format!(
                "{}.{}",
                names.join(", "),
                if any_wait { " Other programs had to wait at times." } else { " Nothing had to wait for them." }
            ),
        )
        .page("processes")
        .more(["These were mostly quiet but briefly busy. That's normal for a browser loading a page or a \
             program checking for updates; a program doing it every few seconds for no reason may be stuck \
             in a loop."
            .to_string()]);
        if any_wait {
            for (pid, t, _, p) in spiky.iter().take(2) {
                let h = hog(*pid, t, *p, 0.0);
                if endable(&h) {
                    c = c.action(end_action(&h));
                }
            }
        }
        checks.push(c);
    }

    if !checks.iter().any(|c| c.status == "bad" || c.status == "warn") {
        checks.insert(
            0,
            check(
                "watch-clear",
                "ok",
                "Nothing came and went",
                format!(
                    "In {n} seconds no program waited noticeably for the processor, memory or the disks, and nothing swapped."
                ),
            ),
        );
    }
    let rank = |s: &str| ["bad", "warn", "ok", "skipped"].iter().position(|x| *x == s).unwrap_or(4);
    checks.sort_by_key(|c| rank(c.status));
    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(cpu_wait: f64, io_wait: f64) -> Point {
        Point { cpu_wait: Some(cpu_wait), memory_wait: Some(0.0), io_wait: Some(io_wait), cpu: 20.0, swapped: 0 }
    }

    #[test]
    fn quiet_watch_is_clear() {
        let points = vec![point(1.0, 0.0); 30];
        let checks = analyse(&points, &HashMap::new(), 8);
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].id, "watch-clear");
    }

    #[test]
    fn finds_disk_stalls_and_who() {
        let mut points = vec![point(1.0, 0.0); 30];
        points[11].io_wait = Some(42.0);
        points[20].io_wait = Some(12.0);
        let mut disk = vec![0.0; 30];
        disk[11] = 80e6;
        disk[20] = 20e6;
        let mut tracks = HashMap::new();
        tracks.insert(7, Track { name: "indexer".into(), cpu: vec![1.0; 30], disk, first_memory: 0, last_memory: 0 });
        let checks = analyse(&points, &tracks, 8);
        let io = checks.iter().find(|c| c.id == "watch-io").unwrap();
        assert_eq!(io.status, "bad");
        assert_eq!(io.title, "Programs waited on the disks in 2 of 30 seconds");
        assert!(io.detail.starts_with("Worst at 0:12: 42% of that second."));
        assert!(io.detail.contains("indexer (50 MB/s)"));
    }

    #[test]
    fn finds_spikes() {
        let points = vec![point(1.0, 0.0); 30];
        let mut cpu = vec![2.0; 30];
        cpu[5] = 98.0;
        cpu[15] = 95.0;
        let mut tracks = HashMap::new();
        tracks.insert(9, Track { name: "updater".into(), cpu, disk: vec![0.0; 30], first_memory: 0, last_memory: 0 });
        let checks = analyse(&points, &tracks, 8);
        let s = checks.iter().find(|c| c.id == "watch-spikes").unwrap();
        assert_eq!(s.status, "ok");
        assert!(s.detail.contains("updater (up to 98% of a core, 2 times)"));
        assert!(checks.iter().any(|c| c.id == "watch-clear"));
    }
}
