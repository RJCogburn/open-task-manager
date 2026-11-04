//! The Security audit page: runs Lynis (CISOfy's open source auditing tool)
//! and reads its report back. Lynis only reads; this app offers no fixes for
//! what it finds, because security settings changed blindly can lock people
//! out. Each finding carries the check's own description from Lynis's source,
//! and a link to its code on GitHub, instead.
//!
//! Without administrator rights Lynis skips the checks that need them; with
//! them it runs through pkexec, which asks for the password.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::security_fix::{self, Action};
use crate::logging::config_dir;
use crate::programs;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Lynis's test id, such as "SSH-7408".
    test: String,
    text: String,
    details: Option<String>,
    solution: Option<String>,
    /// Advice mostly meant for servers rather than a desktop.
    server: bool,
    /// A short tag and a sentence about what the finding means on this computer.
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    note_text: Option<String>,
    /// Lynis's own description of the check, from its source.
    #[serde(default)]
    about: Option<About>,
    /// What Lynis found behind the finding, from its log and report.
    #[serde(default)]
    found: Vec<String>,
    /// Why it's best left as it is, where it is.
    #[serde(default)]
    advice: Option<String>,
    #[serde(default)]
    actions: Vec<Action>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct About {
    description: String,
    notes: Option<String>,
    /// The check's code on GitHub, at the installed version.
    url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    /// Unix time in milliseconds.
    time: u64,
    admin: bool,
    version: Option<String>,
    /// Lynis's hardening index, 0 to 100.
    score: Option<u32>,
    tests: Option<u32>,
    warnings: Vec<Finding>,
    suggestions: Vec<Finding>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    last: Option<Audit>,
    /// Earlier scores, oldest first: (time, admin, score).
    scores: Vec<(u64, bool, u32)>,
}

/// The audit running now, so it can be cancelled.
static RUNNING: Mutex<Option<Child>> = Mutex::new(None);

fn saved_file() -> PathBuf {
    config_dir().join("security-audit.json")
}

/// The last audit and earlier scores.
#[tauri::command]
pub fn security_last() -> Saved {
    fs::read_to_string(saved_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Runs Lynis and returns what it found. Takes one to three minutes.
#[tauri::command]
pub async fn security_audit(admin: bool) -> Result<Audit, String> {
    tauri::async_runtime::spawn_blocking(move || audit(admin)).await.map_err(|e| e.to_string())?
}

/// Stops an audit run without administrator rights. One run through pkexec
/// belongs to root and finishes on its own.
#[tauri::command]
pub fn security_cancel() {
    if let Some(child) = RUNNING.lock().unwrap().as_mut() {
        let _ = child.kill();
    }
}

fn audit(admin: bool) -> Result<Audit, String> {
    let Some(lynis) = programs::which("lynis") else { return Err(programs::missing("lynis")) };
    if RUNNING.lock().unwrap().is_some() {
        return Err("An audit is already running.".into());
    }
    if admin && programs::which("pkexec").is_none() {
        return Err(programs::missing("pkexec"));
    }
    // A folder only this user can open, for the report and Lynis's log.
    let dir = std::env::temp_dir().join(format!("otm-lynis-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).map_err(|e| format!("Could not make a folder for the report: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    let report = dir.join("report.dat");
    let log = dir.join("lynis.log");
    let result = run(&lynis, admin, &dir, &report, &log).and_then(|()| {
        let text = fs::read_to_string(&report).map_err(|_| "Lynis finished without writing a report.".to_string())?;
        let log_text = fs::read_to_string(&log).unwrap_or_default();
        let mut audit = parse(&text, admin, &Local::read(&lynis));
        for f in audit.warnings.iter_mut().chain(audit.suggestions.iter_mut()) {
            let x = security_fix::extra(&f.test, &f.text, &text, &log_text);
            f.found = x.found;
            f.advice = x.advice;
            f.actions = x.actions;
        }
        Ok(audit)
    });
    let _ = fs::remove_dir_all(&dir);
    let audit = result?;
    save(&audit);
    Ok(audit)
}

fn run(lynis: &Path, admin: bool, dir: &Path, report: &Path, log: &Path) -> Result<(), String> {
    let errors = dir.join("errors.txt");
    let errors_file = fs::File::create(&errors).map_err(|e| e.to_string())?;
    let (report_s, log_s) = (report.to_string_lossy(), log.to_string_lossy());
    let lynis_s = lynis.to_string_lossy();
    let audit_args = ["audit", "system", "--quick", "--no-colors", "--report-file", &report_s, "--logfile", &log_s];
    let mut command = if admin {
        // Root writes the report; making it readable afterwards lets this
        // app read it. The folder around it stays this user's alone.
        let mut c = Command::new("pkexec");
        c.args(["/bin/sh", "-c", r#"l="$1"; shift; "$l" "$@"; s=$?; chmod 644 "$6" "$8" 2>/dev/null; exit $s"#, "sh"]);
        c.arg(lynis_s.as_ref()).args(audit_args);
        c
    } else {
        let mut c = Command::new(lynis);
        c.args(audit_args);
        c
    };
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(errors_file)
        .spawn()
        .map_err(|e| format!("Could not start Lynis: {e}"))?;
    *RUNNING.lock().unwrap() = Some(child);
    // Wait without holding the lock, so Cancel can reach the child.
    let status = loop {
        {
            let mut running = RUNNING.lock().unwrap();
            let Some(child) = running.as_mut() else { return Err("Stopped.".into()) };
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                running.take();
                break status;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    };
    let stderr = fs::read_to_string(&errors).unwrap_or_default();
    if admin && status.code() == Some(126) {
        return Err("Cancelled.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if status.signal().is_some() {
            return Err("Stopped.".into());
        }
    }
    // Lynis exits non-zero when it finds warnings; a report means it ran.
    if !report.exists() {
        let why = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no report was written");
        return Err(format!("Lynis did not finish: {why}"));
    }
    Ok(())
}

fn save(audit: &Audit) {
    let mut saved = security_last();
    if let Some(score) = audit.score {
        saved.scores.push((audit.time, audit.admin, score));
        let excess = saved.scores.len().saturating_sub(50);
        saved.scores.drain(..excess);
    }
    saved.last = Some(audit.clone());
    if fs::create_dir_all(config_dir()).is_ok()
        && let Ok(text) = serde_json::to_string(&saved)
    {
        let _ = fs::write(saved_file(), text);
    }
}

/// Tests whose advice is mostly for servers: separate partitions, banners,
/// process accounting, remote logging and the like. Still shown, but marked.
const SERVER: [&str; 22] = [
    "ACCT-9622",
    "ACCT-9626",
    "ACCT-9628",
    "ACCT-9630",
    "BANN-7126",
    "BANN-7130",
    "FILE-6310",
    "FINT-4350",
    "HRDN-7222",
    "HRDN-7230",
    "LOGG-2154",
    "LOGG-2190",
    "MAIL-8818",
    "NAME-4028",
    "NAME-4404",
    "NETW-3200",
    "PKGS-7370",
    "SSH-7408",
    "TIME-3185",
    "TOOL-5002",
    "USB-1000",
    "STRG-1846",
];

/// Reads Lynis's report: `key=value` lines, with `warning[]` and
/// `suggestion[]` holding "TEST|text|details|solution|".
fn parse(text: &str, admin: bool, local: &Local) -> Audit {
    let mut audit = Audit {
        time: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
        admin,
        version: None,
        score: None,
        tests: None,
        warnings: Vec::new(),
        suggestions: Vec::new(),
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        match key {
            "lynis_version" => audit.version = Some(value.to_string()),
            "hardening_index" => audit.score = value.trim().parse().ok(),
            "lynis_tests_done" => audit.tests = value.trim().parse().ok(),
            "warning[]" | "suggestion[]" => {
                if let Some(f) = finding(value, local) {
                    let list = if key == "warning[]" { &mut audit.warnings } else { &mut audit.suggestions };
                    // Lynis sometimes repeats a finding; once is enough.
                    if !list.iter().any(|x| x.test == f.test && x.text == f.text && x.details == f.details) {
                        list.push(f);
                    }
                }
            }
            _ => {}
        }
    }
    audit
}

/// What this computer has that changes how a finding reads.
#[derive(Default)]
struct Local {
    zram: bool,
    /// Lynis's description of each check, by test id.
    about: std::collections::HashMap<String, About>,
}

impl Local {
    fn read(lynis: &Path) -> Self {
        let mut about = std::collections::HashMap::new();
        if let Some(dir) = include_dir(lynis) {
            let version = lynis_version(lynis);
            for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with("tests_")
                    && let Ok(text) = fs::read_to_string(entry.path())
                {
                    headers(&text, &name, version.as_deref(), &mut about);
                }
            }
        }
        Self { zram: fs::read_to_string("/proc/swaps").is_ok_and(|t| t.contains("/dev/zram")), about }
    }
}

/// Where Lynis keeps its checks: next to the program when run from a
/// download, or where packages put it.
fn include_dir(lynis: &Path) -> Option<PathBuf> {
    let real = fs::canonicalize(lynis).ok();
    let beside = real.as_deref().and_then(Path::parent).map(|d| d.join("include"));
    beside
        .into_iter()
        .chain(
            ["/usr/share/lynis/include", "/usr/local/share/lynis/include", "/usr/local/lynis/include"]
                .map(PathBuf::from),
        )
        .find(|d| d.join("consts").is_file())
}

/// The installed version, as its release tag on GitHub, from the `lynis`
/// script itself.
fn lynis_version(lynis: &Path) -> Option<String> {
    let text = fs::read_to_string(lynis).ok()?;
    let line = text.lines().find(|l| l.trim_start().starts_with("PROGRAM_VERSION="))?;
    let v = line.split_once('=')?.1.trim().trim_matches('"');
    v.chars().all(|c| c.is_ascii_digit() || c == '.').then(|| v.to_string())
}

/// Reads the comment block above each check:
/// `# Test : ID`, `# Description : …`, `# Notes : …` (which may run on).
fn headers(text: &str, file: &str, version: Option<&str>, out: &mut std::collections::HashMap<String, About>) {
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let Some(test) =
            comment(line).and_then(|c| c.strip_prefix("Test")).and_then(|r| r.trim_start().strip_prefix(':'))
        else {
            continue;
        };
        let test = test.trim().to_string();
        let (mut description, mut notes): (Option<String>, Option<String>) = (None, None);
        let mut last_was_notes = false;
        for next in &lines[i + 1..] {
            let Some(c) = comment(next) else { break };
            if let Some(d) = field(c, "Description") {
                description = Some(d);
                last_was_notes = false;
            } else if let Some(n) = field(c, "Notes") {
                notes = Some(n);
                last_was_notes = true;
            } else if last_was_notes && !c.trim().is_empty() && !c.contains(" : ") {
                // A note running on to the next line.
                if let Some(n) = notes.as_mut() {
                    n.push(' ');
                    n.push_str(c.trim());
                }
            } else {
                last_was_notes = false;
            }
        }
        let Some(description) = description else { continue };
        let notes = notes.filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("Applicable to all Unix-like OS"));
        let url = version.map(|v| format!("https://github.com/CISOfy/lynis/blob/{v}/include/{file}#L{}", i + 1));
        out.entry(test).or_insert(About { description, notes, url });
    }
}

/// The text of a `#` comment line.
fn comment(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('#').map(str::trim)
}

/// `Name : value` in a header comment.
fn field(c: &str, name: &str) -> Option<String> {
    let rest = c.strip_prefix(name)?.trim_start().strip_prefix(':')?;
    Some(rest.trim().to_string()).filter(|v| !v.is_empty())
}

/// A note for findings that mean something else on this computer.
fn note(test: &str, local: &Local) -> Option<(&'static str, &'static str)> {
    match test {
        "FILE-6394" if local.zram => Some((
            "Normal with zram",
            "This computer swaps to zram, compressed memory, rather than to a disk. Distributions set a high \
             swappiness with zram on purpose: swapping to compressed memory is fast, so the advice about an SSD \
             doesn't apply here.",
        )),
        _ => None,
    }
}

fn finding(value: &str, local: &Local) -> Option<Finding> {
    let mut parts = value.split('|');
    // "-" means empty; a pointer to Lynis's screen output or log means nothing here, where neither is shown.
    let field = |s: Option<&str>| {
        s.map(str::trim)
            .filter(|s| !s.is_empty() && *s != "-" && !s.eq_ignore_ascii_case("See screen output or log file"))
            .map(str::to_string)
    };
    let test = field(parts.next())?;
    let text = field(parts.next())?;
    let details = field(parts.next());
    let solution = field(parts.next());
    let server = SERVER.contains(&test.as_str());
    let (note, note_text) = match note(&test, local) {
        Some((tag, text)) => (Some(tag.to_string()), Some(text.to_string())),
        None => (None, None),
    };
    let about = local.about.get(&test).cloned();
    Some(Finding {
        test,
        text,
        details,
        solution,
        server,
        note,
        note_text,
        about,
        found: Vec::new(),
        advice: None,
        actions: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_check_headers() {
        let text = "#\n    # Test        : AUTH-9230\n    # Description : Check password hashing rounds in login.defs\n    \
                    # Notes       : Applicable to all Unix-like OS\n    PREQS_MET=\"NO\"\n#\n    \
                    # Test        : FILE-6394\n    # Description : Determine level of swappiness.\n    \
                    # Notes       : Swapping is slow;\n    #               see the kernel docs\n    Register\n";
        let mut out = std::collections::HashMap::new();
        headers(text, "tests_x", Some("3.1.7"), &mut out);
        let auth = &out["AUTH-9230"];
        assert_eq!(auth.description, "Check password hashing rounds in login.defs");
        assert_eq!(auth.notes, None);
        assert_eq!(auth.url.as_deref(), Some("https://github.com/CISOfy/lynis/blob/3.1.7/include/tests_x#L2"));
        assert_eq!(out["FILE-6394"].notes.as_deref(), Some("Swapping is slow; see the kernel docs"));
    }

    #[test]
    fn reads_a_report() {
        let text = "lynis_version=3.1.7\n\
                    hardening_index=64\n\
                    lynis_tests_done=243\n\
                    warning[]=FIRE-4512|iptables module(s) loaded, but no rules active|-|-|\n\
                    suggestion[]=FILE-6310|To decrease the impact of a full /home file system, place /home on a separate partition|-|-|\n\
                    suggestion[]=SSH-7408|Consider hardening SSH configuration|AllowTcpForwarding (set YES to NO)|-|\n\
                    suggestion[]=SSH-7408|Consider hardening SSH configuration|AllowTcpForwarding (set YES to NO)|-|\n\
                    suggestion[]=KRNL-6000|One or more sysctl values differ from the scan profile|kernel.kptr_restrict (exp: 2)|-|\n\
                    suggestion[]=FILE-6394|vm.swappiness set to: 150 > 60 (default)|-|-|\n\
                    suggestion[]=FILE-7524|Consider restricting file permissions|See screen output or log file|-|\n";
        let a = parse(text, false, &Local { zram: true, ..Default::default() });
        assert_eq!(a.score, Some(64));
        assert_eq!(a.tests, Some(243));
        assert_eq!(a.version.as_deref(), Some("3.1.7"));
        assert_eq!(a.warnings.len(), 1);
        assert_eq!(a.warnings[0].details, None);
        assert_eq!(a.suggestions.len(), 5);
        assert_eq!(a.suggestions[3].note.as_deref(), Some("Normal with zram"));
        assert_eq!(a.suggestions[4].details, None);
        assert!(a.suggestions[0].server);
        assert_eq!(a.suggestions[1].details.as_deref(), Some("AllowTcpForwarding (set YES to NO)"));
        assert!(!a.suggestions[2].server);
    }
}
