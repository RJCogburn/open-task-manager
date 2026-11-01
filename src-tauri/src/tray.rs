//! The system tray icon: keeps Open Task Manager running, and logging, with
//! the window closed. Linux tray hosts don't pass clicks on the icon itself
//! to the app, so everything goes through its menu.

use std::fs;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, State, Wry};

use crate::AppState;
use crate::logging::config_dir;
use crate::sampler::Sample;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct TrayConfig {
    enabled: bool,
    /// Closing the window hides it instead of quitting.
    close_to_tray: bool,
    /// CPU and memory next to the icon, where the tray host shows a label.
    show_usage: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self { enabled: true, close_to_tray: true, show_usage: true }
    }
}

#[derive(Default)]
pub struct Tray {
    config: Mutex<TrayConfig>,
    icon: Mutex<Option<(TrayIcon<Wry>, MenuItem<Wry>)>>,
    last_update: Mutex<Option<Instant>>,
}

fn config_file() -> std::path::PathBuf {
    config_dir().join("tray.json")
}

impl Tray {
    pub fn load() -> Self {
        let config =
            fs::read_to_string(config_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Self { config: Mutex::new(config), ..Default::default() }
    }

    /// The window should hide rather than close.
    pub fn keeps_running(&self) -> bool {
        let c = self.config.lock().unwrap();
        c.enabled && c.close_to_tray && self.icon.lock().unwrap().is_some()
    }

    pub fn is_shown(&self) -> bool {
        self.icon.lock().unwrap().is_some()
    }

    /// Adds or removes the icon to match the settings.
    pub fn apply(&self, app: &AppHandle) -> Result<(), String> {
        let enabled = self.config.lock().unwrap().enabled;
        let mut icon = self.icon.lock().unwrap();
        match (enabled, icon.is_some()) {
            (true, false) => *icon = Some(build(app).map_err(|e| format!("Could not add the tray icon: {e}"))?),
            (false, true) => {
                if let Some((tray, _)) = icon.take() {
                    let _ = tray.set_visible(false);
                    app.remove_tray_by_id(tray.id());
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Puts the latest readings in the tray, every couple of seconds.
    pub fn update(&self, s: &Sample) {
        {
            let mut last = self.last_update.lock().unwrap();
            if last.is_some_and(|t| t.elapsed() < Duration::from_millis(1900)) {
                return;
            }
            *last = Some(Instant::now());
        }
        let show_usage = self.config.lock().unwrap().show_usage;
        let icon = self.icon.lock().unwrap();
        let Some((tray, status)) = icon.as_ref() else { return };
        let mem = if s.mem_total > 0 { s.mem_used as f64 / s.mem_total as f64 * 100.0 } else { 0.0 };
        let text = format!("CPU {:.0}% · Memory {mem:.0}%", s.cpu);
        let _ = status.set_text(&text);
        let _ = tray.set_tooltip(Some(format!("Open Task Manager\n{text}")));
        let _ = tray.set_title(show_usage.then(|| format!("{:.0}%", s.cpu)));
    }
}

fn build(app: &AppHandle) -> tauri::Result<(TrayIcon<Wry>, MenuItem<Wry>)> {
    let status = MenuItem::with_id(app, "status", "CPU — · Memory —", false, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "show", "Show Open Task Manager", true, None::<&str>)?,
            &MenuItem::with_id(app, "hide", "Hide to tray", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &status,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id("main").menu(&menu).tooltip("Open Task Manager");
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_window(app),
            "hide" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok((tray, status))
}

/// Brings the window back, from the tray or from being minimized.
pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[tauri::command]
pub fn tray_get(state: State<'_, AppState>) -> TrayConfig {
    state.tray.config.lock().unwrap().clone()
}

#[tauri::command]
pub fn tray_set(app: AppHandle, state: State<'_, AppState>, config: TrayConfig) -> Result<(), String> {
    fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(config_file(), text).map_err(|e| format!("Could not save the tray settings: {e}"))?;
    let show_usage = config.show_usage;
    *state.tray.config.lock().unwrap() = config;
    state.tray.apply(&app)?;
    if !show_usage && let Some((tray, _)) = state.tray.icon.lock().unwrap().as_ref() {
        let _ = tray.set_title(None::<&str>);
    }
    Ok(())
}
