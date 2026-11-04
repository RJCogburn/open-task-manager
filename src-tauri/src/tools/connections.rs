//! Open network sockets from iproute2's `ss`, with the process that owns each
//! one and how exposed it is.

use serde::Serialize;

#[cfg(feature = "connections")]
use crate::programs;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub(crate) protocol: String,
    pub(crate) state: String,
    pub(crate) local_address: String,
    pub(crate) local_port: String,
    pub(crate) peer_address: String,
    pub(crate) peer_port: String,
    pub(crate) process: Option<String>,
    pub(crate) pid: Option<u32>,
    /// "network" when other machines can reach a listening socket, "local"
    /// when only this computer can, "outbound" for connections to elsewhere.
    pub(crate) exposure: &'static str,
}

// Network uses this module's parsing without the page's own command.
#[cfg(feature = "connections")]
#[tauri::command]
pub async fn connections() -> Result<Vec<Connection>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let out = programs::run("ss", &["-tunapH"])?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).lines().filter_map(parse_line).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) fn parse_line(line: &str) -> Option<Connection> {
    let mut f = line.split_whitespace();
    let protocol = f.next()?.to_string();
    let state = f.next()?.to_string();
    let (_recv_q, _send_q) = (f.next()?, f.next()?);
    let (local_address, local_port) = split_address(f.next()?);
    let (peer_address, peer_port) = split_address(f.next()?);
    let users = f.collect::<Vec<_>>().join(" ");
    let (process, pid) = parse_users(&users);
    let listening = matches!(state.as_str(), "LISTEN" | "UNCONN");
    let exposure = if !listening {
        "outbound"
    } else if is_loopback(&local_address) {
        "local"
    } else {
        "network"
    };
    Some(Connection { protocol, state, local_address, local_port, peer_address, peer_port, process, pid, exposure })
}

/// "192.168.1.4:443", "[::1]:631", "*:22" or "0.0.0.0%lo:53" → address and port.
fn split_address(s: &str) -> (String, String) {
    let (address, port) = s.rsplit_once(':').unwrap_or((s, ""));
    let address = address.trim_start_matches('[').trim_end_matches(']');
    let address = address.split('%').next().unwrap_or(address);
    (address.to_string(), port.to_string())
}

pub(crate) fn is_loopback(address: &str) -> bool {
    address.starts_with("127.") || address == "::1" || address == "::ffff:127.0.0.1"
}

/// `users:(("brave",pid=3241,fd=35),…)` → the first owner.
fn parse_users(s: &str) -> (Option<String>, Option<u32>) {
    let Some(rest) = s.strip_prefix("users:((\"") else { return (None, None) };
    let name = rest.split('"').next().map(str::to_string);
    let pid = rest.split("pid=").nth(1).and_then(|p| p.split([',', ')']).next()).and_then(|p| p.parse().ok());
    (name, pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_established_udp_line() {
        let c = parse_line("udp ESTAB 0 0 192.168.100.104:34972 104.26.5.68:443 users:((\"brave\",pid=3241,fd=35))")
            .unwrap();
        assert_eq!((c.local_address.as_str(), c.local_port.as_str()), ("192.168.100.104", "34972"));
        assert_eq!((c.peer_address.as_str(), c.peer_port.as_str()), ("104.26.5.68", "443"));
        assert_eq!((c.process.as_deref(), c.pid), (Some("brave"), Some(3241)));
        assert_eq!(c.exposure, "outbound");
    }

    #[test]
    fn tells_local_listeners_from_network_ones() {
        assert_eq!(parse_line("tcp LISTEN 0 4096 [::1]:631 [::]:*").unwrap().exposure, "local");
        assert_eq!(parse_line("tcp LISTEN 0 128 *:22 *:*").unwrap().exposure, "network");
        assert_eq!(parse_line("udp UNCONN 0 0 127.0.0.53%lo:53 0.0.0.0:*").unwrap().exposure, "local");
    }
}
