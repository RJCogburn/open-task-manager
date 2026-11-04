//! What can be done about Security audit findings: what Lynis found for
//! each (from its log and report), advice where the right answer is to
//! leave things as they are, and fixes for the findings that have a safe one.
//!
//! Every fix is narrow: a setting from a fixed list, a file from Lynis's own
//! list, a module from a short list. Each writes only files named after this
//! app, or changes one permission, and is recorded so it can be undone from
//! the page. Changes run through pkexec, which asks for the password.

use std::fs;
use std::io::Write as _;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::logging::config_dir;
use crate::programs;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    /// Passed back to `security_fix` unchanged, with `target`.
    fix: String,
    target: Option<String>,
    label: String,
    explain: String,
    /// The command it runs, to show.
    command: String,
    admin: bool,
    /// Only looks; changes nothing.
    read_only: bool,
}

/// A change made from the page, with what undoes it.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    fix: String,
    target: Option<String>,
    label: String,
    undo_fix: String,
    undo_target: Option<String>,
    time: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixResult {
    message: String,
    /// What a read-only action printed.
    output: Option<String>,
}

/// What the page adds to a finding.
#[derive(Default)]
pub struct Extra {
    pub found: Vec<String>,
    pub advice: Option<String>,
    pub actions: Vec<Action>,
}

const SYSCTL_FILE: &str = "/etc/sysctl.d/90-open-task-manager.conf";
/// ufw, the firewall, applies the kernel settings in this file each time it
/// starts, after the system's own; a setting made only in SYSCTL_FILE would
/// be undone at every start-up.
const UFW_SYSCTL: &str = "/etc/ufw/sysctl.conf";
const MODPROBE_FILE: &str = "/etc/modprobe.d/open-task-manager-blacklist.conf";
const COREDUMP_FILE: &str = "/etc/systemd/coredump.conf.d/50-open-task-manager.conf";
const LIMITS_FILE: &str = "/etc/security/limits.d/50-open-task-manager.conf";

/// Kernel settings this app will change: the value Lynis prefers, and what
/// it costs, if anything.
const SYSCTLS: &[(&str, &str, &str)] = &[
    ("dev.tty.ldisc_autoload", "0", ""),
    ("fs.protected_fifos", "2", ""),
    ("fs.protected_regular", "2", ""),
    ("fs.protected_hardlinks", "1", ""),
    ("fs.protected_symlinks", "1", ""),
    ("fs.suid_dumpable", "0", ""),
    ("kernel.core_uses_pid", "1", ""),
    ("kernel.ctrl-alt-del", "0", ""),
    ("kernel.kptr_restrict", "2", ""),
    ("kernel.randomize_va_space", "2", ""),
    ("kernel.dmesg_restrict", "1", "Reading the kernel log (dmesg) will need administrator rights."),
    (
        "kernel.sysrq",
        "0",
        "Turns off the Magic SysRq keys (Alt+SysRq+REISUB), which some people use to get out of a frozen system.",
    ),
    (
        "kernel.yama.ptrace_scope",
        "1",
        "Debuggers can then only attach to programs they started themselves, unless run as administrator.",
    ),
    ("kernel.unprivileged_bpf_disabled", "1", ""),
    ("net.ipv4.conf.all.log_martians", "1", ""),
    ("net.ipv4.conf.default.log_martians", "1", ""),
    (
        "net.ipv4.conf.all.send_redirects",
        "0",
        "Leave it if this computer routes traffic for other devices, such as sharing its connection.",
    ),
    (
        "net.ipv4.conf.default.send_redirects",
        "0",
        "Leave it if this computer routes traffic for other devices, such as sharing its connection.",
    ),
    ("net.ipv4.conf.all.accept_redirects", "0", ""),
    ("net.ipv4.conf.default.accept_redirects", "0", ""),
    ("net.ipv6.conf.all.accept_redirects", "0", ""),
    ("net.ipv6.conf.default.accept_redirects", "0", ""),
    ("net.ipv4.conf.all.accept_source_route", "0", ""),
    ("net.ipv4.conf.default.accept_source_route", "0", ""),
    ("net.ipv6.conf.all.accept_source_route", "0", ""),
    ("net.ipv6.conf.default.accept_source_route", "0", ""),
    ("net.ipv4.conf.all.bootp_relay", "0", ""),
    ("net.ipv4.conf.all.proxy_arp", "0", ""),
    ("net.ipv4.tcp_syncookies", "1", ""),
    ("net.ipv4.icmp_echo_ignore_broadcasts", "1", ""),
    ("net.ipv4.icmp_ignore_bogus_error_responses", "1", ""),
];

/// Settings left alone on purpose, and why.
const SYSCTL_NO: &[(&str, &str)] = &[
    (
        "kernel.modules_disabled",
        "stops the kernel loading any more drivers until the next restart, so a device plugged in later wouldn't work. Meant for locked-down servers.",
    ),
    ("kernel.perf_event_paranoid", "stops profiling tools such as perf from working for your user."),
    ("net.ipv4.tcp_timestamps", "has a small security benefit and can slow down fast connections."),
];

/// Kernel modules for network protocols and ports a desktop rarely needs.
const MODULES: &[(&str, &str)] = &[
    ("dccp", "the DCCP network protocol"),
    ("sctp", "the SCTP network protocol"),
    ("rds", "the RDS network protocol"),
    ("tipc", "the TIPC network protocol"),
    ("firewire-core", "FireWire ports"),
];

/// Services this app will switch on, and what they do.
const SERVICES: &[(&str, &str)] = &[("clamav-freshclam.service", "keeps ClamAV's virus database up to date")];

/// The files and folders Lynis checks permissions on (from its default
/// profile), the only ones this app will change permissions of.
const PERM_PATHS: &[&str] = &[
    "/boot/grub/grub.cfg",
    "/boot/grub2/grub.cfg",
    "/boot/grub2/user.cfg",
    "/etc/at.allow",
    "/etc/at.deny",
    "/etc/cron.allow",
    "/etc/cron.deny",
    "/etc/crontab",
    "/etc/group",
    "/etc/group-",
    "/etc/hosts.allow",
    "/etc/hosts.deny",
    "/etc/issue",
    "/etc/issue.net",
    "/etc/lilo.conf",
    "/etc/motd",
    "/etc/passwd",
    "/etc/passwd-",
    "/etc/ssh/sshd_config",
    "/etc/hosts.equiv",
    "/etc/shosts.equiv",
    "/root/.rhosts",
    "/root/.rlogin",
    "/root/.shosts",
    "/root/.ssh",
    "/etc/cron.d",
    "/etc/cron.daily",
    "/etc/cron.hourly",
    "/etc/cron.weekly",
    "/etc/cron.monthly",
];

fn action(fix: &str, target: Option<String>, label: String, explain: String, command: String) -> Action {
    Action { fix: fix.into(), target, label, explain, command, admin: true, read_only: false }
}

fn look(fix: &str, target: Option<String>, label: String, explain: String, command: String, admin: bool) -> Action {
    Action { fix: fix.into(), target, label, explain, command, admin, read_only: true }
}

/// The lines of Lynis's log written while one test ran, without timestamps.
fn section<'a>(log: &'a str, test: &str) -> Vec<&'a str> {
    let marker = format!("Performing test ID {test} ");
    let mut out = Vec::new();
    let mut inside = false;
    for line in log.lines() {
        let text = line.get(20..).unwrap_or(line);
        if text.starts_with("Performing test ID ") {
            inside = text.starts_with(&marker);
            continue;
        }
        if inside {
            out.push(text);
        }
    }
    out
}

fn sysctl_now(key: &str) -> Option<String> {
    fs::read_to_string(format!("/proc/sys/{}", key.replace('.', "/"))).ok().map(|v| v.trim().to_string())
}

/// What to show and offer for one finding.
pub fn extra(test: &str, text: &str, report: &str, log: &str) -> Extra {
    let mut x = Extra::default();
    match test {
        "KRNL-6000" => sysctls(report, &mut x),
        "FILE-7524" => {
            for line in section(log, test) {
                // "Outcome: permissions of file /etc/x are not matching expected value (644 != 600)"
                let Some(rest) = line.strip_prefix("Outcome: permissions of file ") else { continue };
                let Some((path, modes)) = rest.split_once(" are not matching expected value (") else { continue };
                let Some((now, want)) = modes.trim_end_matches(')').split_once(" != ") else { continue };
                x.found.push(format!("{path} is {now}; Lynis expects {want}"));
                if PERM_PATHS.contains(&path) && octal(want) && octal(now) {
                    x.actions.push(action(
                        "chmod",
                        Some(format!("{path}:{want}:{now}")),
                        format!("Set {path} to {want}"),
                        format!(
                            "Changes the permissions of {path} from {now} to {want}, so only its owner can read it."
                        ),
                        format!("chmod {want} {path}"),
                    ));
                }
            }
        }
        "BOOT-5264" => {
            // "Result: alsa-state.service has exposure value 9.6 with predicate 'UNSAFE'"
            let mut units: Vec<(f32, String, String)> = section(log, test)
                .iter()
                .filter_map(|l| {
                    let rest = l.strip_prefix("Result: ")?;
                    let (unit, rest) = rest.split_once(" has exposure value ")?;
                    let (value, rest) = rest.split_once(" with predicate '")?;
                    Some((value.parse().ok()?, unit.to_string(), rest.trim_end_matches('\'').to_string()))
                })
                .collect();
            units.sort_by(|a, b| b.0.total_cmp(&a.0));
            let unsafe_count = units.iter().filter(|u| u.2 == "UNSAFE").count();
            if !units.is_empty() {
                x.advice = Some(format!(
                    "systemd rates how tightly each service is sandboxed, from 0 (locked down) to 10 (no limits). \
                     {unsafe_count} of {} services here are rated UNSAFE. That's normal: most services come from your \
                     distribution without sandboxing, and adding it means editing each service's settings and testing \
                     it still works. Worth doing for services you wrote or expose to the network; otherwise, leave them.",
                    units.len()
                ));
            }
            for (value, unit, predicate) in units.iter().take(12) {
                x.found.push(format!("{unit}: {value:.1}, {predicate}"));
            }
            for (_, unit, _) in units.iter().take(5) {
                if valid_unit(unit) {
                    x.actions.push(look(
                        "unit-security",
                        Some(unit.clone()),
                        format!("Details for {unit}"),
                        format!("Lists each sandboxing option {unit} could use, and which it does."),
                        format!("systemd-analyze security {unit}"),
                        false,
                    ));
                }
            }
        }
        "KRNL-5820" => {
            x.advice = Some(
                "When a program crashes, Linux can save a copy of its memory (a core dump) to help find the bug. \
                 Those copies can hold passwords and other secrets from the program's memory. Unless you debug \
                 crashing programs, they aren't needed."
                    .into(),
            );
            if fs::metadata(COREDUMP_FILE).is_err() {
                x.actions.push(action(
                    "coredumps-off",
                    None,
                    "Turn off core dumps".into(),
                    "Tells systemd not to store core dumps, and sets the core dump size limit to 0 for everyone."
                        .into(),
                    format!("write {COREDUMP_FILE} and {LIMITS_FILE}"),
                ));
            }
        }
        "MALW-3286" => {
            for (unit, what) in SERVICES {
                if fs::metadata(format!("/usr/lib/systemd/system/{unit}")).is_ok() {
                    x.actions.push(action(
                        "service-on",
                        Some(unit.to_string()),
                        "Keep virus definitions updated".into(),
                        format!("Starts {unit}, which {what}, now and at every start-up."),
                        format!("systemctl enable --now {unit}"),
                    ));
                }
            }
        }
        "PKGS-7398" if programs::which("pacman").is_some() => {
            if programs::which("arch-audit").is_some() {
                x.actions.push(look(
                    "arch-audit",
                    None,
                    "Check for vulnerable packages".into(),
                    "Lists installed packages with known security problems, from the Arch Linux security tracker."
                        .into(),
                    "arch-audit".into(),
                    false,
                ));
            } else {
                x.actions.push(action(
                    "install",
                    Some("arch-audit".into()),
                    "Install arch-audit".into(),
                    "Installs arch-audit, which compares installed packages with the Arch Linux security tracker. \
                     Lynis looks for it on its next run."
                        .into(),
                    "pacman -S arch-audit".into(),
                ));
            }
        }
        "AUTH-9228" => x.actions.push(look(
            "pwck",
            None,
            "Show what pwck finds".into(),
            "Checks the user and password files for mistakes, without changing them.".into(),
            "pwck -r".into(),
            true,
        )),
        "AUTH-9230" => {
            let defs = fs::read_to_string("/etc/login.defs").unwrap_or_default();
            let method = defs
                .lines()
                .find_map(|l| l.trim().strip_prefix("ENCRYPT_METHOD"))
                .map(|m| m.trim().to_uppercase())
                .unwrap_or_default();
            if method == "YESCRYPT" {
                x.advice = Some(
                    "Nothing to do. This computer stores passwords with yescrypt (ENCRYPT_METHOD in \
                     /etc/login.defs), which is slow to crack by design. The rounds setting Lynis looks for only \
                     applies to the older SHA-512 method."
                        .into(),
                );
            }
        }
        "AUTH-9282" | "AUTH-9286" => {
            x.advice = Some(
                "Best left alone on a personal computer. Making passwords expire is no longer recommended (NIST \
                 SP 800-63B): people forced to change them pick weaker ones. A long password you don't reuse \
                 elsewhere is what matters."
                    .into(),
            )
        }
        "AUTH-9328" => {
            x.advice = Some(
                "Best left alone unless several people use this computer. A stricter umask hides new files from \
                 other user accounts; with one user it changes little, and can stop shared folders and some \
                 programs working."
                    .into(),
            )
        }
        "AUTH-9262" => {
            x.advice = Some(
                "This only checks new passwords set with the passwd command, against rules such as length. On a \
                 computer only you use, you choose your own password; make it long and not used anywhere else."
                    .into(),
            )
        }
        "PKGS-7312" => {
            x.advice = Some(
                "Lynis says this on every rolling-release system, whether updates are waiting or not. Keep \
                 updating as you normally do (for example sudo pacman -Syu). It's best done yourself rather than \
                 from here, so you can see what changes."
                    .into(),
            )
        }
        "USB-1000" => {
            x.advice = Some(
                "Not recommended on a desktop: it would stop USB sticks, card readers and external drives \
                 working. It's meant for servers in shared spaces."
                    .into(),
            )
        }
        "FILE-6310" => {
            x.advice = Some(
                "Only possible when installing or repartitioning, and of little use on a desktop. Leave it.".into(),
            )
        }
        "HRDN-7222" => {
            x.advice =
                Some("Matters on shared servers. On a computer you build software on, you need the compilers.".into())
        }
        "PRNT-2308" => {
            x.advice = Some(
                "CUPS, the printing service, can be reached from the network. If you don't share a printer with \
                 other computers, that's normal and nothing needs changing."
                    .into(),
            )
        }
        _ => {}
    }
    // Modules: "Determine if protocol 'dccp' is really needed on this system",
    // or FireWire for STRG-1846.
    if test == "NETW-3200" || test == "STRG-1846" {
        for (module, what) in MODULES {
            let wanted =
                if test == "STRG-1846" { *module == "firewire-core" } else { text.contains(&format!("'{module}'")) };
            if wanted && !blacklisted(module) {
                x.actions.push(action(
                    "blacklist",
                    Some(module.to_string()),
                    format!("Turn off {what}"),
                    format!(
                        "Stops Linux loading the {module} module, for {what}. Takes effect after a restart. \
                         Leave it if you use {what}."
                    ),
                    format!("write 'install {module} /bin/false' to {MODPROBE_FILE}"),
                ));
            }
        }
    }
    x
}

fn sysctls(report: &str, x: &mut Extra) {
    // details[]=KRNL-6000|sysctl|desc:…;field:key;prefval:0;value:1;|
    let mut safe = Vec::new();
    for line in report.lines() {
        let Some(rest) = line.strip_prefix("details[]=KRNL-6000|sysctl|") else { continue };
        let get = |name: &str| {
            rest.split(';').find_map(|p| p.strip_prefix(name).and_then(|v| v.strip_prefix(':'))).map(str::to_string)
        };
        let (Some(key), Some(want), Some(now)) = (get("field"), get("prefval"), get("value")) else { continue };
        let desc = get("desc").unwrap_or_default();
        x.found.push(format!("{key} is {now}; Lynis prefers {want} ({})", desc.to_lowercase()));
        if let Some((_, why)) = SYSCTL_NO.iter().find(|(k, _)| *k == key) {
            x.found.push(format!("  Not offered: setting {key} {why}"));
            continue;
        }
        if key == "kernel.unprivileged_bpf_disabled" && now == "2" {
            x.found
                .push("  Already as strict: 2 also blocks it, and lets only an administrator turn it back on".into());
            continue;
        }
        let Some((_, value, cost)) = SYSCTLS.iter().find(|(k, v, _)| *k == key && *v == want) else { continue };
        let mut explain = if cost.is_empty() {
            format!("Sets {key} to {value}, now and at every start-up.")
        } else {
            format!("Sets {key} to {value}, now and at every start-up. {cost}")
        };
        let ufw = ufw_value(&ufw_text(), &key).filter(|v| v != value);
        if let Some(theirs) = &ufw {
            explain.push_str(&format!(
                " ufw, the firewall, sets it to {theirs} each time it starts, so its setting in {UFW_SYSCTL} changes too."
            ));
        }
        x.actions.push(action(
            "sysctl",
            Some(format!("{key}={value}")),
            format!("Set {key} to {value}"),
            explain,
            if ufw.is_some() {
                format!("sysctl -w {key}={value}, saved in {SYSCTL_FILE} and {UFW_SYSCTL}")
            } else {
                format!("sysctl -w {key}={value}, saved in {SYSCTL_FILE}")
            },
        ));
        if cost.is_empty() {
            safe.push(format!("{key}={value}"));
        }
    }
    if safe.len() > 1 {
        x.actions.insert(
            0,
            action(
                "sysctl",
                Some(safe.join(",")),
                format!("Apply the {} settings with no downside", safe.len()),
                format!("Sets {}, now and at every start-up.", safe.join(", ")),
                format!("sysctl -w …, saved in {SYSCTL_FILE}"),
            ),
        );
    }
}

fn octal(mode: &str) -> bool {
    (3..=4).contains(&mode.len()) && mode.chars().all(|c| ('0'..='7').contains(&c))
}

fn valid_unit(unit: &str) -> bool {
    !unit.is_empty()
        && !unit.starts_with('-')
        && unit.chars().all(|c| c.is_ascii_alphanumeric() || "-_.@:\\".contains(c))
}

fn blacklisted(module: &str) -> bool {
    fs::read_to_string(MODPROBE_FILE)
        .is_ok_and(|t| t.lines().any(|l| l.trim() == format!("install {module} /bin/false")))
}

fn changes_file() -> std::path::PathBuf {
    config_dir().join("security-changes.json")
}

#[tauri::command]
pub fn security_changes() -> Vec<Change> {
    fs::read_to_string(changes_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_changes(changes: &[Change]) {
    if fs::create_dir_all(config_dir()).is_ok()
        && let Ok(text) = serde_json::to_string_pretty(changes)
    {
        let _ = fs::write(changes_file(), text);
    }
}

/// Applies a fix, or runs a read-only check, offered with a finding.
#[tauri::command]
pub async fn security_fix(fix: String, target: Option<String>) -> Result<FixResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = apply(&fix, target.as_deref())?;
        if let Some((label, undo_fix, undo_target)) = result.undo {
            let mut changes = security_changes();
            changes.retain(|c| !(c.fix == fix && c.target == target));
            changes.push(Change {
                fix,
                target,
                label,
                undo_fix,
                undo_target,
                time: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
            });
            save_changes(&changes);
        }
        Ok(FixResult { message: result.message, output: result.output })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Undoes a change made from the page, by its position in the list.
#[tauri::command]
pub async fn security_undo(index: usize) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut changes = security_changes();
        let Some(change) = changes.get(index).cloned() else { return Err("That change is no longer listed.".into()) };
        let result = apply(&change.undo_fix, change.undo_target.as_deref())?;
        changes.remove(index);
        save_changes(&changes);
        Ok(result.message)
    })
    .await
    .map_err(|e| e.to_string())?
}

struct Applied {
    message: String,
    output: Option<String>,
    /// For changes: a label, and the fix and target that undo it.
    undo: Option<(String, String, Option<String>)>,
}

fn done(message: impl Into<String>, undo: Option<(String, String, Option<String>)>) -> Result<Applied, String> {
    Ok(Applied { message: message.into(), output: None, undo })
}

fn apply(fix: &str, target: Option<&str>) -> Result<Applied, String> {
    let target = target.unwrap_or_default();
    match fix {
        "sysctl" | "sysctl-restore" => {
            let mut pairs = Vec::new();
            // Restoring may also put back ufw's own values, as "ufw:key=value".
            let mut ufw_back = Vec::new();
            for pair in target.split(',').filter(|p| !p.is_empty()) {
                if let Some(theirs) = pair.strip_prefix("ufw:") {
                    let (key, value) = theirs.split_once('=').ok_or("Not a setting")?;
                    if fix != "sysctl-restore"
                        || !SYSCTLS.iter().any(|(k, _, _)| *k == key)
                        || value.is_empty()
                        || !value.chars().all(|c| c.is_ascii_digit())
                    {
                        return Err(format!("Open Task Manager doesn't change {key}."));
                    }
                    ufw_back.push((key.to_string(), value.to_string()));
                    continue;
                }
                let (key, value) = pair.split_once('=').ok_or("Not a setting")?;
                let allowed = if fix == "sysctl" {
                    SYSCTLS.iter().any(|(k, v, _)| *k == key && *v == value)
                } else {
                    SYSCTLS.iter().any(|(k, _, _)| *k == key)
                        && !value.is_empty()
                        && value.chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' ' || c == '\t')
                };
                if !allowed {
                    return Err(format!("Open Task Manager doesn't change {key}."));
                }
                pairs.push((key.to_string(), value.to_string()));
            }
            // The file keeps the settings made here; restoring takes them out.
            let mut kept: Vec<(String, String)> = fs::read_to_string(SYSCTL_FILE)
                .unwrap_or_default()
                .lines()
                .filter_map(|l| l.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
                .filter(|(k, _)| !k.starts_with('#'))
                .collect();
            let before: Vec<(String, String)> =
                pairs.iter().map(|(k, _)| (k.clone(), sysctl_now(k).unwrap_or_default())).collect();
            for (key, value) in &pairs {
                kept.retain(|(k, _)| k != key);
                if fix == "sysctl" {
                    kept.push((key.clone(), value.clone()));
                }
            }
            let mut text = String::from(
                "# Kernel settings made by Open Task Manager's Security audit.\n# Undo them there, or delete this file and restart.\n",
            );
            for (k, v) in &kept {
                text.push_str(&format!("{k} = {v}\n"));
            }
            let sets: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}={v}")).collect();
            // ufw's file: settings made here go in it too where it sets them
            // otherwise; restoring puts its values back.
            let ufw_old = ufw_text();
            let mut ufw_new = ufw_old.clone();
            let mut ufw_undo = Vec::new();
            if fix == "sysctl" {
                for (key, value) in &pairs {
                    if let Some(theirs) = ufw_value(&ufw_new, key).filter(|v| v != value) {
                        ufw_new = ufw_set(&ufw_new, key, value);
                        ufw_undo.push(format!("ufw:{key}={theirs}"));
                    }
                }
            } else {
                for (key, value) in &ufw_back {
                    ufw_new = ufw_set(&ufw_new, key, value);
                }
            }
            // The new ufw file goes through a copy only this user can reach;
            // root then writes it over ufw's, keeping its owner and permissions.
            let staged = if ufw_new != ufw_old { Some(stage(&ufw_new)?) } else { None };
            let args: Vec<String> = [
                "/bin/sh",
                "-c",
                r#"f="$1"; m="$2"; u="$3"; shift 3
                   if [ "$m" = write ]; then cat > "$f" || exit 1; else rm -f "$f"; fi
                   if [ -n "$u" ]; then cat "$u" > /etc/ufw/sysctl.conf || exit 1; fi
                   if [ $# -gt 0 ]; then sysctl -q -w "$@"; fi"#,
                "sh",
                SYSCTL_FILE,
                if kept.is_empty() { "remove" } else { "write" },
            ]
            .into_iter()
            .map(String::from)
            .chain([staged.as_ref().map(|s| s.file.to_string_lossy().into_owned()).unwrap_or_default()])
            .chain(sets.iter().cloned())
            .collect();
            let result = admin_with_input(&args, if kept.is_empty() { None } else { Some(&text) });
            drop(staged);
            result?;
            if fix == "sysctl" {
                let undo: Vec<String> = before
                    .iter()
                    .filter(|(_, v)| !v.is_empty())
                    .map(|(k, v)| format!("{k}={v}"))
                    .chain(ufw_undo)
                    .collect();
                let label =
                    if pairs.len() == 1 { format!("Set {}", sets[0]) } else { format!("Set {}", sets.join(", ")) };
                done(
                    "Done. The setting applies now and after restarts.",
                    Some((label, "sysctl-restore".into(), Some(undo.join(",")))),
                )
            } else {
                done("Put back as it was.", None)
            }
        }
        "chmod" => {
            let mut parts = target.split(':');
            let (Some(path), Some(want), Some(now)) = (parts.next(), parts.next(), parts.next()) else {
                return Err("Not a file".into());
            };
            if !PERM_PATHS.contains(&path) || !octal(want) || !octal(now) {
                return Err(format!("Open Task Manager doesn't change the permissions of {path}."));
            }
            admin_with_input(&["chmod".into(), want.into(), path.into()], None)?;
            done(
                format!("{path} is now {want}."),
                Some((
                    format!("Set {path} to {want} (was {now})"),
                    "chmod".into(),
                    Some(format!("{path}:{now}:{want}")),
                )),
            )
        }
        "coredumps-off" => {
            let script = format!(
                "mkdir -p /etc/systemd/coredump.conf.d /etc/security/limits.d && \
                 printf '%s\\n' '# Made by Open Task Manager' '[Coredump]' 'Storage=none' 'ProcessSizeMax=0' > {COREDUMP_FILE} && \
                 printf '%s\\n' '# Made by Open Task Manager' '* hard core 0' > {LIMITS_FILE}"
            );
            admin_with_input(&["/bin/sh".into(), "-c".into(), script], None)?;
            done(
                "Core dumps are off. The size limit applies from your next login.",
                Some(("Turned off core dumps".into(), "coredumps-on".into(), None)),
            )
        }
        "coredumps-on" => {
            admin_with_input(&["rm".into(), "-f".into(), COREDUMP_FILE.into(), LIMITS_FILE.into()], None)?;
            done("Core dumps are back to how they were.", None)
        }
        "blacklist" | "unblacklist" => {
            let Some((module, what)) = MODULES.iter().find(|(m, _)| *m == target) else {
                return Err(format!("Open Task Manager doesn't turn off {target}."));
            };
            let mut lines: Vec<String> = fs::read_to_string(MODPROBE_FILE)
                .unwrap_or_default()
                .lines()
                .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                .filter(|l| l.split_whitespace().nth(1) != Some(module))
                .map(str::to_string)
                .collect();
            if fix == "blacklist" {
                lines.push(format!("install {module} /bin/false"));
                lines.push(format!("blacklist {module}"));
            }
            write_file(MODPROBE_FILE, &lines, "# Modules turned off by Open Task Manager's Security audit.")?;
            if fix == "blacklist" {
                done(
                    format!("{module} won't load from the next restart."),
                    Some((format!("Turned off {what}"), "unblacklist".into(), Some(module.to_string()))),
                )
            } else {
                done(format!("{module} can load again."), None)
            }
        }
        "service-on" | "service-off" => {
            if !SERVICES.iter().any(|(u, _)| *u == target) {
                return Err(format!("Open Task Manager doesn't manage {target}."));
            }
            let verb = if fix == "service-on" { "enable" } else { "disable" };
            admin_with_input(&["systemctl".into(), verb.into(), "--now".into(), target.into()], None)?;
            if fix == "service-on" {
                done(
                    format!("{target} is running."),
                    Some((format!("Started {target}"), "service-off".into(), Some(target.into()))),
                )
            } else {
                done(format!("{target} is stopped."), None)
            }
        }
        "install" => {
            if target != "arch-audit" {
                return Err(format!("Open Task Manager doesn't install {target} from here."));
            }
            admin_with_input(
                &["pacman".into(), "-S".into(), "--needed".into(), "--noconfirm".into(), "arch-audit".into()],
                None,
            )?;
            done("arch-audit is installed. Run the audit again to update this.", None)
        }
        "arch-audit" => {
            let out = programs::run("arch-audit", &[])?;
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            Ok(Applied {
                message: if text.is_empty() {
                    "No installed package has a known security problem.".into()
                } else {
                    "Packages with known security problems:".into()
                },
                output: (!text.is_empty()).then_some(text),
                undo: None,
            })
        }
        "pwck" => {
            let out = programs::run("pkexec", &["pwck", "-r"])?;
            if out.status.code() == Some(126) {
                return Err("Cancelled.".into());
            }
            let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
                .trim()
                .to_string();
            Ok(Applied {
                message: if text.is_empty() { "pwck found nothing wrong.".into() } else { "What pwck found:".into() },
                output: (!text.is_empty()).then_some(text),
                undo: None,
            })
        }
        "unit-security" => {
            if !valid_unit(target) {
                return Err("Not a service".into());
            }
            let out = programs::run("systemd-analyze", &["security", "--no-pager", target])?;
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            Ok(Applied { message: format!("How {target} is sandboxed:"), output: Some(text), undo: None })
        }
        _ => Err(format!("Unknown fix {fix}")),
    }
}

fn ufw_text() -> String {
    fs::read_to_string(UFW_SYSCTL).unwrap_or_default()
}

/// The value ufw's file sets for a key, written there with slashes
/// ("net/ipv4/conf/all/log_martians=0") or dots.
fn ufw_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| ufw_line(l, key)).map(|(_, v)| v.to_string())
}

fn ufw_line<'a>(line: &'a str, key: &str) -> Option<(&'a str, &'a str)> {
    let l = line.trim();
    if l.starts_with('#') {
        return None;
    }
    let (k, v) = l.split_once('=')?;
    (k.trim().replace('/', ".") == key).then(|| (k, v.trim()))
}

/// ufw's file with a key's value changed where it is set; the rest is kept as it was.
fn ufw_set(text: &str, key: &str, value: &str) -> String {
    let mut out: String = text
        .lines()
        .map(|l| match ufw_line(l, key) {
            Some((k, _)) => format!("{}={value}", k.trim()),
            None => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A file in a folder only this user can open, removed when dropped.
struct Staged {
    dir: std::path::PathBuf,
    file: std::path::PathBuf,
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stage(text: &str) -> Result<Staged, String> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = std::env::temp_dir().join(format!("otm-security-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::DirBuilder::new().mode(0o700).create(&dir).map_err(|e| e.to_string())?;
    let staged = Staged { file: dir.join("sysctl.conf"), dir };
    fs::write(&staged.file, text).map_err(|e| e.to_string())?;
    Ok(staged)
}

/// Writes one of this app's files as administrator; an empty list removes it.
fn write_file(path: &str, lines: &[String], header: &str) -> Result<(), String> {
    if lines.is_empty() {
        return admin_with_input(&["rm".into(), "-f".into(), path.into()], None);
    }
    let text = format!("{header}\n{}\n", lines.join("\n"));
    admin_with_input(&["/bin/sh".into(), "-c".into(), r#"cat > "$1""#.into(), "sh".into(), path.into()], Some(&text))
}

/// Runs a command through pkexec, giving it `input` to read.
fn admin_with_input(args: &[String], input: Option<&str>) -> Result<(), String> {
    if programs::which("pkexec").is_none() {
        return Err(programs::missing("pkexec"));
    }
    let mut child = Command::new("pkexec")
        .args(args)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run pkexec: {e}"))?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    match out.status.code() {
        Some(0) => Ok(()),
        Some(126) => Err("Cancelled.".into()),
        _ => {
            let text = String::from_utf8_lossy(&out.stderr);
            Err(text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("It didn't work.").to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sysctls() {
        let report = "details[]=KRNL-6000|sysctl|desc:Disable magic SysRQ;field:kernel.sysrq;prefval:0;value:16;|\n\
                      details[]=KRNL-6000|sysctl|desc:Restrict FIFO;field:fs.protected_fifos;prefval:2;value:1;|\n\
                      details[]=KRNL-6000|sysctl|desc:Restrict regular;field:fs.protected_regular;prefval:2;value:1;|\n\
                      details[]=KRNL-6000|sysctl|desc:Modules;field:kernel.modules_disabled;prefval:1;value:0;|\n\
                      details[]=KRNL-6000|sysctl|desc:BPF;field:kernel.unprivileged_bpf_disabled;prefval:1;value:2;|\n";
        let x = extra("KRNL-6000", "", report, "");
        let targets: Vec<_> = x.actions.iter().map(|a| a.target.clone().unwrap()).collect();
        assert_eq!(
            targets,
            [
                "fs.protected_fifos=2,fs.protected_regular=2",
                "kernel.sysrq=0",
                "fs.protected_fifos=2",
                "fs.protected_regular=2"
            ]
        );
        assert!(x.found.iter().any(|f| f.contains("Not offered: setting kernel.modules_disabled")));
        assert!(x.found.iter().any(|f| f.contains("Already as strict")));
    }

    #[test]
    fn reads_permissions_and_services() {
        let log = "2026-11-04 11:56:28 Performing test ID FILE-7524 (Perform file permissions check)\n\
                   2026-11-04 11:56:28 Outcome: permissions of file /etc/ssh/sshd_config are not matching expected value (644 != 600)\n\
                   2026-11-04 11:56:28 Outcome: permissions of file /etc/evil are not matching expected value (644 != 600)\n\
                   2026-11-04 11:56:29 Performing test ID BOOT-5264 (Run systemd-analyze security)\n\
                   2026-11-04 11:56:29 Result: a.service has exposure value 4.2 with predicate 'PROTECTED'\n\
                   2026-11-04 11:56:29 Result: b.service has exposure value 9.6 with predicate 'UNSAFE'\n";
        let x = extra("FILE-7524", "", "", log);
        assert_eq!(x.found.len(), 2);
        assert_eq!(x.actions.len(), 1);
        assert_eq!(x.actions[0].target.as_deref(), Some("/etc/ssh/sshd_config:600:644"));
        let x = extra("BOOT-5264", "", "", log);
        assert_eq!(x.found, ["b.service: 9.6, UNSAFE", "a.service: 4.2, PROTECTED"]);
        assert_eq!(x.actions[0].target.as_deref(), Some("b.service"));
    }

    #[test]
    fn edits_ufw_settings() {
        let text = "# Uncomment this to allow this host to route packets\n\
                    #net/ipv4/ip_forward=1\n\
                    net/ipv4/conf/default/log_martians=0\n\
                    net/ipv4/conf/all/log_martians=0\n";
        assert_eq!(ufw_value(text, "net.ipv4.conf.default.log_martians").as_deref(), Some("0"));
        assert_eq!(ufw_value(text, "net.ipv4.ip_forward"), None);
        let new = ufw_set(text, "net.ipv4.conf.default.log_martians", "1");
        assert!(new.contains("net/ipv4/conf/default/log_martians=1\n"));
        assert!(new.contains("net/ipv4/conf/all/log_martians=0\n"));
        assert!(new.contains("#net/ipv4/ip_forward=1\n"));
        assert_eq!(new.lines().count(), 4);
    }

    #[test]
    fn refuses_what_is_not_listed() {
        assert!(apply("chmod", Some("/etc/shadow:600:640")).is_err());
        assert!(apply("sysctl", Some("kernel.modules_disabled=1")).is_err());
        assert!(apply("sysctl", Some("kernel.sysrq=1")).is_err());
        assert!(apply("blacklist", Some("ext4")).is_err());
        assert!(apply("service-on", Some("sshd.service")).is_err());
        assert!(apply("unit-security", Some("--root=/")).is_err());
        assert!(apply("sysctl", Some("ufw:kernel.sysrq=1")).is_err());
        assert!(apply("sysctl-restore", Some("ufw:kernel.modules_disabled=1")).is_err());
    }
}
