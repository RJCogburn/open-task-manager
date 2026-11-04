//! Recording hardware readings to CSV files, one file per kind of hardware
//! per day, so they can be looked at later in a spreadsheet. The sampler
//! hands every sample here; this decides whether it is time to write.

use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;
use crate::sampler::Sample;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct LogConfig {
    enabled: bool,
    cpu: bool,
    memory: bool,
    disks: bool,
    network: bool,
    gpu: bool,
    temperatures: bool,
    /// Seconds between rows.
    every: u64,
    /// Files older than this many days are deleted.
    keep_days: u64,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cpu: true,
            memory: true,
            disks: false,
            network: false,
            gpu: false,
            temperatures: true,
            every: 10,
            keep_days: 14,
        }
    }
}

pub struct Logger {
    config: Mutex<LogConfig>,
    last: Mutex<Option<Instant>>,
    day: Mutex<String>,
}

pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("open-task-manager")
}

fn log_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("open-task-manager").join("logs")
}

pub(crate) fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(fallback))
}

fn config_file() -> PathBuf {
    config_dir().join("logging.json")
}

impl Logger {
    pub fn load() -> Self {
        let config = fs::read_to_string(config_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self { config: Mutex::new(config), last: Mutex::new(None), day: Mutex::new(String::new()) }
    }

    /// Whether the log records a kind of reading: "gpu", "disks" or
    /// "network". Temperatures include the graphics cards'.
    pub fn wants(&self, kind: &str) -> bool {
        let c = self.config.lock().unwrap();
        c.enabled
            && match kind {
                "gpu" => c.gpu || c.temperatures,
                "disks" => c.disks,
                "network" => c.network,
                _ => false,
            }
    }

    pub fn record(&self, s: &Sample) {
        let config = self.config.lock().unwrap().clone();
        if !config.enabled {
            return;
        }
        {
            let mut last = self.last.lock().unwrap();
            if last.is_some_and(|t| {
                t.elapsed() < Duration::from_secs(config.every.max(1)).saturating_sub(Duration::from_millis(100))
            }) {
                return;
            }
            *last = Some(Instant::now());
        }
        let (day, time) = local_time();
        let dir = log_dir();
        if fs::create_dir_all(&dir).is_err() {
            return;
        }
        {
            let mut current = self.day.lock().unwrap();
            if *current != day {
                *current = day.clone();
                remove_old(&dir, config.keep_days);
            }
        }

        if config.cpu {
            let freqs: Vec<u64> = s.freqs.iter().copied().filter(|&f| f > 0).collect();
            let freq = if freqs.is_empty() { 0 } else { freqs.iter().sum::<u64>() / freqs.len() as u64 };
            let cores = s.cores.iter().map(|c| format!("{c:.1}")).collect::<Vec<_>>().join(";");
            append(
                &dir,
                "cpu",
                &day,
                "time,usage_percent,avg_mhz,load_1m,processes,threads,per_core_percent",
                &[format!("{time},{:.1},{freq},{:.2},{},{},{cores}", s.cpu, s.load[0], s.processes, s.threads)],
            );
        }
        if config.memory {
            append(
                &dir,
                "memory",
                &day,
                "time,used_bytes,total_bytes,swap_used_bytes,swap_total_bytes",
                &[format!("{time},{},{},{},{}", s.mem_used, s.mem_total, s.swap_used, s.swap_total)],
            );
        }
        if config.disks {
            let rows: Vec<String> = s
                .disks
                .iter()
                .map(|d| format!("{time},{},{:.0},{:.0},{:.1}", d.name, d.read, d.write, d.busy))
                .collect();
            append(&dir, "disks", &day, "time,disk,read_bytes_per_s,write_bytes_per_s,busy_percent", &rows);
        }
        if config.network {
            let rows: Vec<String> = s
                .nets
                .iter()
                .map(|n| format!("{time},{},{:.0},{:.0},{},{}", n.name, n.rx, n.tx, n.rx_total, n.tx_total))
                .collect();
            append(
                &dir,
                "network",
                &day,
                "time,interface,download_bytes_per_s,upload_bytes_per_s,received_total,sent_total",
                &rows,
            );
        }
        if config.gpu {
            let opt = |v: Option<f32>| v.map(|v| format!("{v:.1}")).unwrap_or_default();
            let rows: Vec<String> = s
                .gpus
                .iter()
                .map(|g| {
                    format!(
                        "{time},{},{},{},{},{}",
                        g.name.replace(',', " "),
                        opt(g.util),
                        g.mem_used.map(|m| m.to_string()).unwrap_or_default(),
                        opt(g.temp),
                        opt(g.power)
                    )
                })
                .collect();
            append(&dir, "gpu", &day, "time,gpu,usage_percent,memory_used_bytes,temp_c,power_w", &rows);
        }
        if config.temperatures {
            let mut rows = Vec::new();
            if let Some(t) = s.cpu_temp {
                rows.push(format!("{time},CPU,{t:.1}"));
            }
            for g in &s.gpus {
                if let Some(t) = g.temp {
                    rows.push(format!("{time},{},{t:.1}", g.name.replace(',', " ")));
                }
            }
            #[cfg(feature = "disks")]
            for name in crate::tools::disks::disk_names() {
                if let Some(t) = crate::tools::disks::sensor_temp(&name) {
                    rows.push(format!("{time},{name},{t:.1}"));
                }
            }
            append(&dir, "temperatures", &day, "time,sensor,celsius", &rows);
        }
    }
}

fn append(dir: &std::path::Path, kind: &str, day: &str, header: &str, rows: &[String]) {
    if rows.is_empty() {
        return;
    }
    let path = dir.join(format!("{kind}-{day}.csv"));
    let new = !path.exists();
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else { return };
    let mut text = String::new();
    if new {
        let _ = writeln!(text, "{header}");
    }
    for row in rows {
        let _ = writeln!(text, "{row}");
    }
    let _ = file.write_all(text.as_bytes());
}

fn remove_old(dir: &std::path::Path, keep_days: u64) {
    let Some(cutoff) = SystemTime::now().checked_sub(Duration::from_secs(keep_days.max(1) * 86_400)) else { return };
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let is_log = entry.path().extension().is_some_and(|e| e == "csv");
        let old = entry.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < cutoff);
        if is_log && old {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Today's date and the time now, in local time: "2026-11-01" and "2026-11-01 14:03:05".
fn local_time() -> (String, String) {
    let now =
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as libc::time_t;
    // SAFETY: localtime_r writes only into the struct it is given.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    let day = format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday);
    let time = format!("{day} {:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec);
    (day, time)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStatus {
    config: LogConfig,
    folder: String,
    size: u64,
    files: usize,
}

#[tauri::command]
pub fn logging_get(state: State<'_, AppState>) -> LogStatus {
    let dir = log_dir();
    let files: Vec<u64> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .collect();
    LogStatus {
        config: state.logger.config.lock().unwrap().clone(),
        folder: dir.to_string_lossy().into_owned(),
        size: files.iter().sum(),
        files: files.len(),
    }
}

#[tauri::command]
pub fn logging_set(state: State<'_, AppState>, config: LogConfig) -> Result<(), String> {
    fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(config_file(), text).map_err(|e| format!("Could not save the logging settings: {e}"))?;
    if config.enabled {
        fs::create_dir_all(log_dir()).map_err(|e| e.to_string())?;
    }
    *state.logger.config.lock().unwrap() = config;
    // Write a row straight away rather than after a full interval.
    *state.logger.last.lock().unwrap() = None;
    Ok(())
}

// Reading the logs back for the History page.

/// The kinds of log, in the order the History page shows them.
const KINDS: [&str; 6] = ["cpu", "memory", "disks", "network", "gpu", "temperatures"];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogDay {
    day: String,
    kinds: Vec<String>,
    size: u64,
}

/// The days with logs, newest first.
#[tauri::command]
pub fn log_days() -> Vec<LogDay> {
    let mut days: Vec<LogDay> = Vec::new();
    for entry in fs::read_dir(log_dir()).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((kind, day)) = name.strip_suffix(".csv").and_then(split_name) else { continue };
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        match days.iter_mut().find(|d| d.day == day) {
            Some(d) => {
                d.kinds.push(kind.to_string());
                d.size += size;
            }
            None => days.push(LogDay { day: day.to_string(), kinds: vec![kind.to_string()], size }),
        }
    }
    for d in &mut days {
        d.kinds.sort_by_key(|k| KINDS.iter().position(|x| x == k));
    }
    days.sort_by(|a, b| b.day.cmp(&a.day));
    days
}

/// "cpu-2026-11-01" into ("cpu", "2026-11-01"), for the kinds this app writes.
fn split_name(name: &str) -> Option<(&str, &str)> {
    let cut = name.len().checked_sub(10)?;
    let (kind, day) = (name.get(..cut)?, name.get(cut..)?);
    let kind = kind.strip_suffix('-')?;
    (KINDS.contains(&kind) && valid_day(day)).then_some((kind, day))
}

fn valid_day(day: &str) -> bool {
    day.len() == 10 && day.char_indices().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSeries {
    label: String,
    /// The average in each slice of the window; null where nothing was recorded.
    values: Vec<Option<f64>>,
    avg: Option<f64>,
    max: Option<f64>,
    /// When the peak was, in seconds after midnight.
    max_at: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogGroup {
    id: String,
    title: String,
    /// "percent", "mhz", "load", "bytes", "celsius" or "watts".
    unit: String,
    series: Vec<LogSeries>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogView {
    day: String,
    /// The window, in seconds after midnight.
    from: u32,
    to: u32,
    /// Seconds each value covers.
    slice: u32,
    /// Seconds between recorded rows.
    every: Option<u32>,
    rows: usize,
    groups: Vec<LogGroup>,
}

/// One recorded value: which chart and line it belongs to, and when.
struct Point {
    group: usize,
    label: String,
    secs: u32,
    value: f64,
}

/// The charts, as (id, title, unit), in the order shown.
const GROUPS: [(&str, &str, &str); 10] = [
    ("cpu", "Processor usage", "percent"),
    ("freq", "Processor speed", "mhz"),
    ("load", "Load average (1 minute)", "load"),
    ("memory", "Memory and swap in use", "percent"),
    ("disk-io", "Disk transfer", "bytes"),
    ("disk-busy", "Disk busy", "percent"),
    ("network", "Network", "bytes"),
    ("gpu", "Graphics usage", "percent"),
    ("gpu-power", "Graphics power", "watts"),
    ("temps", "Temperatures", "celsius"),
];

fn group(id: &str) -> usize {
    GROUPS.iter().position(|g| g.0 == id).unwrap_or(0)
}

/// Reads one day's logs into charts covering `from` to `to` seconds after
/// midnight, in at most `slices` values per line.
#[tauri::command]
pub fn log_read(day: String, from: u32, to: u32, slices: u32) -> Result<LogView, String> {
    if !valid_day(&day) {
        return Err("Not a day".into());
    }
    let to = to.clamp(1, 86_400);
    let from = from.min(to - 1);
    let dir = log_dir();
    let mut points = Vec::new();
    let mut times: Vec<u32> = Vec::new();
    for kind in KINDS {
        let Ok(text) = fs::read_to_string(dir.join(format!("{kind}-{day}.csv"))) else { continue };
        let before = points.len();
        parse(kind, &text, &mut points);
        if times.is_empty() && points.len() > before {
            times = points[before..].iter().map(|p| p.secs).collect();
            times.dedup();
        }
    }
    if points.is_empty() {
        return Err(format!("Nothing was recorded on {day}."));
    }
    let every = spacing(&times);
    // Each slice covers at least two rows, so a slice is never empty while recording ran.
    let span = to - from;
    let n = slices.clamp(1, 2000).min((span / (every.unwrap_or(1).max(1) * 2)).max(1));
    let slice = span.div_ceil(n);
    let rows = times.iter().filter(|&&t| t >= from && t < to).count();
    Ok(LogView { day, from, to, slice, every, rows, groups: collect(points, from, to, n, slice) })
}

/// The usual gap between rows: the median, so pauses don't count.
fn spacing(times: &[u32]) -> Option<u32> {
    let mut gaps: Vec<u32> = times.windows(2).map(|w| w[1].saturating_sub(w[0])).filter(|&g| g > 0).collect();
    gaps.sort_unstable();
    gaps.get(gaps.len() / 2).copied()
}

fn collect(points: Vec<Point>, from: u32, to: u32, n: u32, slice: u32) -> Vec<LogGroup> {
    struct Acc {
        sums: Vec<(f64, u32)>,
        total: f64,
        count: u32,
        max: Option<(f64, u32)>,
    }
    let mut lines: Vec<(usize, String, Acc)> = Vec::new();
    for p in points {
        if p.secs < from || p.secs >= to || !p.value.is_finite() {
            continue;
        }
        let i = match lines.iter().position(|l| l.0 == p.group && l.1 == p.label) {
            Some(i) => i,
            None => {
                let acc = Acc { sums: vec![(0.0, 0); n as usize], total: 0.0, count: 0, max: None };
                lines.push((p.group, p.label, acc));
                lines.len() - 1
            }
        };
        let acc = &mut lines[i].2;
        let s = &mut acc.sums[((p.secs - from) / slice).min(n - 1) as usize];
        s.0 += p.value;
        s.1 += 1;
        acc.total += p.value;
        acc.count += 1;
        if acc.max.is_none_or(|(m, _)| p.value > m) {
            acc.max = Some((p.value, p.secs));
        }
    }
    let mut groups: Vec<LogGroup> = Vec::new();
    for (g, (id, title, unit)) in GROUPS.iter().enumerate() {
        let series: Vec<LogSeries> = lines
            .iter()
            .filter(|l| l.0 == g)
            // Lines that stayed at zero (an idle network link, an unused disk) are left out.
            .filter(|l| l.2.max.is_some_and(|(m, _)| m > 0.0))
            .map(|(_, label, a)| LogSeries {
                label: label.clone(),
                values: a.sums.iter().map(|&(sum, c)| (c > 0).then(|| sum / c as f64)).collect(),
                avg: (a.count > 0).then(|| a.total / a.count as f64),
                max: a.max.map(|m| m.0),
                max_at: a.max.map(|m| m.1),
            })
            .collect();
        if !series.is_empty() {
            groups.push(LogGroup { id: id.to_string(), title: title.to_string(), unit: unit.to_string(), series });
        }
    }
    groups
}

/// "2026-11-01 14:03:05" into seconds after midnight.
fn seconds(time: &str) -> Option<u32> {
    let clock = time.get(11..19)?;
    let mut parts = clock.split(':').map(|p| p.parse::<u32>().ok());
    let (h, m, s) = (parts.next()??, parts.next()??, parts.next()??);
    (h < 24 && m < 60 && s < 61).then_some(h * 3600 + m * 60 + s.min(59))
}

/// Turns one log file into points. Columns are found by name, so files from
/// older versions with fewer columns still read.
fn parse(kind: &str, text: &str, out: &mut Vec<Point>) {
    let mut lines = text.lines();
    let Some(header) = lines.next() else { return };
    let cols: Vec<&str> = header.split(',').collect();
    let at = |name: &str| cols.iter().position(|c| *c == name);
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let Some(secs) = f.first().and_then(|t| seconds(t)) else { continue };
        let num = |name: &str| at(name).and_then(|i| f.get(i)).and_then(|v| v.parse::<f64>().ok());
        let name = |name: &str| at(name).and_then(|i| f.get(i)).map(|v| v.to_string()).unwrap_or_default();
        let mut add = |g: &str, label: String, value: Option<f64>| {
            if let Some(value) = value {
                out.push(Point { group: group(g), label, secs, value });
            }
        };
        match kind {
            "cpu" => {
                add("cpu", "CPU".into(), num("usage_percent"));
                add("freq", "Average".into(), num("avg_mhz").filter(|&v| v > 0.0));
                add("load", "Load".into(), num("load_1m"));
            }
            "memory" => {
                let share = |used: Option<f64>, total: Option<f64>| {
                    used.zip(total).filter(|(_, t)| *t > 0.0).map(|(u, t)| u / t * 100.0)
                };
                add("memory", "Memory".into(), share(num("used_bytes"), num("total_bytes")));
                add("memory", "Swap".into(), share(num("swap_used_bytes"), num("swap_total_bytes")));
            }
            "disks" => {
                let disk = name("disk");
                add("disk-io", format!("{disk} read"), num("read_bytes_per_s"));
                add("disk-io", format!("{disk} write"), num("write_bytes_per_s"));
                add("disk-busy", disk, num("busy_percent"));
            }
            "network" => {
                let link = name("interface");
                add("network", format!("{link} download"), num("download_bytes_per_s"));
                add("network", format!("{link} upload"), num("upload_bytes_per_s"));
            }
            "gpu" => {
                let gpu = name("gpu");
                add("gpu", gpu.clone(), num("usage_percent"));
                add("gpu-power", gpu, num("power_w"));
            }
            "temperatures" => add("temps", name("sensor"), num("celsius")),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_days() {
        assert_eq!(split_name("cpu-2026-11-01"), Some(("cpu", "2026-11-01")));
        assert_eq!(split_name("temperatures-2026-11-01"), Some(("temperatures", "2026-11-01")));
        assert_eq!(split_name("other-2026-11-01"), None);
        assert_eq!(split_name("cpu-2026-11-é1"), None);
        assert!(!valid_day("../../etc/x"));
        assert_eq!(seconds("2026-11-01 14:03:05"), Some(14 * 3600 + 3 * 60 + 5));
    }

    #[test]
    fn reads_and_slices() {
        let mut points = Vec::new();
        let text = "time,usage_percent,avg_mhz,load_1m\n\
                    2026-11-01 00:00:00,10,3000,1\n\
                    2026-11-01 00:00:10,30,3000,1\n\
                    2026-11-01 00:00:20,50,3000,1\n\
                    2026-11-01 00:00:30,70,3000,1\n";
        parse("cpu", text, &mut points);
        let groups = collect(points, 0, 40, 2, 20);
        let cpu = &groups[0].series[0];
        assert_eq!(cpu.values, vec![Some(20.0), Some(60.0)]);
        assert_eq!(cpu.avg, Some(40.0));
        assert_eq!((cpu.max, cpu.max_at), (Some(70.0), Some(30)));
        assert_eq!(spacing(&[0, 10, 20, 500, 510]), Some(10));
    }
}
