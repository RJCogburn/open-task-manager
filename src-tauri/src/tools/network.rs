//! What is using the network right now, program by program, and where each
//! program came from: a package, a Flatpak, the home folder, or nowhere the
//! system knows about.
//!
//! Speeds come from the byte counters the kernel keeps for every TCP socket
//! (shown by `ss -i`). UDP sockets have no counters, so they are counted but
//! not measured; browsers send some of their traffic that way (QUIC).

use std::collections::HashMap;
use std::ffi::CStr;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::programs;
use crate::tools::connections::{is_loopback, parse_line};

/// Bytes received and sent per socket.
type Counters = HashMap<String, (u64, u64)>;

#[derive(Default)]
pub struct NetState {
    /// Byte counters per socket at the last look, to turn into speeds.
    prev: Mutex<Option<(Instant, Counters)>>,
    /// Where each program file came from; checking asks the package manager.
    origins: Mutex<HashMap<String, Origin>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Origin {
    /// package, flatpak, snap, appimage, home, manual, elsewhere, unpackaged,
    /// temporary, deleted, hidden or unchecked.
    kind: &'static str,
    detail: String,
    suspicious: bool,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    rx: f64,
    tx: f64,
    connections: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetProcess {
    pid: Option<u32>,
    name: String,
    exe: Option<String>,
    by_interface: HashMap<String, Usage>,
    /// Sockets that have no byte counters (UDP).
    unmeasured: u32,
    /// Listening sockets other computers can reach.
    listening: u32,
    remotes: Vec<String>,
    origin: Origin,
}

#[tauri::command]
pub async fn net_activity(state: State<'_, AppState>) -> Result<Vec<NetProcess>, String> {
    let net = state.net.clone();
    tauri::async_runtime::spawn_blocking(move || activity(&net)).await.map_err(|e| e.to_string())?
}

fn activity(net: &NetState) -> Result<Vec<NetProcess>, String> {
    let out = programs::run("ss", &["-tuanpiH"])?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout);

    // A socket's line, then for TCP an indented line of counters.
    let mut sockets: Vec<(crate::tools::connections::Connection, Option<(u64, u64)>)> = Vec::new();
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            if let Some(last) = sockets.last_mut() {
                last.1 = counters(line);
            }
        } else if let Some(c) = parse_line(line) {
            sockets.push((c, None));
        }
    }

    let addresses = interface_addresses();
    let now = Instant::now();
    let mut prev = net.prev.lock().unwrap();
    let dt = prev.as_ref().map(|(t, _)| now.duration_since(*t).as_secs_f64().max(0.001));
    let mut seen = HashMap::new();
    let mut procs: HashMap<Option<u32>, NetProcess> = HashMap::new();

    for (c, bytes) in &sockets {
        let entry = procs.entry(c.pid).or_insert_with(|| NetProcess {
            pid: c.pid,
            name: c.process.clone().unwrap_or_else(|| "System or another user".into()),
            exe: None,
            by_interface: HashMap::new(),
            unmeasured: 0,
            listening: 0,
            remotes: Vec::new(),
            origin: Origin { kind: "hidden", detail: String::new(), suspicious: false },
        });
        if matches!(c.state.as_str(), "LISTEN" | "UNCONN") {
            if c.exposure == "network" {
                entry.listening += 1;
            }
            continue;
        }
        let iface = interface_of(&c.local_address, &addresses);
        let usage = entry.by_interface.entry(iface).or_default();
        usage.connections += 1;
        match bytes {
            Some((rx, tx)) => {
                let key =
                    format!("{} {}:{} {}:{}", c.protocol, c.local_address, c.local_port, c.peer_address, c.peer_port);
                if let Some(dt) = dt {
                    // A socket opened since the last look moved all its bytes since then.
                    let (prx, ptx) = prev.as_ref().and_then(|(_, m)| m.get(&key).copied()).unwrap_or((0, 0));
                    usage.rx += rx.saturating_sub(prx) as f64 / dt;
                    usage.tx += tx.saturating_sub(ptx) as f64 / dt;
                }
                seen.insert(key, (*rx, *tx));
            }
            None => entry.unmeasured += 1,
        }
        if !is_loopback(&c.peer_address) && entry.remotes.len() < 8 && !entry.remotes.contains(&c.peer_address) {
            entry.remotes.push(c.peer_address.clone());
        }
    }
    *prev = Some((now, seen));
    drop(prev);

    let mut list: Vec<NetProcess> = procs.into_values().collect();
    for p in &mut list {
        if let Some(pid) = p.pid {
            let (exe, origin) = origin_of(net, pid);
            p.exe = exe;
            p.origin = origin;
        } else {
            p.origin.detail = "Linux shows who owns these connections only to administrators.".into();
        }
    }
    Ok(list)
}

/// `bytes_sent:1234 … bytes_received:5678` → (received, sent).
fn counters(line: &str) -> Option<(u64, u64)> {
    let field =
        |name: &str| line.split_whitespace().find_map(|f| f.strip_prefix(name)?.strip_prefix(':')?.parse::<u64>().ok());
    let (rx, tx) = (field("bytes_received"), field("bytes_sent"));
    (rx.is_some() || tx.is_some()).then(|| (rx.unwrap_or(0), tx.unwrap_or(0)))
}

fn interface_of(local: &str, addresses: &HashMap<IpAddr, String>) -> String {
    let ip = local.parse::<IpAddr>().ok().map(|ip| match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    });
    match ip.and_then(|ip| addresses.get(&ip)) {
        Some(name) => name.clone(),
        None if is_loopback(local) => "lo".into(),
        None => "other".into(),
    }
}

/// Every address on every interface, from getifaddrs.
fn interface_addresses() -> HashMap<IpAddr, String> {
    let mut map = HashMap::new();
    // SAFETY: getifaddrs hands back a linked list that is read here and then
    // freed once; nothing keeps pointers into it.
    unsafe {
        let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut list) != 0 {
            return map;
        }
        let mut cur = list;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_addr.is_null() {
                let name = CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned();
                match i32::from((*ifa.ifa_addr).sa_family) {
                    libc::AF_INET => {
                        let sa = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                        map.insert(IpAddr::V4(Ipv4Addr::from(u32::from_be(sa.sin_addr.s_addr))), name);
                    }
                    libc::AF_INET6 => {
                        let sa = &*(ifa.ifa_addr as *const libc::sockaddr_in6);
                        map.insert(IpAddr::V6(Ipv6Addr::from(sa.sin6_addr.s6_addr)), name);
                    }
                    _ => {}
                }
            }
            cur = ifa.ifa_next;
        }
        libc::freeifaddrs(list);
    }
    map
}

/// The program file a process runs and where it came from.
fn origin_of(net: &NetState, pid: u32) -> (Option<String>, Origin) {
    let proc_dir = Path::new("/proc").join(pid.to_string());
    let Ok(link) = fs::read_link(proc_dir.join("exe")) else {
        return (
            None,
            Origin {
                kind: "hidden",
                detail: "Runs as another user, so only an administrator can see its program file.".into(),
                suspicious: false,
            },
        );
    };
    let link = link.to_string_lossy().into_owned();
    let (exe, deleted) = match link.strip_suffix(" (deleted)") {
        Some(path) => (path.to_string(), true),
        None => (link, false),
    };

    // Flatpak apps see their own filesystem; the sandbox says which app it is.
    if let Ok(info) = fs::read_to_string(proc_dir.join("root/.flatpak-info")) {
        let id = info.lines().find_map(|l| l.strip_prefix("name=")).unwrap_or("unknown app");
        return (Some(exe), Origin { kind: "flatpak", detail: format!("Flatpak app {id}"), suspicious: false });
    }
    if deleted {
        return (
            Some(exe.clone()),
            if Path::new(&exe).exists() {
                Origin {
                    kind: "deleted",
                    detail: "Its program file was replaced after it started, most likely by an update. Restarting it runs the new version.".into(),
                    suspicious: false,
                }
            } else {
                Origin {
                    kind: "deleted",
                    detail: "Its program file was deleted after it started. Uninstalled programs can do this, but so does malware hiding its tracks.".into(),
                    suspicious: true,
                }
            },
        );
    }

    let mut cache = net.origins.lock().unwrap();
    let origin = cache.entry(exe.clone()).or_insert_with(|| classify(&exe)).clone();
    (Some(exe), origin)
}

fn classify(exe: &str) -> Origin {
    let home = std::env::var("HOME").unwrap_or_default();
    let origin = |kind, detail: &str, suspicious| Origin { kind, detail: detail.to_string(), suspicious };
    if exe.starts_with("/snap/") {
        return origin("snap", "Snap package", false);
    }
    if exe.starts_with("/tmp/.mount_") {
        return origin("appimage", "AppImage, a self-contained program file you downloaded", false);
    }
    if ["/tmp/", "/var/tmp/", "/dev/shm/"].iter().any(|d| exe.starts_with(d)) {
        return origin("temporary", "Runs from a temporary folder, which installed programs don't do.", true);
    }
    if let Some(package) = package_owner(exe) {
        return Origin { kind: "package", detail: package, suspicious: false };
    }
    if !home.is_empty() && exe.starts_with(&format!("{home}/")) {
        return origin("home", "Lives in your home folder, not installed by the package manager.", false);
    }
    if exe.starts_with("/opt/") || exe.starts_with("/usr/local/") {
        return origin("manual", "Installed by hand, not by the package manager.", false);
    }
    if package_manager().is_none() {
        return origin("unchecked", "There is no supported package manager to check it against.", false);
    }
    // System folders hold only what packages put there.
    if ["/usr/", "/bin/", "/sbin/", "/lib", "/etc/", "/var/", "/root/", "/boot/"].iter().any(|d| exe.starts_with(d)) {
        return origin("unpackaged", "Sits in a system folder, but no installed package owns it.", true);
    }
    origin("elsewhere", "Not installed by the package manager.", false)
}

fn package_manager() -> Option<&'static str> {
    ["pacman", "dpkg-query", "rpm"].into_iter().find(|p| programs::which(p).is_some())
}

/// The package that installed a file, as "name version".
pub(crate) fn package_owner(path: &str) -> Option<String> {
    let text = |program: &str, args: &[&str]| {
        programs::run(program, args)
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    match package_manager()? {
        // "/usr/bin/curl is owned by curl 8.10.1-2"
        "pacman" => text("pacman", &["-Qo", path])?.split(" is owned by ").nth(1).map(str::to_string),
        "dpkg-query" => {
            // dpkg records /bin paths on systems where /bin is now /usr/bin.
            let alt = path.strip_prefix("/usr").unwrap_or(path);
            let line = text("dpkg-query", &["-S", path]).or_else(|| text("dpkg-query", &["-S", alt]))?;
            let package = line.split(':').next()?.trim().to_string();
            let version = text("dpkg-query", &["-W", "-f=${Version}", &package]).unwrap_or_default();
            Some(format!("{package} {version}").trim().to_string())
        }
        _ => {
            text("rpm", &["-qf", "--qf", "%{NAME} %{VERSION}-%{RELEASE}\\n", path])?.lines().next().map(str::to_string)
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSource {
    pid: u32,
    name: String,
    exe: Option<String>,
    folder: Option<String>,
    command: String,
    cwd: Option<String>,
    user: String,
    parent: Option<String>,
    /// Seconds since 1970.
    started: u64,
    origin: Origin,
}

/// Where a process's program lives and who started it.
#[tauri::command]
pub async fn process_source(state: State<'_, AppState>, pid: u32) -> Result<ProcessSource, String> {
    let (name, command, cwd, user, parent, started) = {
        let s = state.shared.lock().unwrap();
        let p = s.sys.process(sysinfo::Pid::from_u32(pid)).ok_or("That process has exited.")?;
        let parent = p.parent().and_then(|pp| {
            s.sys.process(pp).map(|pp_proc| format!("{} ({})", pp_proc.name().to_string_lossy(), pp.as_u32()))
        });
        (
            p.name().to_string_lossy().into_owned(),
            p.cmd().iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" "),
            p.cwd().map(|c| c.to_string_lossy().into_owned()),
            p.user_id().and_then(|u| s.users.get_user_by_id(u)).map(|u| u.name().to_string()).unwrap_or_default(),
            parent,
            p.start_time(),
        )
    };
    let net = state.net.clone();
    let (exe, origin) =
        tauri::async_runtime::spawn_blocking(move || origin_of(&net, pid)).await.map_err(|e| e.to_string())?;
    Ok(ProcessSource {
        pid,
        name,
        folder: exe.as_deref().and_then(|e| Path::new(e).parent()).map(|p| p.to_string_lossy().into_owned()),
        exe,
        command,
        cwd,
        user,
        parent,
        started,
        origin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tcp_counters() {
        let line = "\t cubic wscale:7,7 rto:204 bytes_sent:140069 bytes_acked:140070 bytes_received:32811 segs_out:219";
        assert_eq!(counters(line), Some((32811, 140069)));
        assert_eq!(counters("\t cubic rto:204"), None);
    }

    #[test]
    fn flags_programs_running_from_temporary_folders() {
        assert!(classify("/tmp/x/miner").suspicious);
        assert!(!classify("/tmp/.mount_abc123/AppRun").suspicious);
        assert!(!classify("/run/media/someone/Drive/tool/python3").suspicious);
    }
}
