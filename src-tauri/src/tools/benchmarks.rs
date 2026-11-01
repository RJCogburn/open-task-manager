//! Benchmarks. CPU and memory run built-in workloads; disk uses fio, GPU uses
//! glmark2, and internet speed uses Ookla's speedtest when installed or curl
//! against LibreSpeed's servers (librespeed.rs).

use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use crate::programs;

mod librespeed;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuScore {
    /// Millions of work loop iterations per second.
    single: f64,
    multi: f64,
    threads: usize,
}

/// A fixed mix of integer and floating point work, run for a set time.
fn cpu_work(duration: Duration) -> f64 {
    let start = Instant::now();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut f = 1.0f64;
    let mut iterations: u64 = 0;
    while start.elapsed() < duration {
        for _ in 0..10_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            f = (f + (x & 0xffff) as f64).sqrt() * 1.000_001;
        }
        iterations += 10_000;
    }
    std::hint::black_box((x, f));
    iterations as f64 / start.elapsed().as_secs_f64() / 1e6
}

#[tauri::command]
pub async fn bench_cpu() -> Result<CpuScore, String> {
    blocking(|| {
        let duration = Duration::from_secs(3);
        let single = cpu_work(duration);
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        let multi = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads).map(|_| s.spawn(|| cpu_work(duration))).collect();
            handles.into_iter().map(|h| h.join().unwrap_or(0.0)).sum()
        });
        Ok(CpuScore { single, multi, threads })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryScore {
    /// Gigabytes copied per second.
    copy_gbps: f64,
}

#[tauri::command]
pub async fn bench_memory() -> Result<MemoryScore, String> {
    blocking(|| {
        const LEN: usize = 32 << 20; // 32 Mi u64s = 256 MiB per buffer, far beyond any cache
        let src = vec![1u64; LEN];
        let mut dst = vec![0u64; LEN];
        dst.copy_from_slice(&src); // fault the pages in before timing
        let start = Instant::now();
        let mut copies = 0u32;
        while start.elapsed() < Duration::from_secs(2) {
            dst.copy_from_slice(&src);
            std::hint::black_box(&dst);
            copies += 1;
        }
        let bytes = copies as f64 * (LEN * 8) as f64;
        Ok(MemoryScore { copy_gbps: bytes / start.elapsed().as_secs_f64() / 1e9 })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskResult {
    name: String,
    /// Bytes per second.
    bandwidth: f64,
    iops: f64,
}

/// Sequential and random reads and writes on a 1 GiB test file, with fio.
#[tauri::command]
pub async fn bench_disk(folder: String) -> Result<Vec<DiskResult>, String> {
    blocking(move || {
        if programs::which("fio").is_none() {
            return Err(programs::missing("fio"));
        }
        let file = std::path::Path::new(&folder).join(".open-task-manager-bench.tmp");
        let file_arg = format!("--filename={}", file.display());
        let mut result = run_fio(&file_arg, "io_uring");
        if result.is_err() {
            result = run_fio(&file_arg, "psync");
        }
        let _ = std::fs::remove_file(&file);
        result
    })
    .await
}

fn run_fio(file_arg: &str, engine: &str) -> Result<Vec<DiskResult>, String> {
    let engine = format!("--ioengine={engine}");
    let mut args = vec![
        "--output-format=json",
        file_arg,
        "--size=1G",
        "--direct=1",
        &engine,
        "--runtime=5",
        "--time_based",
        "--ramp_time=1",
    ];
    let jobs = [
        ["--name=Sequential read", "--rw=read", "--bs=1M", "--iodepth=8"],
        ["--name=Sequential write", "--rw=write", "--bs=1M", "--iodepth=8"],
        ["--name=Random read 4K", "--rw=randread", "--bs=4k", "--iodepth=32"],
        ["--name=Random write 4K", "--rw=randwrite", "--bs=4k", "--iodepth=32"],
    ];
    for job in &jobs {
        args.extend_from_slice(job);
        args.push("--stonewall");
    }
    let out = programs::run("fio", &args)?;
    if !out.status.success() {
        return Err(format!("fio failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    // fio can print warnings before the JSON.
    let text = String::from_utf8_lossy(&out.stdout);
    let json: Value = serde_json::from_str(&text[text.find('{').unwrap_or(0)..])
        .map_err(|e| format!("Could not read fio output: {e}"))?;
    Ok(json["jobs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|job| {
            let side = if job["write"]["io_bytes"].as_u64().unwrap_or(0) > 0 { &job["write"] } else { &job["read"] };
            DiskResult {
                name: job["jobname"].as_str().unwrap_or_default().to_string(),
                bandwidth: side["bw_bytes"].as_f64().unwrap_or(0.0),
                iops: side["iops"].as_f64().unwrap_or(0.0),
            }
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuScore {
    score: u64,
    renderer: String,
}

/// glmark2's standard OpenGL run, drawn off screen. Takes a few minutes.
#[tauri::command]
pub async fn bench_gpu() -> Result<GpuScore, String> {
    blocking(|| {
        let out = programs::run("glmark2", &["--off-screen"])?;
        let text = String::from_utf8_lossy(&out.stdout);
        let find = |key: &str| text.lines().find_map(|l| l.split_once(key).map(|(_, v)| v.trim().to_string()));
        let score = find("glmark2 Score:").and_then(|s| s.parse().ok()).ok_or_else(|| {
            format!("glmark2 did not report a score. {}", String::from_utf8_lossy(&out.stderr).trim())
        })?;
        Ok(GpuScore { score, renderer: find("GL_RENDERER:").unwrap_or_default() })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetScore {
    latency_ms: f64,
    download_mbps: f64,
    upload_mbps: f64,
    /// "ookla" or "librespeed".
    provider: &'static str,
    jitter_ms: Option<f64>,
    server: Option<String>,
    isp: Option<String>,
    /// The result page on speedtest.net.
    url: Option<String>,
}

/// Speed test with Ookla's speedtest program when asked for, or when "auto"
/// and it is installed; LibreSpeed's nearest server otherwise.
#[tauri::command]
pub async fn bench_network(provider: String) -> Result<NetScore, String> {
    let ookla = match provider.as_str() {
        "ookla" => true,
        "librespeed" | "cloudflare" => false,
        _ => programs::which("speedtest").is_some(),
    };
    if ookla { blocking(ookla_test).await } else { blocking(librespeed::test).await }
}

/// Ookla's own command-line client, which picks a nearby speedtest.net server.
fn ookla_test() -> Result<NetScore, String> {
    let out = programs::run("speedtest", &["--format=json", "--progress=no", "--accept-license", "--accept-gdpr"])?;
    parse_ookla(&out.stdout, &out.stderr)
}

fn parse_ookla(stdout: &[u8], stderr: &[u8]) -> Result<NetScore, String> {
    let parse = |bytes: &[u8]| -> Vec<Value> {
        String::from_utf8_lossy(bytes).lines().filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok()).collect()
    };
    let lines = parse(stdout);
    let Some(result) = lines.iter().find(|v| v["type"] == "result") else {
        // Errors come as JSON log lines.
        let message = parse(stderr)
            .into_iter()
            .chain(lines.iter().cloned())
            .find_map(|v| v["message"].as_str().map(str::to_string))
            .unwrap_or_else(|| String::from_utf8_lossy(stderr).trim().to_string());
        return Err(if message.is_empty() { "Ookla's speedtest gave no result.".into() } else { message });
    };
    let mbps = |dir: &str| result[dir]["bandwidth"].as_f64().unwrap_or(0.0) * 8.0 / 1e6;
    let server = &result["server"];
    Ok(NetScore {
        latency_ms: result["ping"]["latency"].as_f64().unwrap_or(0.0),
        download_mbps: mbps("download"),
        upload_mbps: mbps("upload"),
        provider: "ookla",
        jitter_ms: result["ping"]["jitter"].as_f64(),
        server: server["name"].as_str().map(|name| match server["location"].as_str() {
            Some(location) => format!("{name}, {location}"),
            None => name.to_string(),
        }),
        isp: result["isp"].as_str().map(str::to_string),
        url: result["result"]["url"].as_str().map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ookla_results() {
        let out = br#"{"type":"result","ping":{"jitter":0.42,"latency":9.8},"download":{"bandwidth":117500000},"upload":{"bandwidth":43750000},"isp":"Example ISP","server":{"name":"Example","location":"Seattle, WA"},"result":{"url":"https://www.speedtest.net/result/c/abc"}}"#;
        let r = parse_ookla(out, b"").unwrap();
        assert_eq!((r.download_mbps, r.upload_mbps, r.latency_ms), (940.0, 350.0, 9.8));
        assert_eq!(r.server.as_deref(), Some("Example, Seattle, WA"));
        assert_eq!(r.url.as_deref(), Some("https://www.speedtest.net/result/c/abc"));
    }

    #[test]
    fn reports_ookla_errors() {
        let err =
            br#"{"type":"log","timestamp":"2026-11-01T10:00:00Z","message":"Cannot read from socket","level":"error"}"#;
        assert_eq!(parse_ookla(b"", err).err().as_deref(), Some("Cannot read from socket"));
    }
}
