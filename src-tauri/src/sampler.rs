//! The live sampler: one background thread that refreshes CPU, memory,
//! processes, disk and network counters on a timer and sends the result to the
//! window as a `sample` event. Pages that need more (the process table) read
//! the same refreshed `System` instead of refreshing it again.
//!
//! GPU, disk and network readings are skipped while nothing needs them: their
//! page is switched off, and neither the widget panel nor the log uses them.
//! The readings feed shares only what is measured.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sysinfo::{Networks, ProcessRefreshKind, ProcessesToUpdate, System, ThreadKind, UpdateKind, Users};
use tauri::{AppHandle, Emitter, State};

use crate::AppState;
use crate::gpu::{GpuMonitor, GpuSample};
use crate::logging::Logger;
use crate::tray::Tray;
use crate::widgets::Widgets;

pub struct Shared {
    pub sys: System,
    pub users: Users,
    /// Seconds covered by the last process refresh, for turning byte counts into rates.
    pub elapsed: f64,
    /// When a page last asked for process details. Reading every process
    /// costs more than the rest of a sample together, so it is only done
    /// while someone is looking.
    pub process_demand: Option<Instant>,
    processes_read: Instant,
}

impl Shared {
    pub fn new() -> Self {
        let mut sys = System::new().expect("sysinfo is supported on this platform");
        sys.refresh_cpu_all();
        sys.refresh_memory();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, process_refresh());
        let users = Users::new_with_refreshed_list().expect("sysinfo is supported on this platform");
        Self { sys, users, elapsed: 1.0, process_demand: None, processes_read: Instant::now() }
    }

    /// Reads every process's details now.
    pub fn refresh_processes(&mut self) {
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, process_refresh());
        self.elapsed = self.processes_read.elapsed().as_secs_f64().max(0.001);
        self.processes_read = Instant::now();
    }

    /// Marks process details as wanted, and makes sure they are fresh. After a
    /// pause it reads twice, a moment apart, so CPU use is current rather than
    /// averaged over the pause. Called with the lock held; it may release and
    /// take it again.
    #[cfg(feature = "processes")]
    pub fn want_processes(shared: &Mutex<Shared>) {
        let mut s = shared.lock().unwrap();
        s.process_demand = Some(Instant::now());
        if s.processes_read.elapsed() > Duration::from_millis(2500) {
            s.refresh_processes();
            drop(s);
            thread::sleep(Duration::from_millis(300));
            shared.lock().unwrap().refresh_processes();
        }
    }

    fn processes_wanted(&self) -> bool {
        self.process_demand.is_some_and(|t| t.elapsed() < Duration::from_secs(5))
    }
}

/// Processes running, from /proc, without reading any of them.
fn count_processes() -> usize {
    fs::read_dir("/proc")
        .map(|d| d.flatten().filter(|e| e.file_name().as_encoded_bytes().iter().all(u8::is_ascii_digit)).count())
        .unwrap_or(0)
}

pub(crate) fn process_refresh() -> ProcessRefreshKind {
    // Without tasks: sysinfo otherwise also lists each thread as a process.
    ProcessRefreshKind::nothing()
        .without_tasks()
        .with_cpu()
        .with_memory()
        .with_disk_usage()
        .with_user(UpdateKind::OnlyIfNotSet)
        .with_cmd(UpdateKind::OnlyIfNotSet)
        .with_exe(UpdateKind::OnlyIfNotSet)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub(crate) time: u64,
    pub(crate) cpu: f32,
    pub(crate) cores: Vec<f32>,
    pub(crate) freqs: Vec<u64>,
    pub(crate) cpu_temp: Option<f32>,
    pub(crate) mem_used: u64,
    pub(crate) mem_total: u64,
    pub(crate) swap_used: u64,
    pub(crate) swap_total: u64,
    pub(crate) disks: Vec<DiskIo>,
    pub(crate) nets: Vec<NetIo>,
    pub(crate) gpus: Vec<GpuSample>,
    pub(crate) processes: usize,
    pub(crate) threads: usize,
    pub(crate) uptime: u64,
    pub(crate) load: [f64; 3],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiskIo {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) read: f64,
    pub(crate) write: f64,
    pub(crate) busy: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetIo {
    pub(crate) name: String,
    pub(crate) rx: f64,
    pub(crate) tx: f64,
    pub(crate) rx_total: u64,
    pub(crate) tx_total: u64,
}

/// Pages switched off in the window, by tool id.
pub type DisabledTools = Arc<Mutex<Vec<String>>>;

#[allow(clippy::too_many_arguments)]
pub fn spawn(
    app: AppHandle,
    shared: Arc<Mutex<Shared>>,
    gpu: Arc<GpuMonitor>,
    interval_ms: Arc<AtomicU64>,
    disabled: DisabledTools,
    logger: Arc<Logger>,
    tray: Arc<Tray>,
    widgets: Arc<Widgets>,
) {
    // "gpu", "disks" and "network" are both tool ids and kinds of reading.
    let wanted = {
        let (logger, widgets) = (logger.clone(), widgets.clone());
        move |kind: &str| {
            !disabled.lock().unwrap().iter().any(|t| t == kind) || widgets.wants(kind) || logger.wants(kind)
        }
    };
    thread::spawn(move || {
        let mut networks = Networks::new_with_refreshed_list().ok();
        // After a pause the counters cover the whole pause, so the first
        // refresh only starts them again.
        let mut nets_paused = false;
        let mut disks = DiskStats::default();
        disks.read();
        let cpu_temp = find_cpu_temp_input();
        let mut last = Instant::now();
        let mut ticks: u64 = 0;
        loop {
            thread::sleep(Duration::from_millis(interval_ms.load(Ordering::Relaxed)));
            let now = Instant::now();
            let dt = now.duration_since(last).as_secs_f64().max(0.001);
            last = now;
            ticks += 1;

            let mut sample = {
                let mut s = shared.lock().unwrap();
                s.sys.refresh_cpu_all();
                s.sys.refresh_memory();
                let wanted = s.processes_wanted();
                if wanted {
                    s.refresh_processes();
                }
                if ticks.is_multiple_of(60) {
                    s.users.refresh();
                }
                let sys = &s.sys;
                let load = System::load_average().map(|l| [l.one, l.five, l.fifteen]).unwrap_or_default();
                Sample {
                    time: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
                    cpu: sys.global_cpu_usage(),
                    cores: sys.cpus().iter().map(|c| c.usage()).collect(),
                    freqs: sys.cpus().iter().map(|c| c.frequency()).collect(),
                    cpu_temp: None,
                    mem_used: sys.total_memory().saturating_sub(sys.available_memory()),
                    mem_total: sys.total_memory(),
                    swap_used: sys.used_swap(),
                    swap_total: sys.total_swap(),
                    disks: Vec::new(),
                    nets: Vec::new(),
                    gpus: Vec::new(),
                    processes: if wanted {
                        sys.processes().values().filter(|p| p.thread_kind() != Some(ThreadKind::Userland)).count()
                    } else {
                        count_processes()
                    },
                    threads: total_threads(),
                    uptime: System::uptime().unwrap_or(0),
                    load,
                }
            };

            sample.cpu_temp = cpu_temp.as_ref().and_then(read_millidegrees);
            if wanted("disks") {
                sample.disks = disks.rates(dt);
            } else {
                disks.pause();
            }
            if !wanted("network") {
                nets_paused = true;
            } else if let Some(networks) = networks.as_mut() {
                networks.refresh(true);
                if std::mem::take(&mut nets_paused) {
                    networks.refresh(true);
                }
                sample.nets = networks
                    .iter()
                    .filter(|(name, _)| name.as_os_str() != "lo")
                    .map(|(name, data)| NetIo {
                        name: name.to_string_lossy().into_owned(),
                        rx: data.received() as f64 / dt,
                        tx: data.transmitted() as f64 / dt,
                        rx_total: data.total_received(),
                        tx_total: data.total_transmitted(),
                    })
                    .collect();
                sample.nets.sort_by(|a, b| a.name.cmp(&b.name));
            }
            sample.gpus = gpu.sample(wanted("gpu"));
            logger.record(&sample);
            tray.update(&sample);
            widgets.feed(&sample);
            let _ = app.emit("sample", &sample);
        }
    });
}

/// Threads on the whole system: the total in the fourth field of
/// /proc/loadavg ("running/total").
fn total_threads() -> usize {
    fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().nth(3)?.split('/').nth(1)?.parse().ok())
        .unwrap_or(0)
}

/// The CPU package temperature input from the kernel's hwmon interface, the
/// same source lm-sensors reads.
pub(crate) fn find_cpu_temp_input() -> Option<PathBuf> {
    for entry in fs::read_dir("/sys/class/hwmon").ok()?.flatten() {
        let name = fs::read_to_string(entry.path().join("name")).unwrap_or_default();
        if matches!(name.trim(), "k10temp" | "zenpower" | "coretemp" | "cpu_thermal") {
            let input = entry.path().join("temp1_input");
            if input.exists() {
                return Some(input);
            }
        }
    }
    None
}

pub(crate) fn read_millidegrees(path: &PathBuf) -> Option<f32> {
    fs::read_to_string(path).ok()?.trim().parse::<f32>().ok().map(|m| m / 1000.0)
}

/// Read/write rates and busy time per whole disk, from /proc/diskstats.
#[derive(Default)]
struct DiskStats {
    prev: HashMap<String, (u64, u64, u64)>,
    now: HashMap<String, (u64, u64, u64)>,
}

impl DiskStats {
    /// Forgets the counters, so the first rates after a pause don't cover it.
    fn pause(&mut self) {
        self.prev.clear();
        self.now.clear();
    }

    fn read(&mut self) {
        self.now.clear();
        let Ok(text) = fs::read_to_string("/proc/diskstats") else { return };
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 13 {
                continue;
            }
            let name = f[2];
            // Whole disks only: partitions have no entry of their own in /sys/block.
            if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") {
                continue;
            }
            if !std::path::Path::new("/sys/block").join(name).exists() {
                continue;
            }
            let sectors_read: u64 = f[5].parse().unwrap_or(0);
            let sectors_written: u64 = f[9].parse().unwrap_or(0);
            let ms_busy: u64 = f[12].parse().unwrap_or(0);
            self.now.insert(name.to_string(), (sectors_read, sectors_written, ms_busy));
        }
    }

    fn rates(&mut self, dt: f64) -> Vec<DiskIo> {
        std::mem::swap(&mut self.prev, &mut self.now);
        self.read();
        let mut out: Vec<DiskIo> = self
            .now
            .iter()
            .filter_map(|(name, &(r, w, busy))| {
                let &(pr, pw, pbusy) = self.prev.get(name)?;
                Some(DiskIo {
                    name: name.clone(),
                    label: disk_label(name),
                    read: r.saturating_sub(pr) as f64 * 512.0 / dt,
                    write: w.saturating_sub(pw) as f64 * 512.0 / dt,
                    busy: (busy.saturating_sub(pbusy) as f64 / (dt * 1000.0) * 100.0).min(100.0),
                })
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

/// A friendlier name: the model for real drives, the mapper name for dm devices.
fn disk_label(name: &str) -> String {
    let base = std::path::Path::new("/sys/block").join(name);
    if let Ok(dm) = fs::read_to_string(base.join("dm/name")) {
        let dm = dm.trim();
        // A LUKS mapping is named after its UUID; say what it sits on instead.
        let under: Vec<String> = fs::read_dir(base.join("slaves"))
            .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        return match (dm.starts_with("luks-") || dm.starts_with("crypt"), under.is_empty()) {
            (true, false) => format!("Encrypted volume on {} ({name})", under.join(", ")),
            (true, true) => format!("Encrypted volume ({name})"),
            _ => format!("{dm} ({name})"),
        };
    }
    match fs::read_to_string(base.join("device/model")) {
        Ok(model) if !model.trim().is_empty() => format!("{} ({name})", model.trim()),
        _ => name.to_string(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    host_name: String,
    user_name: String,
    home_dir: String,
    os: String,
    kernel: String,
    cpu_brand: String,
    physical_cores: usize,
    logical_cores: usize,
    mem_total: u64,
    swap_total: u64,
}

#[tauri::command]
pub fn system_info(state: State<'_, AppState>) -> SystemInfo {
    let s = state.shared.lock().unwrap();
    SystemInfo {
        host_name: System::host_name().unwrap_or_default(),
        user_name: std::env::var("USER").unwrap_or_default(),
        home_dir: std::env::var("HOME").unwrap_or_default(),
        os: System::long_os_version().unwrap_or_default(),
        kernel: System::kernel_version().unwrap_or_default(),
        cpu_brand: s.sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        physical_cores: System::physical_core_count().unwrap_or(0),
        logical_cores: s.sys.cpus().len(),
        mem_total: s.sys.total_memory(),
        swap_total: s.sys.total_swap(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    device: String,
    mount_point: String,
    file_system: String,
    total: u64,
    available: u64,
    removable: bool,
}

#[tauri::command]
pub fn mounts() -> Vec<Mount> {
    let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else { return Vec::new() };
    let mut seen = std::collections::HashSet::new();
    disks
        .iter()
        // Btrfs subvolumes show up once per mount; keep the first of each device.
        .filter(|d| seen.insert(d.name().to_os_string()))
        .map(|d| Mount {
            device: d.name().to_string_lossy().into_owned(),
            mount_point: d.mount_point().to_string_lossy().into_owned(),
            file_system: d.file_system().to_string_lossy().into_owned(),
            total: d.total_space(),
            available: d.available_space(),
            removable: d.is_removable(),
        })
        .collect()
}

#[tauri::command]
pub fn set_disabled_tools(state: State<'_, AppState>, tools: Vec<String>) {
    *state.disabled_tools.lock().unwrap() = tools;
}

#[tauri::command]
pub fn set_interval(state: State<'_, AppState>, ms: u64) {
    state.interval_ms.store(ms.clamp(250, 10_000), Ordering::Relaxed);
}
