//! Open Task Manager: a free task manager that brings together the open
//! source tools Linux already has, in one window.

mod apps;
mod bench;
mod connections;
mod diskscan;
mod gpu;
mod processes;
mod sampler;
mod sensors;
mod tools;

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

pub struct AppState {
    shared: Arc<Mutex<sampler::Shared>>,
    interval_ms: Arc<AtomicU64>,
    scan: Arc<diskscan::ScanState>,
    rapl: sensors::RaplState,
}

/// WebKitGTK's DMA-BUF renderer draws a blank window on the NVIDIA driver.
pub fn apply_webkit_workarounds() {
    const VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
    if std::env::var_os(VAR).is_none() && std::path::Path::new("/proc/driver/nvidia/version").exists() {
        // SAFETY: called first thing in main, while the process is single-threaded.
        unsafe { std::env::set_var(VAR, "1") };
    }
}

pub fn run() {
    let shared = Arc::new(Mutex::new(sampler::Shared::new()));
    let interval_ms = Arc::new(AtomicU64::new(1000));
    let gpu = Arc::new(gpu::GpuMonitor::start());
    let state = AppState {
        shared: shared.clone(),
        interval_ms: interval_ms.clone(),
        scan: Arc::default(),
        rapl: sensors::RaplState::default(),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup(move |app| {
            sampler::spawn(app.handle().clone(), shared, gpu, interval_ms);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sampler::system_info,
            sampler::mounts,
            sampler::set_interval,
            processes::processes,
            processes::end_process,
            processes::set_priority,
            processes::admin_action,
            sensors::sensors,
            connections::connections,
            diskscan::scan_start,
            diskscan::scan_cancel,
            diskscan::scan_folder,
            diskscan::scan_summary,
            bench::bench_cpu,
            bench::bench_memory,
            bench::bench_disk,
            bench::bench_gpu,
            bench::bench_network,
            apps::installed_apps,
            tools::tool_status,
            tools::open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Open Task Manager");
}
