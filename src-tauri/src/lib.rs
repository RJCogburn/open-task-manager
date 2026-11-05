//! Open Task Manager: a free task manager that brings together the open
//! source tools Linux already has, in one window.

mod gpu;
mod logging;
mod plugins;
mod programs;
mod sampler;
mod startup;
mod tools;
mod tray;
mod updates;
mod widgets;

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use tauri::Manager;

pub struct AppState {
    shared: Arc<Mutex<sampler::Shared>>,
    interval_ms: Arc<AtomicU64>,
    disabled_tools: sampler::DisabledTools,
    #[cfg(feature = "disk-space")]
    scan: Arc<tools::disk_space::ScanState>,
    #[cfg(feature = "disks")]
    health: Arc<tools::disks::HealthCache>,
    #[cfg(feature = "network")]
    net: Arc<tools::network::NetState>,
    logger: Arc<logging::Logger>,
    tray: Arc<tray::Tray>,
    updates: updates::Updates,
    widgets: Arc<widgets::Widgets>,
    #[cfg(feature = "sensors")]
    rapl: tools::sensors::RaplState,
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
    // Tauri starts an async worker per CPU thread by default, 32 on a big
    // machine. The app waits on the system far more than it computes, and slow
    // work goes to blocking threads, so a few workers are plenty.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("could not start the async runtime");
    tauri::async_runtime::set(runtime.handle().clone());
    let shared = Arc::new(Mutex::new(sampler::Shared::new()));
    let interval_ms = Arc::new(AtomicU64::new(1000));
    let disabled_tools = sampler::DisabledTools::default();
    let gpu = Arc::new(gpu::GpuMonitor::start());
    let logger = Arc::new(logging::Logger::load());
    let tray = Arc::new(tray::Tray::load());
    let widgets = Arc::new(widgets::Widgets::load());
    let state = AppState {
        shared: shared.clone(),
        interval_ms: interval_ms.clone(),
        disabled_tools: disabled_tools.clone(),
        #[cfg(feature = "disk-space")]
        scan: Arc::default(),
        #[cfg(feature = "disks")]
        health: Arc::default(),
        #[cfg(feature = "network")]
        net: Arc::default(),
        logger: logger.clone(),
        tray: tray.clone(),
        updates: updates::Updates::default(),
        widgets: widgets.clone(),
        #[cfg(feature = "sensors")]
        rapl: tools::sensors::RaplState::default(),
    };
    tauri::Builder::default()
        // Opening the app again while it runs in the tray brings its window back.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_window(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .setup(move |app| {
            if let Err(e) = tray.apply(app.handle()) {
                eprintln!("{e}");
            }
            sampler::spawn(
                app.handle().clone(),
                shared,
                gpu,
                interval_ms,
                disabled_tools,
                logger,
                tray.clone(),
                widgets.clone(),
            );
            if let Err(e) = widgets.apply(app.handle()) {
                eprintln!("{e}");
            }
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
            let state = window.state::<AppState>();
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                    if state.tray.keeps_running() {
                        api.prevent_close();
                        let _ = window.hide();
                    } else {
                        // Without the tray, closing the window quits, widgets and all.
                        window.app_handle().exit(0);
                    }
                }
                tauri::WindowEvent::CloseRequested { .. } if window.label() == widgets::LABEL => state.widgets.closed(),
                tauri::WindowEvent::Moved(position) if window.label() == widgets::LABEL => {
                    let logical = position.to_logical::<f64>(window.scale_factor().unwrap_or(1.0));
                    state.widgets.moved((logical.x, logical.y));
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            // The app itself
            sampler::system_info,
            sampler::mounts,
            sampler::set_disabled_tools,
            tools::missing_tools,
            // Plugins
            plugins::plugins_list,
            plugins::plugin_source,
            plugins::plugin_approve,
            plugins::plugin_run,
            plugins::plugins_open_folder,
            sampler::set_interval,
            logging::logging_get,
            logging::logging_set,
            logging::log_days,
            logging::log_read,
            startup::autostart_get,
            startup::autostart_set,
            tray::tray_get,
            tray::tray_set,
            programs::tool_status,
            programs::install_tool,
            programs::open_folder,
            updates::update_local,
            updates::update_check,
            updates::update_install,
            updates::app_restart,
            widgets::widgets_get,
            widgets::widgets_set,
            widgets::show_main,
            // Tools: one group per file in tools/
            #[cfg(feature = "benchmarks")]
            tools::benchmarks::bench_cpu,
            #[cfg(feature = "benchmarks")]
            tools::benchmarks::bench_memory,
            #[cfg(feature = "benchmarks")]
            tools::benchmarks::bench_disk,
            #[cfg(feature = "benchmarks")]
            tools::benchmarks::bench_gpu,
            #[cfg(feature = "benchmarks")]
            tools::benchmarks::bench_network,
            #[cfg(feature = "connections")]
            tools::connections::connections,
            #[cfg(feature = "disk-space")]
            tools::disk_space::scan_start,
            #[cfg(feature = "disk-space")]
            tools::disk_space::scan_cancel,
            #[cfg(feature = "disk-space")]
            tools::disk_space::scan_folder,
            #[cfg(feature = "disk-space")]
            tools::disk_space::scan_summary,
            #[cfg(feature = "disks")]
            tools::disks::drives,
            #[cfg(feature = "disks")]
            tools::disks::drive_health_admin,
            #[cfg(feature = "disks")]
            tools::firmware::firmware,
            #[cfg(feature = "disks")]
            tools::firmware::firmware_refresh,
            #[cfg(feature = "disks")]
            tools::firmware::firmware_update,
            #[cfg(feature = "disks")]
            tools::firmware::wd_firmware_check,
            #[cfg(feature = "disks")]
            tools::firmware::wd_firmware_install,
            #[cfg(feature = "installed-apps")]
            tools::installed_apps::installed_apps,
            #[cfg(feature = "network")]
            tools::network::net_activity,
            #[cfg(feature = "network")]
            tools::network::process_source,
            #[cfg(feature = "processes")]
            tools::processes::processes,
            #[cfg(feature = "processes")]
            tools::processes::end_process,
            #[cfg(feature = "processes")]
            tools::processes::set_priority,
            #[cfg(feature = "processes")]
            tools::processes::admin_action,
            #[cfg(feature = "sensors")]
            tools::sensors::sensors,
            #[cfg(feature = "security-audit")]
            tools::security::security_audit,
            #[cfg(feature = "security-audit")]
            tools::security::security_cancel,
            #[cfg(feature = "security-audit")]
            tools::security::security_last,
            #[cfg(feature = "security-audit")]
            tools::security_fix::security_fix,
            #[cfg(feature = "security-audit")]
            tools::security_fix::security_undo,
            #[cfg(feature = "security-audit")]
            tools::security_fix::security_changes,
            #[cfg(feature = "startup-apps")]
            tools::startup_apps::startup_list,
            #[cfg(feature = "startup-apps")]
            tools::startup_apps::startup_set,
            #[cfg(feature = "slowdown")]
            tools::slowdown::slowdown_check,
            #[cfg(feature = "slowdown")]
            tools::slowdown_watch::slowdown_watch,
            #[cfg(feature = "slowdown")]
            tools::slowdown_watch::slowdown_watch_stop,
            #[cfg(feature = "slowdown")]
            tools::slowdown::slowdown_fix,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Open Task Manager");
}
