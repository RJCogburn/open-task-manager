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
