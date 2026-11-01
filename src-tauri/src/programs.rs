//! Running the open source tools this app builds on, and finding out which
//! of them are installed.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use serde::Serialize;

pub fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(program)).find(|p| p.is_file())
}

/// Runs a tool to completion and keeps its output. A missing tool becomes a
/// message that says how to install it.
pub fn run(program: &str, args: &[&str]) -> Result<Output, String> {
    if which(program).is_none() {
        return Err(missing(program));
    }
    Command::new(program).args(args).stdin(Stdio::null()).output().map_err(|e| format!("Could not run {program}: {e}"))
}

pub fn missing(program: &str) -> String {
    match install_hint(program) {
        Some(hint) => format!("missing-tool:{program} is not installed. Install it with: {hint}"),
        None => format!("missing-tool:{program} is not installed."),
    }
}

/// The package that provides each tool, for the package manager on this system.
fn install_hint(program: &str) -> Option<String> {
    let package = |arch: &str, debian: &str, fedora: &str, suse: &str| {
        if which("pacman").is_some() {
            Some(format!("sudo pacman -S {arch}"))
        } else if which("apt").is_some() {
            Some(format!("sudo apt install {debian}"))
        } else if which("zypper").is_some() {
            Some(format!("sudo zypper install {suse}"))
        } else if which("dnf").is_some() {
            Some(format!("sudo dnf install {fedora}"))
        } else {
            None
        }
    };
    if program == "speedtest" {
        // Ookla ships it themselves rather than through distribution repositories.
        return Some(if which("pacman").is_some() {
            "an AUR helper, e.g. yay -S ookla-speedtest-bin, or see https://www.speedtest.net/apps/cli".into()
        } else {
            "the instructions at https://www.speedtest.net/apps/cli".into()
        });
    }
    match program {
        "sensors" => package("lm_sensors", "lm-sensors", "lm_sensors", "sensors"),
        "ss" => package("iproute2", "iproute2", "iproute", "iproute2"),
        "fio" => package("fio", "fio", "fio", "fio"),
        "glmark2" => package("glmark2", "glmark2", "glmark2", "glmark2"),
        "curl" => package("curl", "curl", "curl", "curl"),
        "lspci" => package("pciutils", "pciutils", "pciutils", "pciutils"),
        "pkexec" => package("polkit", "pkexec", "polkit", "polkit"),
        "smartctl" => package("smartmontools", "smartmontools", "smartmontools", "smartmontools"),
        "lsblk" => package("util-linux", "util-linux", "util-linux", "util-linux"),
        "xdg-open" => package("xdg-utils", "xdg-utils", "xdg-utils", "xdg-utils"),
        _ => None,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    program: String,
    installed: bool,
    install_hint: Option<String>,
}

/// Whether each program is installed, and how to install it if not. The
/// window asks about the programs its tools' credits name.
#[tauri::command]
pub fn tool_status(programs: Vec<String>) -> Vec<ToolStatus> {
    programs
        .into_iter()
        // Plain program names only; never a path.
        .filter(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)))
        .map(|program| ToolStatus {
            installed: which(&program).is_some(),
            install_hint: install_hint(&program),
            program,
        })
        .collect()
}

/// Opens a folder in the file manager.
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    if which("xdg-open").is_none() {
        return Err(missing("xdg-open"));
    }
    Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        // Reap it once it exits so it doesn't linger as a zombie.
        .map(|mut child| drop(std::thread::spawn(move || child.wait())))
        .map_err(|e| e.to_string())
}
