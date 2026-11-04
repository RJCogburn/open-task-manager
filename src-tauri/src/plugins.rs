//! Plugins: pages other people write, installed as folders in
//! `~/.local/share/open-task-manager/plugins/<id>/`, each with a
//! `manifest.json` and a built `index.js`. See docs/plugins.md.
//!
//! A plugin's code runs in the app's window, with the app's access, so none is
//! loaded until the user approves it. Approval covers the plugin as it was:
//! if its manifest or code changes, it must be approved again. The only
//! programs a plugin can run are the ones its manifest declares, and never one
//! that asks for administrator rights.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::logging::{config_dir, xdg};
use crate::programs;

/// The plugin API this version of the app offers. A plugin names the one it
/// was made for, and is refused if they differ.
pub const API_VERSION: u32 = 1;

/// Ids the app's own pages use; a plugin can't take one.
const RESERVED: &[&str] = &[
    "overview",
    "cpu",
    "memory",
    "gpu",
    "disks",
    "network",
    "processes",
    "connections",
    "sensors",
    "space",
    "apps",
    "bench",
    "settings",
    "about",
    "widgets",
];

/// Programs that ask for administrator rights. A plugin that declares one is
/// refused: plugins never get root.
const FORBIDDEN: &[&str] = &["pkexec", "sudo", "su", "doas", "run0"];

fn plugins_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("open-task-manager").join("plugins")
}

fn approvals_file() -> PathBuf {
    config_dir().join("plugins.json")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    id: String,
    name: String,
    version: String,
    api_version: u32,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    /// An SVG path on a 24×24 grid, like the app's own tools use.
    #[serde(default)]
    icon: String,
    #[serde(default)]
    section: String,
    #[serde(default = "default_order")]
    order: i32,
    /// The programs it may run, by a name of its own: `"status": ["upower", "-d"]`.
    #[serde(default)]
    commands: BTreeMap<String, Vec<String>>,
}

fn default_order() -> i32 {
    500
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    id: String,
    name: String,
    version: String,
    description: String,
    author: String,
    icon: String,
    section: &'static str,
    order: i32,
    commands: BTreeMap<String, Vec<String>>,
    /// Why it can't be loaded, if it can't.
    problem: Option<String>,
    approved: bool,
}

/// What approval was given for, per plugin: a fingerprint of its manifest
/// and code at the time.
#[derive(Serialize, Deserialize, Default)]
struct Approvals {
    approved: HashMap<String, String>,
}

fn load_approvals() -> Approvals {
    fs::read_to_string(approvals_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_approvals(a: &Approvals) -> Result<(), String> {
    fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(a).map_err(|e| e.to_string())?;
    fs::write(approvals_file(), text).map_err(|e| e.to_string())
}

/// FNV-1a over the manifest and the code. It only needs to notice changes:
/// anyone able to change a plugin's files can change the approvals file too.
fn fingerprint(dir: &Path) -> Option<String> {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for file in ["manifest.json", "index.js"] {
        for byte in fs::read(dir.join(file)).ok()? {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        // Keeps "ab" + "c" apart from "a" + "bc".
        hash ^= 0xff;
    }
    Some(format!("{hash:016x}"))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-')
}

/// Reads one plugin folder. Problems are reported, not hidden, so the
/// Settings page can say why a plugin isn't showing.
fn read_plugin(dir: &Path, approvals: &Approvals) -> PluginInfo {
    let folder = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let manifest =
        fs::read_to_string(dir.join("manifest.json")).map_err(|_| "It has no manifest.json.".to_string()).and_then(
            |t| serde_json::from_str::<Manifest>(&t).map_err(|e| format!("Its manifest.json can't be read: {e}")),
        );
    let m = match manifest {
        Ok(m) => m,
        Err(problem) => {
            return PluginInfo {
                id: folder.clone(),
                name: folder,
                version: String::new(),
                description: String::new(),
                author: String::new(),
                icon: String::new(),
                section: "tools",
                order: default_order(),
                commands: BTreeMap::new(),
                problem: Some(problem),
                approved: false,
            };
        }
    };
    let problem = if !valid_id(&m.id) {
        Some("Its id must be lowercase letters, digits and dashes.".to_string())
    } else if m.id != folder {
        Some(format!("Its id, \"{}\", must match its folder's name, \"{folder}\".", m.id))
    } else if RESERVED.contains(&m.id.as_str()) {
        Some(format!("Its id, \"{}\", is used by one of the app's own pages.", m.id))
    } else if m.api_version != API_VERSION {
        Some(format!(
            "It was made for plugin API {}; this version of Open Task Manager offers API {API_VERSION}.",
            m.api_version
        ))
    } else if !dir.join("index.js").is_file() {
        Some("It has no index.js.".to_string())
    } else if let Some((name, _)) = m.commands.iter().find(|(_, argv)| argv.is_empty()) {
        Some(format!("Its command \"{name}\" names no program."))
    } else if let Some((name, _)) =
        m.commands.iter().find(|(_, argv)| argv[0].starts_with("./") && argv[0].split('/').any(|c| c == ".."))
    {
        Some(format!("Its command \"{name}\" reaches outside the plugin's folder."))
    } else {
        m.commands.values().map(|argv| program_name(&argv[0])).find(|p| FORBIDDEN.contains(&p.as_str())).map(
            |program| {
                format!("It wants to run {program}, which asks for administrator rights. Plugins can't have those.")
            },
        )
    };
    let approved =
        problem.is_none() && approvals.approved.get(&m.id).is_some_and(|f| Some(f) == fingerprint(dir).as_ref());
    PluginInfo {
        id: m.id,
        name: m.name,
        version: m.version,
        description: m.description,
        author: m.author,
        icon: m.icon,
        section: if m.section == "monitor" { "monitor" } else { "tools" },
        order: m.order,
        commands: m.commands,
        problem,
        approved,
    }
}

fn program_name(program: &str) -> String {
    Path::new(program).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The plugin with this id, if it is installed and loadable.
fn find(id: &str) -> Result<(PathBuf, PluginInfo), String> {
    if !valid_id(id) {
        return Err(format!("There is no plugin called \"{id}\"."));
    }
    let dir = plugins_dir().join(id);
    if !dir.is_dir() {
        return Err(format!("There is no plugin called \"{id}\"."));
    }
    let info = read_plugin(&dir, &load_approvals());
    match &info.problem {
        Some(problem) => Err(problem.clone()),
        None => Ok((dir, info)),
    }
}

#[tauri::command]
pub fn plugins_list() -> Vec<PluginInfo> {
    let approvals = load_approvals();
    let mut list: Vec<PluginInfo> = fs::read_dir(plugins_dir())
        .map(|d| d.flatten().filter(|e| e.path().is_dir()).map(|e| read_plugin(&e.path(), &approvals)).collect())
        .unwrap_or_default();
    list.sort_by_key(|p| p.name.to_lowercase());
    list
}

/// The plugin's code, for the window to load. Only once it is approved.
#[tauri::command]
pub fn plugin_source(id: String) -> Result<String, String> {
    let (dir, info) = find(&id)?;
    if !info.approved {
        return Err(format!("{} hasn't been approved.", info.name));
    }
    fs::read_to_string(dir.join("index.js")).map_err(|e| e.to_string())
}

/// Approves a plugin as it is now, or takes approval back.
#[tauri::command]
pub fn plugin_approve(id: String, approve: bool) -> Result<(), String> {
    let mut approvals = load_approvals();
    if approve {
        let (dir, _) = find(&id)?;
        let print = fingerprint(&dir).ok_or("The plugin's files can't be read.")?;
        approvals.approved.insert(id, print);
    } else {
        approvals.approved.remove(&id);
    }
    save_approvals(&approvals)
}

#[derive(Serialize)]
pub struct RunOutput {
    /// The exit code, or -1 if it was ended by a signal.
    status: i32,
    stdout: String,
    stderr: String,
}

/// Runs one of the commands a plugin's manifest declares, by its name there.
/// A program given as `./script` is the plugin's own, in its folder.
#[tauri::command]
pub async fn plugin_run(id: String, command: String) -> Result<RunOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (dir, info) = find(&id)?;
        if !info.approved {
            return Err(format!("{} hasn't been approved.", info.name));
        }
        let argv =
            info.commands.get(&command).ok_or_else(|| format!("{} declares no command \"{command}\".", info.name))?;
        let out = if let Some(own) = argv[0].strip_prefix("./") {
            Command::new(dir.join(own))
                .args(&argv[1..])
                .current_dir(&dir)
                .stdin(Stdio::null())
                .output()
                .map_err(|e| format!("Could not run {}: {e}", argv[0]))?
        } else {
            let args: Vec<&str> = argv[1..].iter().map(String::as_str).collect();
            programs::run(&argv[0], &args)?
        };
        Ok(RunOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Opens the plugins folder in the file manager, making it first if needed.
#[tauri::command]
pub fn plugins_open_folder() -> Result<(), String> {
    let dir = plugins_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    programs::run("xdg-open", &[&dir.to_string_lossy()]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert!(valid_id("battery"));
        assert!(valid_id("rgb-fans2"));
        assert!(!valid_id("Battery"));
        assert!(!valid_id("../evil"));
        assert!(!valid_id("-x"));
        assert!(!valid_id(""));
    }

    #[test]
    fn forbidden_programs_by_name() {
        assert_eq!(program_name("/usr/bin/pkexec"), "pkexec");
        assert_eq!(program_name("sudo"), "sudo");
    }
}
