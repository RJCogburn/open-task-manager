//! Open Task Manager: a free task manager that brings together the open
//! source tools Linux already has, in one window.

mod apps;
mod bench;
mod connections;
mod diskscan;
mod drives;
mod gpu;
mod logging;
mod netactivity;
mod processes;
mod sampler;
mod sensors;
mod startup;
mod tools;
mod tray;

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use tauri::Manager;

pub struct AppState {
    shared: Arc<Mutex<sampler::Shared>>,
    interval_ms: Arc<AtomicU64>,
    scan: Arc<diskscan::ScanState>,
    health: Arc<drives::HealthCache>,
    net: Arc<netactivity::NetState>,
    logger: Arc<logging::Logger>,
    tray: Arc<tray::Tray>,
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
    let logger = Arc::new(logging::Logger::load());
    let tray = Arc::new(tray::Tray::load());
    let state = AppState {
        shared: shared.clone(),
        interval_ms: interval_ms.clone(),
        scan: Arc::default(),
        health: Arc::default(),
        net: Arc::default(),
        logger: logger.clone(),
        tray: tray.clone(),
        rapl: sensors::RaplState::default(),
    };
    tauri::Builder::default()
        // Opening the app again while it runs in the tray brings its window back.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_window(app)))
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup(move |app| {
            if let Err(e) = tray.apply(app.handle()) {
                eprintln!("{e}");
            }
            sampler::spawn(app.handle().clone(), shared, gpu, interval_ms, logger, tray.clone());
            // The window starts hidden. Started minimized (from the login
            // autostart entry), it stays in the tray, or minimized without one.
            let minimized = std::env::args().any(|a| a == startup::MINIMIZED_FLAG);
            if let Some(window) = app.get_webview_window("main")
                && !(minimized && tray.is_shown())
            {
                let _ = window.show();
                if minimized {
                    let _ = window.minimize();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.state::<AppState>().tray.keeps_running()
            {
                api.prevent_close();
                let _ = window.hide();
            }
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
            drives::drives,
            drives::drive_health_admin,
            netactivity::net_activity,
            netactivity::process_source,
            logging::logging_get,
            logging::logging_set,
            startup::autostart_get,
            startup::autostart_set,
            tray::tray_get,
            tray::tray_set,
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
