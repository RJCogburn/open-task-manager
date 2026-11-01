//! New versions: checking GitHub for one, and installing it with Tauri's
//! updater. Updates are signed with the project's key; the updater refuses
//! anything else. The public key and the address checked are in
//! tauri.conf.json.
//!
//! Only installs from our own packages can update themselves. A build from
//! the Arch package belongs to pacman, so it is told about new versions but
//! never replaces its own files.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::utils::config::BundleType;
use tauri::utils::platform::bundle_type;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::AppState;

#[derive(Default)]
pub struct Updates {
    /// The update found by the last check, ready to install.
    pending: Mutex<Option<Update>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    current: String,
    available: Option<Available>,
    /// How this copy was installed: appimage, deb, rpm, package-manager or development.
    method: &'static str,
    can_install: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    version: String,
    notes: Option<String>,
}

fn method() -> &'static str {
    match bundle_type() {
        Some(BundleType::AppImage) => "appimage",
        Some(BundleType::Deb) => "deb",
        Some(BundleType::Rpm) => "rpm",
        _ if cfg!(debug_assertions) => "development",
        _ => "package-manager",
    }
}

fn can_install() -> bool {
    matches!(method(), "appimage" | "deb" | "rpm")
}

/// This copy's version and how it can update, without going online.
#[tauri::command]
pub fn update_local(app: AppHandle) -> UpdateStatus {
    UpdateStatus {
        current: app.package_info().version.to_string(),
        available: None,
        method: method(),
        can_install: can_install(),
    }
}

#[tauri::command]
pub async fn update_check(app: AppHandle, state: State<'_, AppState>) -> Result<UpdateStatus, String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| format!("Could not check for updates: {e}"))?;
    let available = update.as_ref().map(|u| Available { version: u.version.clone(), notes: u.body.clone() });
    *state.updates.pending.lock().unwrap() = update;
    Ok(UpdateStatus {
        current: app.package_info().version.to_string(),
        available,
        method: method(),
        can_install: can_install(),
    })
}

/// Downloads and installs the update found by the last check, reporting
/// progress as `update-progress` events. A .deb or .rpm asks for the
/// password through polkit. The new version runs from the next start.
#[tauri::command]
pub async fn update_install(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if !can_install() {
        return Err("This copy was installed by your package manager; update it there.".into());
    }
    let update = state.updates.pending.lock().unwrap().clone().ok_or("Check for updates first.")?;
    let mut downloaded = 0u64;
    let progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit("update-progress", (downloaded, total));
            },
            || {},
        )
        .await
        .map_err(|e| format!("The update could not be installed: {e}"))?;
    *state.updates.pending.lock().unwrap() = None;
    Ok(())
}

/// Starts the program again, so an installed update takes over. The new copy
/// waits a moment, so the single-instance check doesn't find this one still
/// running and hand over to it.
#[tauri::command]
pub fn app_restart(app: AppHandle) -> Result<(), String> {
    let exe = match std::env::var_os("APPIMAGE") {
        Some(appimage) => PathBuf::from(appimage),
        None => std::env::current_exe().map_err(|e| e.to_string())?,
    };
    // A package update replaces the file this process runs from.
    let exe = exe.to_string_lossy().trim_end_matches(" (deleted)").to_string();
    std::process::Command::new("sh")
        .args(["-c", r#"sleep 1; exec "$0""#, &exe])
        .spawn()
        .map_err(|e| format!("Could not restart: {e}"))?;
    app.exit(0);
    Ok(())
}
