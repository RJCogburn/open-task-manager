//! The Startup page: everything that starts with the computer or the
//! desktop session, in one list, each with a switch.
//!
//! - Apps that start when you log in: XDG autostart entries, yours
//!   (~/.config/autostart) and the system's (/etc/xdg/autostart). A system
//!   entry is turned off the way the XDG spec intends, with a copy in your
//!   own folder marked Hidden=true; the system's file is never changed.
//! - Your services: systemd user units, switched with `systemctl --user`.
//! - System services: systemd system units, through pkexec.
//! - Commands your crontab runs at start-up (@reboot), commented out to
//!   turn them off.
//!
//! The parts the desktop or the system can't work without can't be
//! switched off here; a few that matter (networking, the firewall, fan
//! control) can, after a warning. Ideas from Autostart Manager by filonux.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::logging::{config_dir, xdg};
use crate::programs::{self, parse_duration};

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// "app:<file>", "user:<unit>", "system:<unit>" or "cron:<line>".
    id: String,
    /// "app", "user-service", "service" or "cron".
    kind: &'static str,
    name: String,
    description: String,
    /// What it runs.
    command: String,
    enabled: bool,
    /// For apps: "yours", or "system" (and whether you turned it off).
    origin: &'static str,
    /// Seconds it kept start-up busy at the last boot or login.
    startup: Option<f64>,
    /// Memory it uses now, in bytes.
    memory: Option<u64>,
    /// "running", "failed", "not running", or null if unknown.
    state: Option<&'static str>,
    /// Why it should stay on; switching it off asks first.
    caution: Option<String>,
    /// Why it can't be switched off here.
    locked: Option<String>,
    /// Switching it asks for the administrator password.
    admin: bool,
    /// Starts only on other desktops, so it doesn't start here anyway.
    other_desktop: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Startup {
    items: Vec<Item>,
    /// The desktop, as XDG_CURRENT_DESKTOP names it.
    desktop: String,
    /// Whether crontab is installed, so @reboot commands can be listed.
    cron: bool,
}

// ---------------------------------------------------------------------------
// Rules for what can be switched off.

/// System services that can't be switched off here: the system or the login
/// screen depends on them.
const SYSTEM_LOCKED: &[&str] = &[
    "systemd-",
    "dbus",
    "polkit",
    "getty@",
    "user@",
    "sddm",
    "gdm",
    "lightdm",
    "plasmalogin",
    "ly.service",
    "greetd",
    "display-manager",
    "lvm2-",
    "cryptsetup",
    "plymouth",
    "udev",
];

/// Your services the desktop can't work without.
const USER_LOCKED: &[&str] = &[
    "dbus",
    "pipewire",
    "wireplumber",
    "plasma-",
    "xdg-desktop-portal",
    "gnome-shell",
    "gnome-session",
    "at-spi",
    "kwin",
    "xdg-permission-store",
];

/// Things that work to switch off but cost something, and what.
const CAUTION: &[(&str, &str)] = &[
    (
        "NetworkManager",
        "it connects this computer to networks; with it off there's no network until you turn it back on",
    ),
    ("wpa_supplicant", "Wi-Fi needs it"),
    ("iwd", "Wi-Fi may need it"),
    ("bluetooth", "Bluetooth devices stop working"),
    ("ufw", "it's your firewall"),
    ("firewalld", "it's your firewall"),
    ("apparmor", "it confines programs for security"),
    ("power-profiles-daemon", "it handles power modes, and battery life may suffer"),
    ("tlp", "it handles power saving, and battery life may suffer"),
    ("upower", "battery and power readings need it"),
    ("udisks2", "mounting USB drives and disks from the desktop needs it"),
    ("accounts-daemon", "the login screen and user settings use it"),
    ("coolercontrold", "it controls your fans and pumps"),
    ("fancontrol", "it controls your fans"),
    ("nvidia-", "the NVIDIA driver uses it for suspend and power management"),
    ("snapper", "it takes your system snapshots"),
    ("limine-snapper", "it keeps the boot menu's snapshots up to date"),
    ("ananicy", "it keeps the desktop responsive under load"),
    ("clamav-freshclam", "it keeps virus definitions current"),
];

/// Autostart apps that are part of the desktop itself.
const APP_CAUTION: &[&str] = &[
    "plasmashell",
    "polkit",
    "at-spi",
    "gnome-keyring",
    "pam_kwallet",
    "kaccess",
    "powerdevil",
    "xembedsniproxy",
    "gmenudbusmenuproxy",
    "plasma-fallback-session-restore",
    "gsd-",
    "gnome-shell",
    "xfce4-",
    "xfsettingsd",
    "lxqt-",
    "mate-settings-daemon",
    "cinnamon-",
];

fn starts(name: &str, list: &[&str]) -> bool {
    list.iter().any(|p| name.starts_with(p))
}

fn caution_for(unit: &str) -> Option<String> {
    CAUTION.iter().find(|(p, _)| unit.starts_with(p)).map(|(_, why)| format!("Keep it on unless you're sure: {why}."))
}

// ---------------------------------------------------------------------------
// systemd

/// systemd's escaping of a unit name part: "arch-update" → "arch\x2dupdate".
fn escape(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_alphanumeric() || c == '_' || (c == '.' && i > 0) || c == ':' {
            out.push(c);
        } else {
            for b in c.to_string().bytes() {
                out.push_str(&format!("\\x{b:02x}"));
            }
        }
    }
    out
}

fn systemctl(user: bool, args: &[&str]) -> Option<String> {
    let mut all = Vec::new();
    if user {
        all.push("--user");
    }
    all.extend_from_slice(args);
    let out = programs::run("systemctl", &all).ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Service unit files and whether each is enabled, leaving out templates.
fn unit_files(user: bool) -> Vec<(String, bool)> {
    let text = systemctl(
        user,
        &["list-unit-files", "--type=service", "--state=enabled,disabled", "--no-legend", "--no-pager"],
    )
    .unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            Some((w.next()?.to_string(), w.next()? == "enabled"))
        })
        .filter(|(u, _)| u.ends_with(".service") && !u.contains("@."))
        .collect()
}

#[derive(Default, Clone, Debug, PartialEq)]
struct UnitInfo {
    description: String,
    command: String,
    state: Option<&'static str>,
    memory: Option<u64>,
}

/// Description, command, state and memory of each unit, in one call.
fn unit_info(user: bool, units: &[String]) -> HashMap<String, UnitInfo> {
    let mut out = HashMap::new();
    for chunk in units.chunks(200) {
        let mut args = vec!["show", "-p", "Id,Description,ExecStart,ActiveState,MemoryCurrent", "--no-pager"];
        args.extend(chunk.iter().map(String::as_str));
        out.extend(parse_show(&systemctl(user, &args).unwrap_or_default()));
    }
    out
}

fn parse_show(text: &str) -> HashMap<String, UnitInfo> {
    let mut out = HashMap::new();
    for block in text.split("\n\n") {
        let mut id = String::new();
        let mut info = UnitInfo::default();
        for line in block.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k {
                "Id" => id = v.to_string(),
                "Description" => info.description = v.to_string(),
                // "{ path=/usr/bin/x ; argv[]=/usr/bin/x --flag ; … }"
                "ExecStart" => {
                    info.command = v
                        .split(" ; ")
                        .find_map(|p| p.trim_start_matches("{ ").strip_prefix("argv[]="))
                        .unwrap_or_default()
                        .to_string()
                }
                "ActiveState" => {
                    info.state = match v {
                        "active" | "activating" | "reloading" => Some("running"),
                        "failed" => Some("failed"),
                        "inactive" | "deactivating" => Some("not running"),
                        _ => None,
                    }
                }
                "MemoryCurrent" => info.memory = v.parse().ok().filter(|&m: &u64| m > 0 && m < u64::MAX / 2),
                _ => {}
            }
        }
        if !id.is_empty() {
            out.insert(id, info);
        }
    }
    out
}

/// How long each unit kept start-up busy, from `systemd-analyze blame`.
fn blame(user: bool) -> HashMap<String, f64> {
    let args: &[&str] = if user { &["--user", "blame", "--no-pager"] } else { &["blame", "--no-pager"] };
    let text = programs::run("systemd-analyze", args).map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    parse_blame(&text.unwrap_or_default())
}

fn parse_blame(text: &str) -> HashMap<String, f64> {
    text.lines()
        .filter_map(|l| {
            let (time, unit) = l.trim().rsplit_once(' ')?;
            Some((unit.to_string(), parse_duration(time)?))
        })
        .collect()
}

/// Services switched off from this page, so they stay listed to switch back on.
fn turned_off_file() -> PathBuf {
    config_dir().join("startup-turned-off.json")
}

fn turned_off() -> HashSet<String> {
    fs::read_to_string(turned_off_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn remember(id: &str, off: bool) {
    let mut set = turned_off();
    if off {
        set.insert(id.to_string());
    } else {
        set.remove(id);
    }
    if fs::create_dir_all(config_dir()).is_ok()
        && let Ok(text) = serde_json::to_string(&set)
    {
        let _ = fs::write(turned_off_file(), text);
    }
}

fn services(user: bool, off: &HashSet<String>) -> Vec<Item> {
    let prefix = if user { "user" } else { "system" };
    let files: Vec<(String, bool)> = unit_files(user)
        .into_iter()
        .filter(|(u, enabled)| *enabled || off.contains(&format!("{prefix}:{u}")))
        .collect();
    let names: Vec<String> = files.iter().map(|(u, _)| u.clone()).collect();
    let info = unit_info(user, &names);
    let times = blame(user);
    files
        .into_iter()
        .map(|(unit, enabled)| {
            let i = info.get(&unit).cloned().unwrap_or_default();
            let locked = if user {
                starts(&unit, USER_LOCKED)
                    .then(|| "Your desktop needs it, so it can't be switched off here.".to_string())
            } else {
                starts(&unit, SYSTEM_LOCKED)
                    .then(|| "The system needs it to start or to log in, so it can't be switched off here.".to_string())
            };
            Item {
                id: format!("{prefix}:{unit}"),
                kind: if user { "user-service" } else { "service" },
                name: unit.trim_end_matches(".service").to_string(),
                description: i.description,
                command: i.command,
                enabled,
                origin: if user { "yours" } else { "system" },
                startup: times.get(&unit).copied(),
                memory: i.memory,
                state: i.state,
                caution: if locked.is_none() { caution_for(&unit) } else { None },
                locked,
                admin: !user,
                other_desktop: false,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// XDG autostart

#[derive(Default, Debug, Clone, PartialEq)]
struct Desktop {
    name: String,
    comment: String,
    exec: String,
    hidden: bool,
    gnome_enabled: Option<bool>,
    only: Vec<String>,
    not: Vec<String>,
    ours: bool,
}

/// The keys of a .desktop file's [Desktop Entry] group.
fn parse_desktop(text: &str) -> Desktop {
    let mut d = Desktop::default();
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        let list = |v: &str| v.split(';').filter(|s| !s.is_empty()).map(str::to_string).collect::<Vec<_>>();
        match k {
            "Name" => d.name = v.to_string(),
            "Comment" => d.comment = v.to_string(),
            "Exec" => d.exec = v.to_string(),
            "Hidden" => d.hidden = v.eq_ignore_ascii_case("true"),
            "X-GNOME-Autostart-enabled" => d.gnome_enabled = Some(!v.eq_ignore_ascii_case("false")),
            "OnlyShowIn" => d.only = list(v),
            "NotShowIn" => d.not = list(v),
            OVERRIDE_KEY => d.ours = true,
            _ => {}
        }
    }
    d
}

/// Marks a copy this app made to turn a system entry off.
const OVERRIDE_KEY: &str = "X-Open-Task-Manager-Override";

fn user_autostart() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("autostart")
}

fn system_autostart() -> Vec<PathBuf> {
    let dirs = std::env::var("XDG_CONFIG_DIRS").ok().filter(|v| !v.is_empty()).unwrap_or("/etc/xdg".into());
    dirs.split(':').filter(|d| d.starts_with('/')).map(|d| Path::new(d).join("autostart")).collect()
}

fn desktop_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".desktop"))
        .collect();
    names.sort();
    names
}

fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().split(':').map(str::to_string).collect()
}

fn on_this_desktop(d: &Desktop, here: &[String]) -> bool {
    (d.only.is_empty() || d.only.iter().any(|x| here.contains(x))) && !d.not.iter().any(|x| here.contains(x))
}

fn apps() -> Vec<Item> {
    let here = current_desktops();
    let user_dir = user_autostart();
    let mut names: Vec<String> = desktop_files(&user_dir);
    let systems = system_autostart();
    for dir in &systems {
        for n in desktop_files(dir) {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    // Desktops that run autostart entries as systemd units (Plasma 6, GNOME) report on them.
    let units: Vec<String> =
        names.iter().map(|n| format!("app-{}@autostart.service", escape(n.trim_end_matches(".desktop")))).collect();
    let info = unit_info(true, &units);
    let times = blame(true);
    let mut items = Vec::new();
    for (name, unit) in names.iter().zip(&units) {
        let system = systems.iter().map(|d| d.join(name)).find(|p| p.is_file());
        let user = Some(user_dir.join(name)).filter(|p| p.is_file());
        let read = |p: &Path| parse_desktop(&fs::read_to_string(p).unwrap_or_default());
        let sys = system.as_deref().map(read);
        let mine = user.as_deref().map(read);
        // An entry the system ships already hidden isn't meant to start at all.
        if mine.is_none() && sys.as_ref().is_some_and(|s| s.hidden) {
            continue;
        }
        let effective = mine.clone().or(sys.clone()).unwrap_or_default();
        // A copy of ours shows the system's details.
        let shown = if effective.ours { sys.clone().unwrap_or(effective.clone()) } else { effective.clone() };
        let enabled = !effective.hidden && effective.gnome_enabled != Some(false);
        let id = name.trim_end_matches(".desktop");
        let i = info.get(unit).cloned().unwrap_or_default();
        items.push(Item {
            id: format!("app:{name}"),
            kind: "app",
            name: if shown.name.is_empty() { id.to_string() } else { shown.name.clone() },
            description: shown.comment.clone(),
            command: shown.exec.clone(),
            enabled,
            origin: if sys.is_some() { "system" } else { "yours" },
            startup: times.get(unit).copied(),
            memory: i.memory,
            state: i.state,
            caution: APP_CAUTION
                .iter()
                .any(|p| id.contains(p))
                .then(|| "It's part of your desktop; things may stop working without it.".to_string()),
            locked: None,
            admin: false,
            other_desktop: !on_this_desktop(&shown, &here),
        });
    }
    items
}

/// Sets Hidden= in a .desktop file's text, keeping everything else.
fn set_hidden(text: &str, hidden: bool, marker: bool) -> String {
    let mut out = Vec::new();
    let mut in_entry = false;
    let mut placed = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if in_entry && !placed {
                push_keys(&mut out, hidden, marker);
                placed = true;
            }
            in_entry = t == "[Desktop Entry]";
            out.push(line.to_string());
            continue;
        }
        let key = t.split_once('=').map(|(k, _)| k.trim());
        if in_entry && matches!(key, Some("Hidden") | Some(OVERRIDE_KEY)) {
            continue;
        }
        if in_entry && key == Some("X-GNOME-Autostart-enabled") && !hidden {
            continue;
        }
        out.push(line.to_string());
    }
    if !placed {
        if !in_entry && !out.iter().any(|l| l.trim() == "[Desktop Entry]") {
            out.push("[Desktop Entry]".into());
        }
        push_keys(&mut out, hidden, marker);
    }
    out.join("\n") + "\n"
}

fn push_keys(out: &mut Vec<String>, hidden: bool, marker: bool) {
    // Trailing blank lines stay after the keys.
    let blanks = out.iter().rev().take_while(|l| l.trim().is_empty()).count();
    let at = out.len() - blanks;
    let mut keys = Vec::new();
    if hidden {
        keys.push("Hidden=true".to_string());
    }
    if marker {
        keys.push(format!("{OVERRIDE_KEY}=true"));
    }
    out.splice(at..at, keys);
}

/// Writes a file in place of another without following links.
fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("{} is a link; change it by hand.", path.display()));
    }
    let tmp = path.with_extension("desktop.otm-tmp");
    fs::write(&tmp, text).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

fn set_app(name: &str, on: bool) -> Result<String, String> {
    if !name.ends_with(".desktop") || name.contains('/') || name.starts_with('.') {
        return Err("Not an autostart entry".into());
    }
    let user_dir = user_autostart();
    let mine = user_dir.join(name);
    let system = system_autostart().into_iter().map(|d| d.join(name)).find(|p| p.is_file());
    let later = "It takes effect the next time you log in; if it's running now, it keeps running.";
    if mine.is_file() {
        let text = fs::read_to_string(&mine).map_err(|e| e.to_string())?;
        let d = parse_desktop(&text);
        if on && d.ours {
            // Our copy only hid the system's entry: removing it restores that.
            fs::remove_file(&mine).map_err(|e| e.to_string())?;
        } else {
            write_atomic(&mine, &set_hidden(&text, !on, d.ours))?;
        }
    } else if let Some(system) = system {
        if on {
            return Ok("It's already on.".into());
        }
        let text = fs::read_to_string(&system).map_err(|e| e.to_string())?;
        fs::create_dir_all(&user_dir).map_err(|e| e.to_string())?;
        write_atomic(&mine, &set_hidden(&text, true, true))?;
    } else {
        return Err("That autostart entry is gone.".into());
    }
    Ok(if on { format!("Turned on. {later}") } else { format!("Turned off. {later}") })
}

// ---------------------------------------------------------------------------
// crontab @reboot

const CRON_OFF: &str = "#otm-off# ";

fn cron_lines() -> Option<String> {
    programs::which("crontab")?;
    let out = programs::run("crontab", &["-l"]).ok()?;
    // "no crontab for user" is an empty crontab.
    Some(if out.status.success() { String::from_utf8_lossy(&out.stdout).into_owned() } else { String::new() })
}

fn cron_items(text: &str) -> Vec<Item> {
    text.lines()
        .filter_map(|l| {
            let (enabled, rest) = match l.strip_prefix(CRON_OFF) {
                Some(r) => (false, r),
                None => (true, l),
            };
            let command = rest.trim().strip_prefix("@reboot")?.trim().to_string();
            Some(Item {
                id: format!("cron:{rest}"),
                kind: "cron",
                name: command
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .to_string(),
                description: "Your crontab runs it at start-up".into(),
                command,
                enabled,
                origin: "yours",
                startup: None,
                memory: None,
                state: None,
                caution: None,
                locked: None,
                admin: false,
                other_desktop: false,
            })
        })
        .collect()
}

fn set_cron(line: &str, on: bool) -> Result<String, String> {
    let text = cron_lines().ok_or("crontab isn't installed.")?;
    let mut found = false;
    let new: Vec<String> = text
        .lines()
        .map(|l| {
            let bare = l.strip_prefix(CRON_OFF).unwrap_or(l);
            if bare == line && line.trim_start().starts_with("@reboot") {
                found = true;
                if on { bare.to_string() } else { format!("{CRON_OFF}{bare}") }
            } else {
                l.to_string()
            }
        })
        .collect();
    if !found {
        return Err("That line isn't in your crontab any more.".into());
    }
    let mut child = std::process::Command::new("crontab")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all((new.join("\n") + "\n").as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(if on { "Turned on; it runs at the next start-up.".into() } else { "Turned off.".into() })
}

// ---------------------------------------------------------------------------
// Commands

#[tauri::command]
pub async fn startup_list() -> Result<Startup, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let off = turned_off();
        let mut items = apps();
        items.extend(services(true, &off));
        items.extend(services(false, &off));
        let cron = cron_lines();
        if let Some(text) = &cron {
            items.extend(cron_items(text));
        }
        Startup { items, desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(), cron: cron.is_some() }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Switches one item on or off.
#[tauri::command]
pub async fn startup_set(id: String, on: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (kind, rest) = id.split_once(':').ok_or("Not a start-up item")?;
        match kind {
            "app" => set_app(rest, on),
            "cron" => set_cron(rest, on),
            "user" | "system" => {
                let user = kind == "user";
                let unit = rest;
                if !unit.ends_with(".service") || unit.contains('/') || unit.starts_with('-') || unit.contains("@.") {
                    return Err("Not a service".into());
                }
                if !unit_files(user).iter().any(|(u, _)| u == unit) {
                    return Err(format!("{unit} isn't installed any more."));
                }
                if !on && starts(unit, if user { USER_LOCKED } else { SYSTEM_LOCKED }) {
                    return Err(format!("{unit} can't be switched off here."));
                }
                let verb = if on { "enable" } else { "disable" };
                let out = if user {
                    programs::run("systemctl", &["--user", verb, unit])?
                } else {
                    programs::run("pkexec", &["systemctl", verb, unit])?
                };
                if out.status.code() == Some(126) {
                    return Err("Cancelled.".into());
                }
                if !out.status.success() {
                    let text = String::from_utf8_lossy(&out.stderr);
                    return Err(text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("It didn't work.").into());
                }
                remember(&id, !on);
                Ok(if on {
                    format!("{unit} will start at the next start-up. To start it now, restart or run: systemctl {}start {unit}", if user { "--user " } else { "" })
                } else {
                    format!("{unit} won't start at the next start-up. It keeps running until then.")
                })
            }
            _ => Err("Not a start-up item".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_systemd() {
        assert_eq!(escape("arch-update-tray"), "arch\\x2dupdate\\x2dtray");
        assert_eq!(escape("org.kde.discover.notifier"), "org.kde.discover.notifier");
    }

    #[test]
    fn reads_desktop_entries() {
        let d = parse_desktop(
            "[Desktop Entry]\nName=Blueman Applet\nExec=blueman-applet\nOnlyShowIn=KDE;GNOME;\n\
             X-GNOME-Autostart-enabled=false\n[Desktop Action x]\nName=Other\n",
        );
        assert_eq!(d.name, "Blueman Applet");
        assert_eq!(d.only, ["KDE", "GNOME"]);
        assert_eq!(d.gnome_enabled, Some(false));
        assert!(on_this_desktop(&d, &["KDE".into()]));
        assert!(!on_this_desktop(&d, &["XFCE".into()]));
    }

    #[test]
    fn hides_and_unhides() {
        let text = "[Desktop Entry]\nName=X\nExec=x\n\n[Desktop Action a]\nExec=y\n";
        let off = set_hidden(text, true, true);
        assert_eq!(
            off,
            "[Desktop Entry]\nName=X\nExec=x\nHidden=true\nX-Open-Task-Manager-Override=true\n\n[Desktop Action a]\nExec=y\n"
        );
        let d = parse_desktop(&off);
        assert!(d.hidden && d.ours);
        let on = set_hidden("[Desktop Entry]\nName=X\nHidden=true\nX-GNOME-Autostart-enabled=false\n", false, false);
        assert_eq!(on, "[Desktop Entry]\nName=X\n");
    }

    #[test]
    fn reads_systemctl_show() {
        let text = "Id=psd.service\nDescription=Profile-sync-daemon\nExecStart={ path=/usr/bin/psd ; argv[]=/usr/bin/psd startup ; ignore_errors=no }\nActiveState=active\nMemoryCurrent=4194304\n\nId=x.service\nDescription=X\nActiveState=failed\nMemoryCurrent=[not set]\n";
        let m = parse_show(text);
        assert_eq!(m["psd.service"].command, "/usr/bin/psd startup");
        assert_eq!(m["psd.service"].memory, Some(4194304));
        assert_eq!(m["x.service"].state, Some("failed"));
        assert_eq!(m["x.service"].memory, None);
    }

    #[test]
    fn reads_blame_and_cron() {
        let b = parse_blame("5.236s arch-update.service\n 639ms plasma-kded6.service\n");
        assert_eq!(b["arch-update.service"], 5.236);
        let items = cron_items(
            "# hi\n@reboot /home/me/bin/sync.sh --quiet\n#otm-off# @reboot rclone mount x\n0 * * * * hourly\n",
        );
        assert_eq!(items.len(), 2);
        assert_eq!((items[0].name.as_str(), items[0].enabled), ("sync.sh", true));
        assert_eq!(items[1].id, "cron:@reboot rclone mount x");
        assert!(!items[1].enabled);
    }

    #[test]
    fn refuses_what_it_should() {
        assert!(set_app("../evil.desktop", false).is_err());
        assert!(set_app("x.txt", false).is_err());
    }
}
