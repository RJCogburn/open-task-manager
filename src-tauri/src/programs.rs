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

/// The package that provides each tool, by distribution: Arch, Debian,
/// Fedora, openSUSE.
fn package(program: &str) -> Option<[&'static str; 4]> {
    Some(match program {
        "sensors" => ["lm_sensors", "lm-sensors", "lm_sensors", "sensors"],
        "ss" => ["iproute2", "iproute2", "iproute", "iproute2"],
        "fio" => ["fio"; 4],
        "glmark2" => ["glmark2"; 4],
        "curl" => ["curl"; 4],
        "nvme" => ["nvme-cli"; 4],
        "lynis" => ["lynis"; 4],
        "fwupdmgr" => ["fwupd"; 4],
        "lspci" => ["pciutils"; 4],
        "pkexec" => ["polkit", "pkexec", "polkit", "polkit"],
        "smartctl" => ["smartmontools"; 4],
        "lsblk" => ["util-linux"; 4],
        "xdg-open" => ["xdg-utils"; 4],
        _ => return None,
    })
}

/// This system's package manager: its index into `package`'s list (Arch,
/// Debian, Fedora, openSUSE), and the command that installs without asking.
fn manager() -> Option<(usize, &'static [&'static str])> {
    if which("pacman").is_some() {
        Some((0, &["pacman", "-S", "--needed", "--noconfirm"]))
    } else if which("apt-get").is_some() {
        Some((1, &["apt-get", "install", "-y"]))
    } else if which("zypper").is_some() {
        Some((3, &["zypper", "--non-interactive", "install"]))
    } else if which("dnf").is_some() {
        Some((2, &["dnf", "install", "-y"]))
    } else {
        None
    }
}

/// The command that installs a tool's package without further questions,
/// with this system's package manager.
fn install_command(program: &str) -> Option<Vec<String>> {
    let (i, command) = manager()?;
    let name = package(program)?[i];
    Some(command.iter().copied().chain([name]).map(str::to_string).collect())
}

fn install_hint(program: &str) -> Option<String> {
    install_command(program).map(|c| format!("sudo {}", c.join(" ")))
}

/// Whether the app can install a tool itself. Installing goes through
/// pkexec, so pkexec can't install itself.
fn installable(program: &str) -> bool {
    program != "pkexec" && install_command(program).is_some()
}

/// Installs a tool's package through polkit, which asks for the password.
/// Only the packages listed in `package` can be installed this way.
#[tauri::command]
pub async fn install_tool(program: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(command) = install_command(&program).filter(|_| installable(&program)) else {
            return Err(format!("Open Task Manager can't install {program} on this system."));
        };
        let args: Vec<&str> = command.iter().map(String::as_str).collect();
        let out = run("pkexec", &args)?;
        if out.status.code() == Some(126) {
            return Err("Cancelled.".into());
        }
        if !out.status.success() || which(&program).is_none() {
            let text = String::from_utf8_lossy(&out.stderr);
            let why = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the package manager gave no reason");
            return Err(format!("Installing {program} didn't work: {why}"));
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    program: String,
    installed: bool,
    install_hint: Option<String>,
    /// Whether the app can install it itself, with install_tool.
    installable: bool,
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
            installable: installable(&program),
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
