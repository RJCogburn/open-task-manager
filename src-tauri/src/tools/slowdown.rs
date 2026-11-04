//! The slowdown check: looks at what most often makes a Linux system feel
//! slow and says so in plain words, with what to do about it. Everything is
//! read without administrator rights.
//!
//! The kernel's pressure stall information (/proc/pressure) is the core of
//! it: it says how much of the time programs were waiting for the CPU, for
//! memory or for the disks, which usage percentages can't. The rest are the
//! usual suspects: swapping, low memory, a program hogging the CPU or memory,
//! full or failing drives, heat, power saving, and failed or slow services.
//!
//! Some findings come with fixes. Those are a fixed list, run by
//! `slowdown_fix`, which checks its target again before acting; the ones that
//! need administrator rights ask for the password through polkit.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use sysinfo::ThreadKind;
use tauri::State;

use crate::AppState;
use crate::logging::xdg;
use crate::programs;
use crate::sampler::{self, Shared};

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    id: &'static str,
    title: String,
    /// "bad", "warn", "ok", or "skipped" when it couldn't be checked.
    status: &'static str,
    detail: String,
    /// The page with more, by tool id.
    page: Option<&'static str>,
    /// Paragraphs on what was found, why it matters and what to do.
    more: Vec<String>,
    actions: Vec<Action>,
}

/// A fix the user can run from the check. `fix` and `target` go back to
/// `slowdown_fix` unchanged.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    fix: &'static str,
    target: Option<String>,
    label: String,
    /// What it does, shown before asking.
    explain: String,
    /// The command it runs, shown to the user.
    command: String,
    /// Asks for the administrator password.
    admin: bool,
}

fn check(id: &'static str, status: &'static str, title: impl Into<String>, detail: impl Into<String>) -> Check {
    Check { id, title: title.into(), status, detail: detail.into(), page: None, more: Vec::new(), actions: Vec::new() }
}

impl Check {
    fn page(mut self, page: &'static str) -> Self {
        self.page = Some(page);
        self
    }

    fn more(mut self, paragraphs: impl IntoIterator<Item = String>) -> Self {
        self.more.extend(paragraphs);
        self
    }

    fn action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    checks: Vec<Check>,
    /// Share of the last minute some programs waited, in percent, if the kernel reports it.
    pressure: Option<Pressure>,
}

#[derive(Serialize, Clone, Copy, Default, Debug, PartialEq)]
pub struct Pressure {
    cpu: f64,
    memory: f64,
    io: f64,
}

/// One /proc/pressure file: the 60-second averages of "some" (at least one
/// program waited) and "full" (every program waited), in percent.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Stall {
    some: f64,
    full: f64,
}

fn parse_pressure(text: &str) -> Option<Stall> {
    let avg60 = |kind: &str| {
        text.lines()
            .find(|l| l.starts_with(kind))?
            .split_whitespace()
            .find_map(|f| f.strip_prefix("avg60="))?
            .parse::<f64>()
            .ok()
    };
    Some(Stall { some: avg60("some")?, full: avg60("full").unwrap_or(0.0) })
}

fn read_pressure(kind: &str) -> Option<Stall> {
    parse_pressure(&fs::read_to_string(format!("/proc/pressure/{kind}")).ok()?)
}

/// Pages swapped in and out since boot.
fn swap_pages() -> u64 {
    let text = fs::read_to_string("/proc/vmstat").unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let (key, value) = l.split_once(' ')?;
            matches!(key, "pswpin" | "pswpout").then(|| value.trim().parse::<u64>().ok())?
        })
        .sum()
}

struct Hog {
    pid: u32,
    name: String,
    /// Percent of one core over the last second.
    cpu: f32,
    memory: u64,
    /// Bytes read and written over the last second.
    disk: u64,
}

/// What the checks look at, gathered once.
struct Facts {
    /// Busiest by memory first.
    hogs: Vec<Hog>,
    mem_total: u64,
    mem_available: u64,
    cores: usize,
    cpu_total: f32,
    swapped: u64,
}

#[tauri::command]
pub async fn slowdown_check(state: State<'_, AppState>) -> Result<Report, String> {
    let shared = state.shared.clone();
    #[cfg(feature = "disks")]
    let health = state.health.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut checks = Vec::new();
        let facts = gather(&shared);

        let pressure = pressure_checks(&mut checks, &facts);
        checks.push(swapping(&facts));
        checks.push(memory_low(&facts));
        checks.extend(cpu_hogs(&facts, pressure.map(|p| p.cpu)));
        checks.extend(memory_hog(&facts));
        checks.extend(disk_space());
        #[cfg(feature = "disks")]
        checks.push(drive_health(&crate::tools::disks::health_summary(&health)));
        #[cfg(feature = "disks")]
        checks.extend(firmware_bugs());
        checks.push(cpu_heat());
        checks.extend(gpu_throttle());
        checks.push(power_saving());
        checks.extend(failed_services());
        checks.extend(slow_startup());

        let rank = |s: &str| ["bad", "warn", "ok", "skipped"].iter().position(|x| *x == s).unwrap_or(4);
        checks.sort_by_key(|c| rank(c.status));
        Report { checks, pressure }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Swapping and per-process use are measured over one second.
fn gather(shared: &Arc<Mutex<Shared>>) -> Facts {
    let swap_before = swap_pages();
    shared.lock().unwrap().refresh_processes();
    thread::sleep(Duration::from_secs(1));
    let swapped = swap_pages().saturating_sub(swap_before);
    let mut s = shared.lock().unwrap();
    s.refresh_processes();
    s.sys.refresh_memory();
    let mut hogs: Vec<Hog> = s
        .sys
        .processes()
        .iter()
        .filter(|(_, p)| p.thread_kind() != Some(ThreadKind::Userland))
        .map(|(pid, p)| Hog {
            pid: pid.as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            cpu: p.cpu_usage(),
            memory: p.memory(),
            disk: p.disk_usage().read_bytes + p.disk_usage().written_bytes,
        })
        .collect();
    hogs.sort_by_key(|h| std::cmp::Reverse(h.memory));
    Facts {
        hogs,
        mem_total: s.sys.total_memory(),
        mem_available: s.sys.available_memory(),
        cores: s.sys.cpus().len().max(1),
        cpu_total: s.sys.global_cpu_usage(),
        swapped,
    }
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

/// "Ending" the app itself, or the desktop, would do more harm than good.
fn endable(h: &Hog) -> bool {
    h.pid != std::process::id()
        && !matches!(
            h.name.as_str(),
            "systemd"
                | "kwin_wayland"
                | "kwin_x11"
                | "plasmashell"
                | "gnome-shell"
                | "Xorg"
                | "Xwayland"
                | "sddm"
                | "gdm"
        )
}

fn end_action(h: &Hog) -> Action {
    Action {
        fix: "end-process",
        target: Some(format!("{}:{}", h.pid, h.name)),
        label: format!("End {}", h.name),
        explain: format!(
            "Asks {} (process {}) to close, the way closing its window would. Anything unsaved in it may be lost.",
            h.name, h.pid
        ),
        command: format!("kill -TERM {}", h.pid),
        admin: false,
    }
}

/// The first few, as "name (amount)".
fn top<F: Fn(&Hog) -> String>(hogs: &[&Hog], n: usize, amount: F) -> String {
    hogs.iter().take(n).map(|h| format!("{} ({})", h.name, amount(h))).collect::<Vec<_>>().join(", ")
}

fn pressure_checks(checks: &mut Vec<Check>, facts: &Facts) -> Option<Pressure> {
    let (Some(cpu), Some(memory), Some(io)) = (read_pressure("cpu"), read_pressure("memory"), read_pressure("io"))
    else {
        checks.push(check(
            "pressure",
            "skipped",
            "Waiting time isn't reported",
            "Your kernel doesn't report pressure stall information, so the check can't tell how long programs wait. \
             It may be turned off with psi=0.",
        ));
        return None;
    };
    checks.push(memory_pressure(memory, facts));
    checks.push(io_pressure(io, facts));
    checks.push(cpu_pressure(cpu, facts));
    Some(Pressure { cpu: cpu.some, memory: memory.some, io: io.some })
}

const PRESSURE_EXPLAINED: &str = "This comes from the kernel's pressure stall information: the share of time \
    programs were ready to work but had to wait. Unlike a usage percentage, it measures the slowdown you feel.";

fn memory_pressure(p: Stall, facts: &Facts) -> Check {
    let detail = format!(
        "Over the last minute, programs were held up waiting for memory {:.0}% of the time, and everything at once {:.0}%.",
        p.some, p.full
    );
    let c = if p.full >= 5.0 {
        check("pressure-memory", "bad", "Programs are stalled waiting for memory", detail)
    } else if p.some >= 10.0 {
        check("pressure-memory", "warn", "Programs are waiting for memory", detail)
    } else {
        return check("pressure-memory", "ok", "Memory isn't holding anything up", detail).page("memory");
    };
    let big: Vec<&Hog> = facts.hogs.iter().take(5).collect();
    let mut c = c.page("memory").more([
        PRESSURE_EXPLAINED.to_string(),
        "When memory runs short, the kernel keeps dropping and re-reading the files programs use, and moves memory to \
         swap. Everything slows down while it does."
            .to_string(),
        format!("The programs using the most memory: {}.", top(&big, 5, |h| gb(h.memory))),
        "Close what you don't need, or add swap or zram. Restarting a program that has grown large over time, like a \
         browser, often frees a lot."
            .to_string(),
    ]);
    if let Some(h) = facts.hogs.iter().find(|h| endable(h)) {
        c = c.action(end_action(h));
    }
    c
}

fn io_pressure(p: Stall, facts: &Facts) -> Check {
    let detail = format!("Over the last minute, programs waited for the disks {:.0}% of the time.", p.some);
    let c = if p.full >= 10.0 {
        check("pressure-io", "bad", "Programs are stalled waiting for the disks", detail)
    } else if p.some >= 20.0 {
        check("pressure-io", "warn", "Programs are waiting for the disks", detail)
    } else {
        return check("pressure-io", "ok", "The disks are keeping up", detail).page("disks");
    };
    let mut busy: Vec<&Hog> = facts.hogs.iter().filter(|h| h.disk > 1_000_000).collect();
    busy.sort_by_key(|h| std::cmp::Reverse(h.disk));
    let who = if busy.is_empty() {
        "No single program stood out in the last second; it may be the kernel writing out cached files, or a scan by \
         an indexer or backup."
            .to_string()
    } else {
        format!("Reading or writing the most right now: {}.", top(&busy, 3, |h| format!("{}/s", gb(h.disk))))
    };
    c.page("disks").more([
        PRESSURE_EXPLAINED.to_string(),
        who,
        "A slow or nearly full drive, swapping, or a backup, indexer or update running in the background are the usual \
         causes. A hard drive is far slower than an SSD when many programs use it at once."
            .to_string(),
    ])
}

fn cpu_pressure(p: Stall, facts: &Facts) -> Check {
    let detail = format!("Over the last minute, programs waited for a free processor core {:.0}% of the time.", p.some);
    let c = if p.some >= 60.0 {
        check("pressure-cpu", "bad", "The processor can't keep up", detail)
    } else if p.some >= 25.0 {
        check("pressure-cpu", "warn", "Programs are queuing for the processor", detail)
    } else {
        return check("pressure-cpu", "ok", "The processor is keeping up", detail).page("processes");
    };
    let mut busy: Vec<&Hog> = facts.hogs.iter().collect();
    busy.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    let mut c = c.page("processes").more([
        PRESSURE_EXPLAINED.to_string(),
        format!("The busiest programs: {}.", top(&busy, 5, |h| format!("{:.0}% of a core", h.cpu))),
        "Ending the busiest one helps right away. Lowering its priority on the Processes page lets it keep working \
         without getting in the way."
            .to_string(),
    ]);
    if let Some(h) = busy.iter().find(|h| endable(h) && h.cpu >= 50.0) {
        c = c.action(end_action(h));
    }
    c
}

fn swapping(facts: &Facts) -> Check {
    let pages = facts.swapped;
    let mb = pages * 4096 / 1_000_000;
    let c = if pages >= 1000 {
        check(
            "swapping",
            "bad",
            "Your system is swapping heavily",
            format!("About {mb} MB moved between memory and swap in one second."),
        )
    } else if pages >= 100 {
        check(
            "swapping",
            "warn",
            "Your system is swapping",
            format!("About {mb} MB moved between memory and swap in one second."),
        )
    } else {
        return check("swapping", "ok", "No swapping right now", "Nothing was moved between memory and swap.")
            .page("memory");
    };
    let big: Vec<&Hog> = facts.hogs.iter().take(5).collect();
    c.page("memory").more([
        "Swap is space on a drive used as extra memory. It's far slower than memory, so when programs need what was \
         moved there, they wait."
            .to_string(),
        format!("The programs using the most memory: {}.", top(&big, 5, |h| gb(h.memory))),
        "Closing a large program stops it. If it happens often, more memory helps most; zram, compressed swap in memory, \
         helps too, and many distributions turn it on by default."
            .to_string(),
    ])
}

fn memory_low(facts: &Facts) -> Check {
    let (total, available) = (facts.mem_total, facts.mem_available);
    let pct = if total > 0 { available as f64 / total as f64 * 100.0 } else { 100.0 };
    let mut detail = format!("{} of {} is free ({pct:.0}%).", gb(available), gb(total));
    if let Some(h) = facts.hogs.first() {
        detail += &format!(" The biggest program is {}, using {}.", h.name, gb(h.memory));
    }
    let c = if pct < 5.0 {
        check("memory-low", "bad", "Memory is nearly full", detail)
    } else if pct < 10.0 {
        check("memory-low", "warn", "Memory is running low", detail)
    } else {
        return check("memory-low", "ok", "There is enough free memory", detail).page("memory");
    };
    let big: Vec<&Hog> = facts.hogs.iter().take(5).collect();
    let mut c = c.page("memory").more([
        format!("The programs using the most memory: {}.", top(&big, 5, |h| gb(h.memory))),
        "When it runs out, the system starts swapping, and in the end the kernel ends a program by itself to free \
         memory. Closing something now avoids both."
            .to_string(),
    ]);
    if let Some(h) = facts.hogs.iter().find(|h| endable(h)) {
        c = c.action(end_action(h));
    }
    c
}

/// Busy programs only slow things down when the processor is short of time:
/// mostly busy, or with programs waiting for it. Otherwise they're just
/// doing their work on cores that would be idle.
fn cpu_hogs(facts: &Facts, waiting: Option<f64>) -> Option<Check> {
    let (cores, total) = (facts.cores, facts.cpu_total);
    let mut busy: Vec<&Hog> = facts.hogs.iter().filter(|h| h.cpu >= 90.0).collect();
    busy.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    if busy.is_empty() {
        return None;
    }
    let names = top(&busy, 3, |h| format!("{:.0}% of a core", h.cpu));
    if total < 75.0 && waiting.unwrap_or(0.0) < 10.0 {
        return Some(
            check(
                "cpu-hog",
                "ok",
                "The processor has time to spare",
                format!("It is {total:.0}% busy overall across {cores} threads. Busiest: {names}."),
            )
            .page("processes"),
        );
    }
    let status = if total >= 90.0 { "bad" } else { "warn" };
    let title = if busy.len() == 1 {
        format!("{} is keeping the processor busy", busy[0].name)
    } else {
        format!("{} programs are keeping the processor busy", busy.len())
    };
    let detail = format!("{names}. Your processor has {cores} threads and is {total:.0}% busy overall.");
    let mut c = check("cpu-hog", status, title, detail).page("processes").more([
        "A program using 100% of a core is using one thread completely. That's fine for a game or a build that you \
         started; it's a problem when it's something you didn't expect, or the whole processor is busy."
            .to_string(),
        "Ending it frees the processor right away. To keep it running but out of the way, lower its priority on the \
         Processes page instead."
            .to_string(),
    ]);
    for h in busy.iter().filter(|h| endable(h)).take(3) {
        c = c.action(end_action(h));
    }
    Some(c)
}

fn memory_hog(facts: &Facts) -> Option<Check> {
    let h = facts.hogs.first()?;
    let pct = h.memory as f64 / facts.mem_total.max(1) as f64 * 100.0;
    if pct < 30.0 {
        return None;
    }
    let mut c = check(
        "memory-hog",
        "warn",
        format!("{} is using {pct:.0}% of your memory", h.name),
        format!("{}. If it doesn't need that much, restarting it may help.", gb(h.memory)),
    )
    .page("processes")
    .more([
        format!(
            "{} is process {}. Some programs, like databases, virtual machines or AI models, need a lot of memory by \
             design. Others grow over time and give it back when restarted.",
            h.name, h.pid
        ),
        "If you didn't expect it to be this large, save your work in it and restart it.".to_string(),
    ]);
    if endable(h) {
        c = c.action(end_action(h));
    }
    Some(c)
}

fn trash_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("Trash")
}

/// Bytes in the user's Trash, roughly: file sizes, not blocks used.
fn trash_size() -> u64 {
    fn walk(dir: &std::path::Path) -> u64 {
        fs::read_dir(dir)
            .map(|d| {
                d.flatten()
                    .map(|e| match e.metadata() {
                        Ok(m) if m.is_dir() => walk(&e.path()),
                        Ok(m) => m.len(),
                        Err(_) => 0,
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
    walk(&trash_dir().join("files"))
}

fn disk_space() -> Vec<Check> {
    let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else { return Vec::new() };
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut fine = 0;
    for d in disks.iter() {
        let fs_name = d.file_system().to_string_lossy();
        // fuseblk is a real drive (NTFS); other FUSE mounts, like a running
        // AppImage's, are not.
        if d.total_space() < 1_000_000_000
            || d.is_removable()
            || d.is_read_only()
            || fs_name.starts_with("fuse.")
            || matches!(fs_name.as_ref(), "squashfs" | "tmpfs" | "overlay" | "iso9660" | "vfat")
            || !seen.insert(d.name().to_os_string())
        {
            continue;
        }
        let free = d.available_space();
        let pct = free as f64 / d.total_space() as f64 * 100.0;
        let mount = d.mount_point().to_string_lossy().into_owned();
        let detail = format!("{} free of {} ({pct:.0}%).", gb(free), gb(d.total_space()));
        let c = if pct < 5.0 || free < 2_000_000_000 {
            check("disk-full", "bad", format!("{mount} is nearly full"), detail)
        } else if pct < 10.0 {
            check("disk-full", "warn", format!("{mount} is getting full"), detail)
        } else {
            fine += 1;
            continue;
        };
        let mut c = c.page("space").more([
            "A nearly full drive slows down writing, especially an SSD, and programs can fail or lose work when it \
             fills up completely."
                .to_string(),
            "The Disk Space page shows what is using it, folder by folder. Common places space goes: the Trash, \
             package caches, old logs, downloads, and virtual machine or container images."
                .to_string(),
            "To clear package caches and old logs, run in a terminal: `sudo paccache -rk1` (Arch, CachyOS), \
             `sudo apt clean` (Debian, Ubuntu) or `sudo dnf clean all` (Fedora), and \
             `sudo journalctl --vacuum-size=200M`."
                .to_string(),
        ]);
        // The Trash lives in the home folder: offer to empty it on that drive.
        let trash = trash_size();
        if home.starts_with(d.mount_point()) && trash > 50_000_000 && programs::which("gio").is_some() {
            c = c.action(Action {
                fix: "empty-trash",
                target: None,
                label: format!("Empty the Trash ({})", gb(trash)),
                explain: "Deletes everything in your Trash for good.".into(),
                command: "gio trash --empty".into(),
                admin: false,
            });
        }
        out.push(c);
    }
    if out.is_empty() && fine > 0 {
        out.push(
            check("disk-full", "ok", "Your drives have room", format!("{fine} checked, all with at least 10% free."))
                .page("space"),
        );
    }
    out
}

#[cfg(feature = "disks")]
fn drive_health(drives: &[(String, &'static str, String)]) -> Check {
    // Each with its own note, which says what is wrong.
    let with_note = |status: &str| -> Vec<String> {
        drives
            .iter()
            .filter(|d| d.1 == status)
            .map(|d| if d.2.is_empty() { d.0.clone() } else { format!("{}: {}", d.0, d.2.trim_end_matches('.')) })
            .collect()
    };
    let failing = with_note("failing");
    let warning = with_note("warning");
    let known = drives.iter().filter(|d| d.1 != "unknown").count();
    let advice = [
        "Drives keep a record of their own health, called SMART. Bad or pending sectors are spots the drive couldn't \
         read reliably; it moves data away from them when it can. A few can stay stable for years, but a growing \
         number means the drive is wearing out, and reading around them makes it slow."
            .to_string(),
        "No setting fixes this. Back up anything on it you'd miss, and don't keep the only copy of anything there. \
         The Disks page shows its full SMART report; check again in a few weeks to see whether the number grows."
            .to_string(),
    ];
    let c = if !failing.is_empty() {
        check("drive-health", "bad", "A drive is failing", failing.join("; ") + ". Back up what's on it now.")
            .more(advice)
    } else if !warning.is_empty() {
        check("drive-health", "warn", "A drive shows signs of wear", warning.join("; ") + ".").more(advice)
    } else if known == 0 {
        let note = drives.first().map(|d| d.2.clone()).unwrap_or_default();
        check("drive-health", "skipped", "Drive health wasn't read", note)
    } else {
        check("drive-health", "ok", "Your drives are healthy", format!("{known} checked."))
    };
    c.page("disks")
}

/// Drives running firmware with a known serious bug.
#[cfg(feature = "disks")]
fn firmware_bugs() -> Option<Check> {
    let found = crate::tools::firmware::known_issues();
    let (drive, _) = found.first()?;
    Some(
        check("firmware", "bad", "A drive's firmware has a known bug", format!("{drive}."))
            .page("disks")
            .more(found.iter().map(|(d, i)| format!("{d}: {}", i.text())))
            .more(["Firmware is the software inside the drive. The Disks page shows each drive's version and                     where its maker publishes updates."
                .to_string()]),
    )
}

const HEAT_ADVICE: &str = "Dust in the fans and heatsink is the most common cause, then blocked airflow (a laptop on a \
    bed or sofa) and, on older machines, dried-out thermal paste. The Power & Thermals page shows the temperatures and \
    fan speeds.";

fn cpu_heat() -> Check {
    let Some(t) = sampler::find_cpu_temp_input().as_ref().and_then(sampler::read_millidegrees) else {
        return check(
            "cpu-heat",
            "skipped",
            "Processor temperature isn't available",
            "No temperature sensor for the processor was found.",
        );
    };
    let detail = format!("The processor is at {t:.0}°C.");
    let c = if t >= 95.0 {
        check("cpu-heat", "bad", "The processor is too hot", detail + " It slows itself down at this temperature.")
    } else if t >= 88.0 {
        check(
            "cpu-heat",
            "warn",
            "The processor is running hot",
            detail + " Near this point it starts slowing itself down.",
        )
    } else {
        return check("cpu-heat", "ok", "The processor isn't overheating", detail).page("sensors");
    };
    c.page("sensors").more([
        "To protect itself, a processor lowers its speed when it gets too hot. That's called thermal throttling, and \
         it can make it much slower."
            .to_string(),
        HEAT_ADVICE.to_string(),
    ])
}

/// NVIDIA cards report why their clocks are held back. Only heat is a
/// problem: a power cap under load is normal.
fn gpu_throttle() -> Option<Check> {
    programs::which("nvidia-smi")?;
    let query = |field: &str| {
        let out = programs::run(
            "nvidia-smi",
            &[&format!("--query-gpu=name,temperature.gpu,{field}"), "--format=csv,noheader,nounits"],
        )
        .ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    // Newer drivers call them event reasons.
    let text = query("clocks_event_reasons.active").or_else(|| query("clocks_throttle_reasons.active"))?;
    let mut hot = Vec::new();
    let mut names = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        if f.len() < 3 {
            continue;
        }
        names.push(f[0].to_string());
        let reasons = u64::from_str_radix(f[2].trim_start_matches("0x"), 16).unwrap_or(0);
        // Software and hardware thermal slowdown.
        if reasons & (0x20 | 0x40) != 0 {
            hot.push(format!("{} at {}°C", f[0], f[1]));
        }
    }
    Some(if hot.is_empty() {
        check("gpu-heat", "ok", "The graphics card isn't held back by heat", names.join(", ")).page("gpu")
    } else {
        check("gpu-heat", "bad", "The graphics card is slowing itself down to cool off", hot.join(", ") + ".")
            .page("gpu")
            .more([
                "The card's driver reports that it lowered its clocks because of heat. Games and other graphics work \
                 get slower while it does."
                    .to_string(),
                HEAT_ADVICE.to_string(),
            ])
    })
}

fn power_saving() -> Check {
    let profile = programs::which("powerprofilesctl")
        .and_then(|_| programs::run("powerprofilesctl", &["get"]).ok())
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|p| !p.is_empty());
    let cpufreq = |file: &str| {
        fs::read_to_string(format!("/sys/devices/system/cpu/cpu0/cpufreq/{file}"))
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty())
    };
    let epp = cpufreq("energy_performance_preference");
    if profile.as_deref() == Some("power-saver") || epp.as_deref() == Some("power") {
        let how = match profile.as_deref() {
            Some("power-saver") => "The power profile is set to Power Saver.",
            _ => "The processor is set to prefer saving power.",
        };
        let mut c = check("power", "warn", "Power saving is holding the processor back", how).more([
            "Power saving keeps the processor at lower speeds to save battery and stay cool. That's right on battery, \
             but slows things down when plugged in."
                .to_string(),
            "You can change it in your desktop's power settings, usually from the battery icon.".to_string(),
        ]);
        if profile.is_some() {
            c = c.action(Action {
                fix: "power-balanced",
                target: None,
                label: "Switch to Balanced".into(),
                explain: "Sets the power profile to Balanced, the usual setting when plugged in.".into(),
                command: "powerprofilesctl set balanced".into(),
                admin: false,
            });
        }
        c
    } else {
        let now = match (profile, epp, cpufreq("scaling_governor")) {
            (Some(p), _, _) => format!("The power profile is {p}."),
            (None, Some(e), _) => format!("The processor's energy preference is {e}."),
            (None, None, Some(g)) => format!("The processor's governor is {g}."),
            _ => "No power setting could be read.".to_string(),
        };
        check("power", "ok", "Power saving isn't holding things back", now)
    }
}

/// A systemd service name safe to pass on to systemctl.
fn valid_unit(name: &str) -> bool {
    name.ends_with(".service")
        && name.len() < 200
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"@._:-\\".contains(&b))
}

fn failed_units() -> Vec<String> {
    programs::run("systemctl", &["--failed", "--no-legend", "--plain"])
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.split_whitespace().next().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn failed_services() -> Option<Check> {
    programs::which("systemctl")?;
    let names = failed_units();
    if names.is_empty() {
        return Some(check(
            "services",
            "ok",
            "No services have failed",
            "systemd reports every service as running or stopped normally.",
        ));
    }
    let mut c = check(
        "services",
        "warn",
        format!("{} service{} failed", names.len(), if names.len() == 1 { "" } else { "s" }),
        format!("{}.", names.join(", ")),
    )
    .more(["A failed service stopped with an error. Some keep retrying in the background; others just mean a \
            feature isn't working. What each one last said:"
        .to_string()]);
    for name in names.iter().take(5) {
        // Its last few log lines say why: they follow the status block's blank line.
        let log = programs::run("systemctl", &["status", name, "--no-pager", "--lines=4", "--output=cat"])
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .skip_while(|l| !l.trim().is_empty())
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let log =
            if log.trim().is_empty() { "no log lines could be read.".to_string() } else { log.trim().to_string() };
        c = c.more([format!("{name}: {log}")]);
        if valid_unit(name) {
            c = c.action(Action {
                fix: "restart-service",
                target: Some(name.clone()),
                label: format!("Restart {name}"),
                explain: format!("Starts {name} again. If what made it fail hasn't changed, it may fail again."),
                command: format!("systemctl restart {name}"),
                admin: true,
            });
        }
    }
    Some(c)
}

/// Services that make start-up wait for the network, and are safe to turn
/// off on a desktop.
const WAIT_ONLINE: &[&str] = &["NetworkManager-wait-online.service", "systemd-networkd-wait-online.service"];

/// The slowest services at the last start-up, from systemd-analyze. Ones a
/// timer starts run in the background later, and devices are the kernel
/// finding hardware, so neither counts.
fn slow_startup() -> Option<Check> {
    let out = programs::run("systemd-analyze", &["blame", "--no-pager"]).ok()?;
    let timers = timer_units();
    let text = String::from_utf8_lossy(&out.stdout);
    let slow: Vec<(f64, String)> = text
        .lines()
        .filter_map(|l| {
            let (time, unit) = l.trim().rsplit_once(' ')?;
            Some((parse_duration(time)?, unit.to_string()))
        })
        .filter(|(secs, unit)| *secs >= 10.0 && unit.ends_with(".service") && !timers.contains(unit))
        .take(3)
        .collect();
    let boot =
        programs::run("systemd-analyze", &["time"]).ok().and_then(|o| parse_boot(&String::from_utf8_lossy(&o.stdout)));
    let took = boot
        .map(|b| {
            format!("Start-up took {}, {} of it after the firmware and boot loader. ", secs(b.total), secs(b.linux))
        })
        .unwrap_or_default();
    // A slow firmware is worth knowing about even when the services are fine.
    let firmware_tip = boot.filter(|b| b.firmware >= 15.0).map(|b| {
        format!(
            "Your computer's firmware takes {} before Linux even starts. Turning on Fast Boot in its setup (BIOS or \
             UEFI; usually F2 or Delete at power-on) can shorten that a lot. Memory training after a BIOS update or \
             with new memory also makes it slow for a while.",
            secs(b.firmware)
        )
    });
    if slow.is_empty() {
        return Some(
            check("startup", "ok", "Nothing slows down start-up", took + "No service took 10 seconds or more.")
                .more(firmware_tip),
        );
    }
    let list: Vec<String> = slow.iter().map(|(s, u)| format!("{u} ({s:.0} s)")).collect();
    let mut c = check("startup", "warn", "Some services slow down start-up", took + &list.join(", ") + ".").more([
        "These services kept start-up waiting at the last boot. Times can overlap, since services start side by side."
            .to_string(),
    ]);
    for (_, unit) in &slow {
        if WAIT_ONLINE.contains(&unit.as_str()) {
            c = c
                .more([format!(
                    "{unit} makes start-up wait until the network is fully connected. Desktops and laptops rarely need \
                     that; it matters only if something must reach the network during start-up, like network drives in \
                     /etc/fstab or a server. Turning it off doesn't turn off the network."
                )])
                .action(Action {
                    fix: "disable-wait-online",
                    target: Some(unit.clone()),
                    label: "Don't wait for the network at start-up".into(),
                    explain: format!(
                        "Turns off {unit}. To undo it, run: sudo systemctl enable {unit}. Keep it if you mount network \
                         drives at start-up."
                    ),
                    command: format!("systemctl disable {unit}"),
                    admin: true,
                });
        }
    }
    Some(c.more(firmware_tip))
}

/// Services that timers start.
fn timer_units() -> HashSet<String> {
    programs::run("systemctl", &["list-timers", "--all", "--no-legend", "--plain"])
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .filter(|w| w.ends_with(".service"))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug)]
struct Boot {
    total: f64,
    firmware: f64,
    /// After the firmware and boot loader.
    linux: f64,
}

/// From "Startup finished in 29s (firmware) + 5.7s (loader) + … = 1min 6.0s".
fn parse_boot(text: &str) -> Option<Boot> {
    let line = text.lines().find(|l| l.starts_with("Startup finished in"))?;
    let (parts, total) = line.trim_start_matches("Startup finished in").rsplit_once('=')?;
    let total = parse_duration(total.trim())?;
    let part = |name: &str| -> f64 {
        parts
            .split('+')
            .filter_map(|p| {
                let (time, what) = p.trim().split_once(" (")?;
                (what.trim_end_matches(')') == name).then(|| parse_duration(time))?
            })
            .sum()
    };
    let (firmware, loader) = (part("firmware"), part("loader"));
    Some(Boot { total, firmware, linux: total - firmware - loader })
}

fn secs(s: f64) -> String {
    if s >= 60.0 { format!("{} min {:.0} s", (s / 60.0) as u64, s % 60.0) } else { format!("{s:.0} s") }
}

/// systemd's durations: "1min 2.345s", "12.3s", "456ms".
fn parse_duration(text: &str) -> Option<f64> {
    let mut total = 0.0;
    for part in text.split_whitespace() {
        let (num, unit) = part.split_at(part.find(|c: char| c.is_ascii_alphabetic())?);
        let n: f64 = num.parse().ok()?;
        total += n * match unit {
            "ms" => 0.001,
            "s" => 1.0,
            "min" => 60.0,
            "h" => 3600.0,
            "us" | "µs" => 0.000_001,
            _ => return None,
        };
    }
    Some(total)
}

/// Runs one of the fixes the check offered. Each checks its target again, so
/// only what the check found is acted on, whatever is passed in.
#[tauri::command]
pub async fn slowdown_fix(fix: String, target: Option<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || match fix.as_str() {
        "end-process" => {
            let (pid, name) = target.as_deref().and_then(|t| t.split_once(':')).ok_or("No program was named.")?;
            let pid: u32 = pid.parse().map_err(|_| "No program was named.")?;
            // The process id may have been reused since the check. The
            // kernel's name for it is cut to 15 characters.
            let now = fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
            let now = now.trim();
            if now.is_empty() || !name.starts_with(now) || pid == std::process::id() {
                return Err(format!("{name} is no longer running."));
            }
            if unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) } == 0 {
                Ok(format!("Asked {name} to close."))
            } else {
                Err(format!(
                    "{name} couldn't be ended: {}. If it belongs to another user, end it from the Processes page.",
                    std::io::Error::last_os_error()
                ))
            }
        }
        "empty-trash" => {
            let out = programs::run("gio", &["trash", "--empty"])?;
            if out.status.success() { Ok("The Trash is empty.".into()) } else { Err(stderr(&out)) }
        }
        "power-balanced" => {
            let out = programs::run("powerprofilesctl", &["set", "balanced"])?;
            if out.status.success() { Ok("The power profile is now Balanced.".into()) } else { Err(stderr(&out)) }
        }
        "restart-service" => {
            let unit = target.ok_or("No service was named.")?;
            if !valid_unit(&unit) || !failed_units().contains(&unit) {
                return Err(format!("{unit} isn't a failed service any more."));
            }
            admin(&["systemctl", "restart", &unit]).map(|_| format!("{unit} was started again."))
        }
        "disable-wait-online" => {
            let unit = target.ok_or("No service was named.")?;
            if !WAIT_ONLINE.contains(&unit.as_str()) {
                return Err(format!("{unit} isn't one this can turn off."));
            }
            admin(&["systemctl", "disable", &unit])
                .map(|_| format!("{unit} is off. Start-up won't wait for the network from the next boot."))
        }
        _ => Err(format!("Unknown fix {fix}.")),
    })
    .await
    .map_err(|e| e.to_string())?
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).trim().to_string()
}

/// Runs a command as administrator through polkit, which asks for the password.
fn admin(args: &[&str]) -> Result<(), String> {
    let out = programs::run("pkexec", args)?;
    if out.status.success() {
        Ok(())
    } else if out.status.code() == Some(126) {
        Err("Cancelled.".into())
    } else {
        Err(stderr(&out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(hogs: Vec<Hog>, total: f32) -> Facts {
        Facts { hogs, mem_total: 16_000_000_000, mem_available: 8_000_000_000, cores: 16, cpu_total: total, swapped: 0 }
    }

    fn hog(name: &str, cpu: f32) -> Hog {
        Hog { pid: 4242, name: name.into(), cpu, memory: 1, disk: 0 }
    }

    #[test]
    fn reads_pressure() {
        let text = "some avg10=1.50 avg60=12.25 avg300=3.00 total=1\nfull avg10=0.00 avg60=6.00 avg300=0.00 total=0\n";
        assert_eq!(parse_pressure(text), Some(Stall { some: 12.25, full: 6.0 }));
        // The cpu file has no "full" line on older kernels.
        assert_eq!(parse_pressure("some avg10=0 avg60=30.5 avg300=0 total=0"), Some(Stall { some: 30.5, full: 0.0 }));
        assert_eq!(parse_pressure(""), None);
    }

    #[test]
    fn grades_pressure() {
        let f = facts(vec![hog("big", 1.0)], 10.0);
        assert_eq!(memory_pressure(Stall { some: 20.0, full: 6.0 }, &f).status, "bad");
        assert_eq!(memory_pressure(Stall { some: 12.0, full: 1.0 }, &f).status, "warn");
        assert_eq!(memory_pressure(Stall { some: 1.0, full: 0.0 }, &f).status, "ok");
        assert_eq!(cpu_pressure(Stall { some: 70.0, full: 0.0 }, &f).status, "bad");
        assert_eq!(io_pressure(Stall { some: 25.0, full: 2.0 }, &f).status, "warn");
        // A problem explains itself.
        assert!(!memory_pressure(Stall { some: 20.0, full: 6.0 }, &f).more.is_empty());
    }

    #[test]
    fn grades_swapping() {
        let mut f = facts(vec![], 10.0);
        assert_eq!(swapping(&f).status, "ok");
        f.swapped = 500;
        assert_eq!(swapping(&f).status, "warn");
        f.swapped = 5000;
        assert_eq!(swapping(&f).status, "bad");
    }

    #[test]
    fn reads_systemd_durations() {
        assert_eq!(parse_duration("1min 2.5s"), Some(62.5));
        assert_eq!(parse_duration("456ms"), Some(0.456));
        assert_eq!(parse_duration("12.3s"), Some(12.3));
        assert_eq!(parse_duration("fast"), None);
    }

    #[test]
    fn finds_cpu_hogs() {
        // Busy, but with time to spare: not a slowdown.
        let f = facts(vec![hog("a", 99.0), hog("b", 10.0)], 20.0);
        assert_eq!(cpu_hogs(&f, Some(0.0)).unwrap().status, "ok");
        assert_eq!(cpu_hogs(&f, Some(15.0)).unwrap().status, "warn");
        // Short of time: it is, with a button to end it.
        let f = facts(vec![hog("a", 99.0), hog("b", 10.0)], 80.0);
        let c = cpu_hogs(&f, Some(0.0)).unwrap();
        assert_eq!((c.status, c.title.as_str()), ("warn", "a is keeping the processor busy"));
        assert_eq!(c.actions.len(), 1);
        assert_eq!(c.actions[0].target.as_deref(), Some("4242:a"));
        assert!(cpu_hogs(&facts(vec![hog("b", 10.0)], 20.0), None).is_none());
    }

    #[test]
    fn never_offers_to_end_the_desktop() {
        assert!(!endable(&hog("plasmashell", 99.0)));
        assert!(endable(&hog("python3", 99.0)));
    }

    #[test]
    fn reads_boot_time() {
        let text = "Startup finished in 28.967s (firmware) + 5.783s (loader) + 2.795s (kernel) + 10.100s (initrd) + 18.394s (userspace) = 1min 6.041s \ngraphical.target reached after 18.350s in userspace.\n";
        let b = parse_boot(text).unwrap();
        assert!((b.total - 66.041).abs() < 1e-6);
        assert!((b.firmware - 28.967).abs() < 1e-6);
        assert!((b.linux - 31.291).abs() < 1e-6);
        assert_eq!(secs(66.0), "1 min 6 s");
    }

    #[test]
    fn checks_unit_names() {
        assert!(valid_unit("NetworkManager-wait-online.service"));
        assert!(valid_unit("getty@tty1.service"));
        assert!(!valid_unit("x.service; rm -rf /"));
        assert!(!valid_unit("sshd.socket"));
    }
}
