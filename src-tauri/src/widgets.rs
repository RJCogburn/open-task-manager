//! Desktop widgets: a small borderless panel showing the readings you pick,
//! and the readings feed, a file other desktop widgets (Plasma, GNOME,
//! conky...) can read instead of collecting the same data again.
//!
//! Both take the sampler's readings; nothing here measures anything itself.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::AppState;
use crate::logging::config_dir;
use crate::sampler::Sample;

/// The panel window's label, also used in the capabilities file.
pub const LABEL: &str = "widgets";

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct WidgetConfig {
    enabled: bool,
    /// Readings shown, in order: cpu, memory, gpu, network, disks, temperature.
    items: Vec<String>,
    /// "vertical" or "horizontal".
    layout: String,
    graphs: bool,
    /// How solid the tiles are, 0.3 to 1.
    opacity: f64,
    /// Where it was last put, on desktops that let apps place windows.
    position: Option<(f64, f64)>,
    /// Write readings to the feed file.
    feed: bool,
}

impl Default for WidgetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            items: vec!["cpu".into(), "memory".into(), "network".into()],
            layout: "vertical".into(),
            graphs: true,
            opacity: 0.85,
            position: None,
            feed: false,
        }
    }
}

#[derive(Default)]
pub struct Widgets {
    config: Mutex<WidgetConfig>,
    save_pending: AtomicBool,
}

fn config_file() -> PathBuf {
    config_dir().join("widgets.json")
}

/// The feed lives in the per-user runtime folder: in memory, and readable
/// only by this user.
fn feed_file() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|d| d.is_absolute())?;
    Some(dir.join("open-task-manager").join("readings.json"))
}

/// Panel size for the chosen readings: fixed tiles, stacked or side by side.
fn size(c: &WidgetConfig) -> (f64, f64) {
    let n = c.items.len().max(1) as f64;
    let (tile_w, tile_h) = (210.0, if c.graphs { 108.0 } else { 64.0 });
    let (gap, pad) = (8.0, 8.0);
    if c.layout == "horizontal" {
        (pad * 2.0 + n * tile_w + (n - 1.0) * gap, pad * 2.0 + tile_h)
    } else {
        (pad * 2.0 + tile_w, pad * 2.0 + n * tile_h + (n - 1.0) * gap)
    }
}

impl Widgets {
    pub fn load() -> Self {
        let config =
            fs::read_to_string(config_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Self { config: Mutex::new(config), ..Default::default() }
    }

    fn save(&self) -> Result<(), String> {
        fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(&*self.config.lock().unwrap()).map_err(|e| e.to_string())?;
        fs::write(config_file(), text).map_err(|e| format!("Could not save the widget settings: {e}"))
    }

    /// Opens, resizes or closes the panel to match the settings.
    pub fn apply(&self, app: &AppHandle) -> Result<(), String> {
        let c = self.config.lock().unwrap().clone();
        let existing = app.get_webview_window(LABEL);
        if !c.enabled || c.items.is_empty() {
            if let Some(window) = existing {
                let _ = window.destroy();
            }
            return Ok(());
        }
        let (width, height) = size(&c);
        match existing {
            Some(window) => {
                let _ = window.set_size(LogicalSize::new(width, height));
            }
            None => {
                let mut builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html#widgets".into()))
                    .title("Open Task Manager widgets")
                    .inner_size(width, height)
                    .resizable(false)
                    .decorations(false)
                    .transparent(true)
                    .shadow(false)
                    .skip_taskbar(true)
                    // Kept behind other windows where the desktop allows it (X11);
                    // on Wayland the desktop decides.
                    .always_on_bottom(true);
                if let Some((x, y)) = c.position {
                    builder = builder.position(x, y);
                }
                builder.build().map_err(|e| format!("Could not open the widget panel: {e}"))?;
            }
        }
        let _ = app.emit("widgets-changed", &c);
        Ok(())
    }

    /// Remembers where the panel was dragged, saving once the dragging stops.
    pub fn moved(self: &Arc<Self>, position: (f64, f64)) {
        self.config.lock().unwrap().position = Some(position);
        if !self.save_pending.swap(true, Ordering::SeqCst) {
            let me = self.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(700));
                me.save_pending.store(false, Ordering::SeqCst);
                let _ = me.save();
            });
        }
    }

    /// The panel was closed from the desktop (Alt+F4 and the like).
    pub fn closed(&self) {
        self.config.lock().unwrap().enabled = false;
        let _ = self.save();
    }

    /// Whether the panel or the feed needs a kind of reading: "gpu", "disks"
    /// or "network". The temperature reading includes the graphics card's.
    pub fn wants(&self, kind: &str) -> bool {
        let c = self.config.lock().unwrap();
        c.feed || (c.enabled && c.items.iter().any(|i| i == kind || (kind == "gpu" && i == "temperature")))
    }

    /// Writes the latest readings for other desktop widgets, if turned on.
    /// Written to a temporary file and renamed, so readers never see half a file.
    pub fn feed(&self, s: &Sample) {
        if !self.config.lock().unwrap().feed {
            return;
        }
        let Some(file) = feed_file() else { return };
        let Some(dir) = file.parent() else { return };
        if !dir.exists() {
            if fs::create_dir_all(dir).is_err() {
                return;
            }
            // Only this user may read the readings.
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        }
        let body = serde_json::json!({ "format": 1, "app": "open-task-manager", "readings": s });
        let tmp = file.with_extension("json.tmp");
        if fs::write(&tmp, body.to_string()).is_ok() {
            let _ = fs::rename(&tmp, &file);
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetStatus {
    config: WidgetConfig,
    /// Where the feed is written, or None when there is no runtime folder.
    feed_path: Option<String>,
}

#[tauri::command]
pub fn widgets_get(state: State<'_, AppState>) -> WidgetStatus {
    WidgetStatus {
        config: state.widgets.config.lock().unwrap().clone(),
        feed_path: feed_file().map(|f| f.to_string_lossy().into_owned()),
    }
}

#[tauri::command]
pub fn widgets_set(app: AppHandle, state: State<'_, AppState>, mut config: WidgetConfig) -> Result<(), String> {
    const KNOWN: [&str; 6] = ["cpu", "memory", "gpu", "network", "disks", "temperature"];
    config.items.retain(|i| KNOWN.contains(&i.as_str()));
    config.items.dedup();
    config.opacity = config.opacity.clamp(0.3, 1.0);
    // The window remembers where it was put; the settings page doesn't know.
    config.position = state.widgets.config.lock().unwrap().position;
    if !config.feed
        && let Some(file) = feed_file()
    {
        // No stale readings left behind for widgets to show.
        let _ = fs::remove_file(file);
    }
    *state.widgets.config.lock().unwrap() = config;
    state.widgets.save()?;
    state.widgets.apply(&app)
}

/// Opens the main window, from a double-click on the panel.
#[tauri::command]
pub fn show_main(app: AppHandle) {
    crate::tray::show_window(&app);
}
