//! Internet speed test against LibreSpeed's public servers
//! (https://librespeed.org), measured the way LibreSpeed's own test does:
//! the nearest server by ping, latency and jitter from ten pings, then
//! download and upload over several connections at once, each for a fixed
//! time after a warm-up.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::NetScore;
use crate::programs;

const SERVERS: &str = "https://librespeed.org/backend-servers/servers.php";
const DOWN_STREAMS: usize = 6;
const UP_STREAMS: usize = 6;
/// Connections ramp up (TCP slow start) before the measured window begins.
const WARMUP: Duration = Duration::from_secs(2);
const WINDOW: Duration = Duration::from_secs(8);
/// LibreSpeed adds this for HTTP and TCP overhead, so results match its own test.
const OVERHEAD: f64 = 1.06;
/// The most each direction moves while measuring, so a fast line doesn't pull gigabytes.
const DOWN_BUDGET: u64 = 400_000_000;
const UP_BUDGET: u64 = 150_000_000;
/// The servers refuse uploads much over 4 MB.
const MAX_CHUNK: usize = 4_000_000;

/// When each uploaded chunk was confirmed, and its HTTP status.
type Confirmations = Arc<Mutex<Vec<(Instant, u32)>>>;

struct Server {
    name: String,
    ping: String,
    down: String,
    up: String,
}

pub fn test() -> Result<NetScore, String> {
    if programs::which("curl").is_none() {
        return Err(programs::missing("curl"));
    }
    let servers = servers()?;
    let server = nearest(&servers)?;
    let (latency, jitter) = ping(server)?;
    let download = download(server)? * OVERHEAD;
    let upload = upload(server)? * OVERHEAD;
    Ok(NetScore {
        latency_ms: latency,
        download_mbps: download * 8.0 / 1e6,
        upload_mbps: upload * 8.0 / 1e6,
        provider: "librespeed",
        jitter_ms: Some(jitter),
        server: Some(server.name.clone()),
        isp: None,
        url: None,
    })
}

fn curl(args: &[&str]) -> Result<String, String> {
    let out = programs::run("curl", args)?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn servers() -> Result<Vec<Server>, String> {
    let text = curl(&["-s", "--max-time", "10", SERVERS])?;
    let list: Vec<Value> =
        serde_json::from_str(&text).map_err(|_| "Could not get LibreSpeed's list of servers.".to_string())?;
    let join = |base: &str, path: &str| {
        if path.starts_with("http") {
            return path.to_string();
        }
        let base = if base.starts_with("//") { format!("https:{base}") } else { base.to_string() };
        format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
    };
    Ok(list
        .iter()
        .filter_map(|s| {
            let base = s["server"].as_str()?;
            Some(Server {
                name: s["name"].as_str()?.to_string(),
                ping: join(base, s["pingURL"].as_str()?),
                down: join(base, s["dlURL"].as_str()?),
                up: join(base, s["ulURL"].as_str()?),
            })
        })
        .collect())
}

/// The server that answers fastest, asked all at once.
fn nearest(servers: &[Server]) -> Result<&Server, String> {
    let times: Vec<Option<f64>> = std::thread::scope(|scope| {
        let asks: Vec<_> = servers
            .iter()
            .map(|s| {
                scope.spawn(move || {
                    let text = curl(&[
                        "-s",
                        "-o",
                        "/dev/null",
                        "--max-time",
                        "2",
                        "-w",
                        "%{http_code} %{time_connect}",
                        &s.ping,
                    ])
                    .ok()?;
                    let mut f = text.split_whitespace();
                    (f.next()? == "200").then_some(())?;
                    f.next()?.parse::<f64>().ok()
                })
            })
            .collect();
        asks.into_iter().map(|a| a.join().ok().flatten()).collect()
    });
    servers
        .iter()
        .zip(times)
        .filter_map(|(s, t)| Some((s, t?)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(s, _)| s)
        .ok_or_else(|| "None of LibreSpeed's servers answered. Check the internet connection.".into())
}

/// Average latency and jitter (how much it varies) over ten requests on one connection.
fn ping(server: &Server) -> Result<(f64, f64), String> {
    let urls: Vec<String> = (0..10).map(|i| format!("{}?r={i}", server.ping)).collect();
    let mut args = vec!["-s", "-w", "%{time_pretransfer} %{time_starttransfer}\n"];
    for url in &urls {
        args.extend(["-o", "/dev/null", url.as_str()]);
    }
    let pings: Vec<f64> = curl(&args)?
        .lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace().filter_map(|v| v.parse::<f64>().ok());
            Some((f.next()?, f.next()?))
        })
        .map(|(pre, first)| (first - pre) * 1000.0)
        .collect();
    if pings.is_empty() {
        return Err(format!("{} didn't answer.", server.name));
    }
    let latency = pings.iter().sum::<f64>() / pings.len() as f64;
    let jitter = if pings.len() > 1 {
        pings.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (pings.len() - 1) as f64
    } else {
        0.0
    };
    Ok((latency, jitter))
}

fn refused(server: &Server, code: u32) -> String {
    match code {
        0 => format!("The connection to {} dropped during the test.", server.name),
        _ => format!("{} refused the test (HTTP {code}). Try again; it may pick another server.", server.name),
    }
}

/// Waits out the warm-up, then the measured window, stopping early when the
/// budget is used or every connection has ended. Returns when the window
/// started and the counter's value then.
fn run_window(children: &mut [Child], counter: &AtomicU64, budget: u64) -> (Instant, u64) {
    let ended = |children: &mut [Child]| children.iter_mut().all(|c| c.try_wait().ok().flatten().is_some());
    let began = Instant::now();
    while began.elapsed() < WARMUP && counter.load(Ordering::Relaxed) < budget / 5 && !ended(children) {
        std::thread::sleep(Duration::from_millis(25));
    }
    let (start, before) = (Instant::now(), counter.load(Ordering::Relaxed));
    while start.elapsed() < WINDOW && counter.load(Ordering::Relaxed) - before < budget && !ended(children) {
        std::thread::sleep(Duration::from_millis(25));
    }
    (start, before)
}

/// Bytes per second downloaded, counted as they arrive through curl's output.
fn download(server: &Server) -> Result<f64, String> {
    let url = format!("{}?ckSize=100", server.down);
    let moved = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(Mutex::new(None::<u32>));
    let mut children = Vec::new();
    let mut threads = Vec::new();
    for _ in 0..DOWN_STREAMS {
        // Back-to-back 100 MB requests on one connection; statuses go to stderr.
        let mut child = Command::new("curl")
            .args(["-s", "-w", "%{stderr}%{http_code}\n"])
            .args(std::iter::repeat_n(url.as_str(), 40))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not run curl: {e}"))?;
        let mut out = child.stdout.take().ok_or("curl has no output")?;
        let err = child.stderr.take().ok_or("curl has no output")?;
        let (moved, stop, failed) = (moved.clone(), stop.clone(), failed.clone());
        threads.push(std::thread::spawn(move || {
            let mut buf = vec![0u8; 64 * 1024];
            while !stop.load(Ordering::Relaxed) {
                match out.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => moved.fetch_add(n as u64, Ordering::Relaxed),
                };
            }
        }));
        threads.push(std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                if let Ok(code) = line.trim().parse::<u32>()
                    && code != 200
                {
                    failed.lock().unwrap().get_or_insert(code);
                }
            }
        }));
        children.push(child);
    }

    let (start, before) = run_window(&mut children, &moved, DOWN_BUDGET);
    let (seconds, bytes) = (start.elapsed().as_secs_f64(), moved.load(Ordering::Relaxed) - before);
    stop.store(true, Ordering::Relaxed);
    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }
    for thread in threads {
        let _ = thread.join();
    }
    if let Some(code) = *failed.lock().unwrap() {
        return Err(refused(server, code));
    }
    if bytes < 64 * 1024 {
        return Err("No data arrived during the download. Try again.".into());
    }
    Ok(bytes as f64 / seconds)
}

/// Bytes per second uploaded. The servers only take uploads of a few
/// megabytes, so each connection sends chunk after chunk, and the speed comes
/// from when the server confirms each one.
fn upload(server: &Server) -> Result<f64, String> {
    // Random bytes, so nothing along the way can compress them.
    let mut chunk = vec![0u8; MAX_CHUNK];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut chunk))
        .map_err(|e| format!("Could not make upload data: {e}"))?;
    let dir = std::env::temp_dir();
    let file = dir.join(format!("open-task-manager-upload-{}.bin", std::process::id()));

    // A one-megabyte probe sizes the chunks so a slow line still finishes several.
    std::fs::write(&file, &chunk[..1_000_000]).map_err(|e| e.to_string())?;
    let at = format!("@{}", file.to_string_lossy());
    let probe =
        curl(&["-s", "-o", "/dev/null", "-w", "%{http_code} %{speed_upload}", "--data-binary", &at, &server.up])?;
    let mut f = probe.split_whitespace();
    let code: u32 = f.next().and_then(|c| c.parse().ok()).unwrap_or(0);
    let rate: f64 = f.next().and_then(|r| r.parse().ok()).unwrap_or(0.0);
    if code != 200 {
        let _ = std::fs::remove_file(&file);
        return Err(refused(server, code));
    }
    let size = ((rate * 0.5) as usize).clamp(256 * 1024, MAX_CHUNK);
    std::fs::write(&file, &chunk[..size]).map_err(|e| e.to_string())?;

    let confirmed = Arc::new(AtomicU64::new(0));
    // One log per connection.
    let done: Vec<Confirmations> = (0..UP_STREAMS).map(|_| Arc::default()).collect();
    let mut children = Vec::new();
    let mut threads = Vec::new();
    for log in &done {
        let mut command = Command::new("curl");
        for i in 0..400 {
            if i > 0 {
                command.arg("--next");
            }
            // Statuses go to stderr, which curl doesn't buffer, so each arrives as its chunk is confirmed.
            command.args([
                "-s",
                "-o",
                "/dev/null",
                "-w",
                "%{stderr}%{http_code}\n",
                "-H",
                "Content-Type: application/octet-stream",
            ]);
            command.args(["--data-binary", &at, &server.up]);
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not run curl: {e}"))?;
        let out = child.stderr.take().ok_or("curl has no output")?;
        let (log, confirmed) = (log.clone(), confirmed.clone());
        threads.push(std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let code = line.trim().parse::<u32>().unwrap_or(0);
                if code == 200 {
                    confirmed.fetch_add(size as u64, Ordering::Relaxed);
                }
                log.lock().unwrap().push((Instant::now(), code));
            }
        }));
        children.push(child);
    }

    let (start, _) = run_window(&mut children, &confirmed, UP_BUDGET);
    let end = Instant::now();
    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }
    for thread in threads {
        let _ = thread.join();
    }
    let _ = std::fs::remove_file(&file);

    let mut total = 0.0;
    for log in &done {
        let log = log.lock().unwrap();
        if let Some(&(_, code)) = log.iter().find(|(_, c)| *c != 200) {
            return Err(refused(server, code));
        }
        // Chunks confirmed after the warm-up. Between the first and last of
        // them, the connection was busy the whole time, so that span is exact.
        let times: Vec<Instant> = log.iter().map(|(t, _)| *t).filter(|t| *t >= start && *t <= end).collect();
        total += match times.as_slice() {
            [first, .., last] => (times.len() - 1) as f64 * size as f64 / last.duration_since(*first).as_secs_f64(),
            [one] => size as f64 / one.duration_since(start).as_secs_f64().max(0.05),
            [] => 0.0,
        };
    }
    if total <= 0.0 {
        return Err("The upload didn't complete in time. The connection may be very slow; try again.".into());
    }
    Ok(total)
}

#[cfg(test)]
mod live_check {
    #[test]
    #[ignore]
    fn run() {
        let t = std::time::Instant::now();
        let r = super::test().unwrap();
        println!(
            "{} | ping {:.1} ms jitter {:.1} | down {:.0} Mbps | up {:.0} Mbps | {:?}",
            r.server.unwrap(),
            r.latency_ms,
            r.jitter_ms.unwrap(),
            r.download_mbps,
            r.upload_mbps,
            t.elapsed()
        );
    }
}
