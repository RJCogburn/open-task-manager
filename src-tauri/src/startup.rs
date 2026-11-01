//! Starting with the desktop session, through an XDG autostart entry, which
//! GNOME, KDE, XFCE and the other desktops all read.

use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use crate::logging::xdg;

pub const MINIMIZED_FLAG: &str = "--minimized";

fn entry() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("autostart").join("open-task-manager.desktop")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Autostart {
    enabled: bool,
    minimized: bool,
    /// The program the entry starts.
    program: String,
}

#[tauri::command]
pub fn autostart_get() -> Autostart {
    let text = fs::read_to_string(entry()).unwrap_or_default();
    let exec = text.lines().find_map(|l| l.strip_prefix("Exec=")).unwrap_or_default();
    Autostart {
        enabled: !text.is_empty() && !text.lines().any(|l| l.trim() == "Hidden=true"),
        minimized: exec.contains(MINIMIZED_FLAG),
        program: exec.split(" --").next().unwrap_or_default().trim_matches('"').to_string(),
    }
}

#[tauri::command]
pub fn autostart_set(enabled: bool, minimized: bool) -> Result<Autostart, String> {
    let path = entry();
    if !enabled {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("Could not remove {}: {e}", path.display()))?;
        }
        return Ok(autostart_get());
    }
    // An AppImage runs from a temporary mount; start the AppImage file itself.
    let program = match std::env::var_os("APPIMAGE") {
        Some(appimage) => PathBuf::from(appimage),
        None => std::env::current_exe().map_err(|e| e.to_string())?,
    };
    let mut exec = format!("\"{}\"", program.to_string_lossy().replace('"', "\\\""));
    if minimized {
        exec.push(' ');
        exec.push_str(MINIMIZED_FLAG);
    }
    let text = format!(
        "[Desktop Entry]\nType=Application\nName=Open Task Manager\nComment=Start Open Task Manager when you log in\n\
         Exec={exec}\nIcon=open-task-manager\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    );
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&path, text).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(autostart_get())
}
