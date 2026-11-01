//! The process table and the actions on a process: end, kill and priority.

use std::io;
use std::sync::Mutex;

use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::programs;
use crate::sampler::Shared;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcInfo {
    pid: u32,
    ppid: Option<u32>,
    name: String,
    command: String,
    user: String,
    /// Share of the whole CPU (all cores), 0–100.
    cpu: f32,
    memory: u64,
    disk_read: f64,
    disk_write: f64,
    status: String,
    start_time: u64,
    nice: Option<i32>,
    /// A kernel thread (kworker, ksoftirqd…) rather than a program.
    kernel: bool,
}

#[tauri::command]
pub async fn processes(state: State<'_, AppState>) -> Result<Vec<ProcInfo>, String> {
    // Off the main thread: the first call after a pause waits a moment for fresh readings.
    let shared = state.shared.clone();
    tauri::async_runtime::spawn_blocking(move || list(&shared)).await.map_err(|e| e.to_string())
}

fn list(shared: &Mutex<Shared>) -> Vec<ProcInfo> {
    Shared::want_processes(shared);
    let s = shared.lock().unwrap();
    let cores = s.sys.cpus().len().max(1) as f32;
    let dt = s.elapsed;
    s.sys
        .processes()
        .values()
        // Threads are listed under their process on Linux; skip them here.
        .filter(|p| p.thread_kind() != Some(sysinfo::ThreadKind::Userland))
        .map(|p| {
            let disk = p.disk_usage();
            let command = p.cmd().iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" ");
            ProcInfo {
                pid: p.pid().as_u32(),
                ppid: p.parent().map(|pp| pp.as_u32()),
                name: full_name(p),
                command,
                user: p
                    .user_id()
                    .and_then(|uid| s.users.get_user_by_id(uid))
                    .map(|u| u.name().to_string())
                    .unwrap_or_default(),
                cpu: p.cpu_usage() / cores,
                memory: p.memory(),
                disk_read: disk.read_bytes as f64 / dt,
                disk_write: disk.written_bytes as f64 / dt,
                status: p.status().to_string(),
                start_time: p.start_time(),
                nice: nice_of(p.pid().as_u32()),
                kernel: p.thread_kind() == Some(sysinfo::ThreadKind::Kernel),
            }
        })
        .collect()
}

/// Linux cuts process names to 15 characters ("open-task-manag"); use the
/// program's file name or first argument when it is the longer form of the same name.
fn full_name(p: &sysinfo::Process) -> String {
    let name = p.name().to_string_lossy();
    if name.len() == 15 {
        let candidates = [
            p.exe().and_then(|e| e.file_name()).map(|f| f.to_string_lossy().into_owned()),
            p.cmd().first().and_then(|a| std::path::Path::new(a).file_name()).map(|f| f.to_string_lossy().into_owned()),
        ];
        if let Some(longer) = candidates.into_iter().flatten().find(|c| c.starts_with(name.as_ref())) {
            return longer;
        }
    }
    name.into_owned()
}

fn nice_of(pid: u32) -> Option<i32> {
    // getpriority can legitimately return -1, so errno tells success apart.
    unsafe {
        *libc::__errno_location() = 0;
        let n = libc::getpriority(libc::PRIO_PROCESS, pid as libc::id_t);
        if *libc::__errno_location() != 0 { None } else { Some(n) }
    }
}

/// Errors that the window can offer to retry as administrator start with
/// `needs-admin:`.
fn explain(e: io::Error, what: &str) -> String {
    match e.raw_os_error() {
        Some(libc::EPERM) | Some(libc::EACCES) => {
            format!("needs-admin:{what} needs administrator rights for this process.")
        }
        Some(libc::ESRCH) => "That process has already exited.".to_string(),
        _ => format!("Could not {what}: {e}"),
    }
}

#[tauri::command]
pub fn end_process(pid: u32, force: bool) -> Result<(), String> {
    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    if unsafe { libc::kill(pid as libc::pid_t, signal) } == 0 {
        Ok(())
    } else {
        Err(explain(io::Error::last_os_error(), if force { "Killing it" } else { "Ending it" }))
    }
}

#[tauri::command]
pub fn set_priority(pid: u32, nice: i32) -> Result<(), String> {
    let nice = nice.clamp(-20, 19);
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) } == 0 {
        Ok(())
    } else {
        Err(explain(io::Error::last_os_error(), "Changing its priority"))
    }
}

/// The same actions through polkit's pkexec, which asks for the password.
#[tauri::command]
pub async fn admin_action(pid: u32, action: String, nice: Option<i32>) -> Result<(), String> {
    let pid = pid.to_string();
    let args: Vec<String> = match action.as_str() {
        "end" => vec!["kill".into(), "-TERM".into(), pid],
        "kill" => vec!["kill".into(), "-KILL".into(), pid],
        "priority" => {
            vec!["renice".into(), "-n".into(), nice.unwrap_or(0).clamp(-20, 19).to_string(), "-p".into(), pid]
        }
        _ => return Err(format!("Unknown action {action}")),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let out = programs::run("pkexec", &args.iter().map(String::as_str).collect::<Vec<_>>())?;
        if out.status.success() {
            Ok(())
        } else if out.status.code() == Some(126) {
            Err("Cancelled.".into())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
