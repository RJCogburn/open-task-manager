//! Benchmarks. CPU and memory run built-in workloads; disk uses fio, GPU uses
//! glmark2 and internet speed uses curl against Cloudflare's speed test.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use crate::tools;

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
        if tools::which("fio").is_none() {
            return Err(tools::missing("fio"));
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
    let out = tools::run("fio", &args)?;
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
        let out = tools::run("glmark2", &["--off-screen"])?;
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
}

const SPEED: &str = "https://speed.cloudflare.com";

/// Latency, download and upload against speed.cloudflare.com, using curl.
#[tauri::command]
pub async fn bench_network() -> Result<NetScore, String> {
    blocking(|| {
        let mut latency = f64::MAX;
        for _ in 0..5 {
            let out = tools::run(
                "curl",
                &[
                    "-s",
                    "-o",
                    "/dev/null",
                    "-w",
                    "%{time_pretransfer} %{time_starttransfer}",
                    &format!("{SPEED}/__down?bytes=0"),
                ],
            )?;
            let text = String::from_utf8_lossy(&out.stdout);
            let mut t = text.split_whitespace().filter_map(|v| v.parse::<f64>().ok());
            if let (Some(pre), Some(first)) = (t.next(), t.next()) {
                latency = latency.min((first - pre) * 1000.0);
            }
        }
        if latency == f64::MAX {
            return Err("Could not reach speed.cloudflare.com.".into());
        }
        let down = tools::run(
            "curl",
            &["-s", "-o", "/dev/null", "-w", "%{speed_download}", &format!("{SPEED}/__down?bytes=100000000")],
        )?;
        let download = String::from_utf8_lossy(&down.stdout).trim().parse::<f64>().unwrap_or(0.0);
        Ok(NetScore { latency_ms: latency, download_mbps: download * 8.0 / 1e6, upload_mbps: upload()? * 8.0 / 1e6 })
    })
    .await
}

/// Bytes per second uploading 25 MB, streamed to curl's stdin.
fn upload() -> Result<f64, String> {
    let mut child = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{speed_upload}", "--data-binary", "@-", &format!("{SPEED}/__up")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("curl has no input")?;
    let chunk = vec![0u8; 1 << 20];
    for _ in 0..25 {
        if stdin.write_all(&chunk).is_err() {
            break;
        }
    }
    drop(stdin);
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0.0))
}
