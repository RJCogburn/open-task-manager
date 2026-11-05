//! The Rust side of each tool, one file per tool, named like the tool's
//! folder in `src/tools/`. Shared code lives outside this folder: the live
//! sampler, GPU readings, logging, the tray, start-up, and running external
//! programs (`programs.rs`).
//!
//! A new tool adds its file here and its commands to the list in `lib.rs`,
//! each behind a Cargo feature of its own (see `Cargo.toml`).

#[cfg(feature = "benchmarks")]
pub mod benchmarks;
// Network reads sockets with Connections' code, so it needs the module too.
#[cfg(any(feature = "connections", feature = "network"))]
pub mod connections;
#[cfg(feature = "disk-space")]
pub mod disk_space;
#[cfg(feature = "disks")]
pub mod disks;
#[cfg(feature = "disks")]
pub mod firmware;
#[cfg(feature = "installed-apps")]
pub mod installed_apps;
#[cfg(feature = "network")]
pub mod network;
#[cfg(feature = "processes")]
pub mod processes;
#[cfg(feature = "security-audit")]
pub mod security;
#[cfg(feature = "security-audit")]
pub mod security_fix;
#[cfg(feature = "sensors")]
pub mod sensors;
#[cfg(feature = "slowdown")]
pub mod slowdown;
#[cfg(feature = "slowdown")]
pub mod slowdown_watch;
#[cfg(feature = "startup-apps")]
pub mod startup_apps;

/// Tools this build left out, by their id in `src/tools/`, so the window can
/// leave out their pages too.
#[tauri::command]
pub fn missing_tools() -> Vec<&'static str> {
    [
        ("bench", cfg!(feature = "benchmarks")),
        ("connections", cfg!(feature = "connections")),
        ("space", cfg!(feature = "disk-space")),
        ("disks", cfg!(feature = "disks")),
        ("apps", cfg!(feature = "installed-apps")),
        ("network", cfg!(feature = "network")),
        ("processes", cfg!(feature = "processes")),
        ("sensors", cfg!(feature = "sensors")),
        ("security", cfg!(feature = "security-audit")),
        ("slowdown", cfg!(feature = "slowdown")),
        ("startup", cfg!(feature = "startup-apps")),
    ]
    .into_iter()
    .filter(|&(_, built)| !built)
    .map(|(id, _)| id)
    .collect()
}
