//! GPU readings. NVIDIA cards come from one long-running `nvidia-smi` that
//! prints a line per card every second; AMD cards come from the amdgpu
//! driver's files in /sys/class/drm. While nothing needs GPU readings,
//! `nvidia-smi` is stopped.

use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Serialize;

use crate::programs;

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuSample {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) vendor: &'static str,
    pub(crate) source: &'static str,
    pub(crate) util: Option<f32>,
    pub(crate) mem_used: Option<u64>,
    pub(crate) mem_total: Option<u64>,
    pub(crate) temp: Option<f32>,
    pub(crate) power: Option<f32>,
    pub(crate) power_limit: Option<f32>,
    pub(crate) clock_mhz: Option<f32>,
    pub(crate) fan: Option<f32>,
}

pub struct GpuMonitor {
    nvidia: Arc<Mutex<Vec<GpuSample>>>,
    /// The running `nvidia-smi`, while GPU readings are wanted.
    nvidia_smi: Mutex<Option<Child>>,
    /// False when there is no `nvidia-smi`, or it would not start.
    has_nvidia_smi: AtomicBool,
    amd: Vec<AmdCard>,
}

struct AmdCard {
    id: String,
    name: String,
    device: PathBuf,
    hwmon: Option<PathBuf>,
}

impl GpuMonitor {
    pub fn start() -> Self {
        Self {
            nvidia: Arc::default(),
            nvidia_smi: Mutex::new(None),
            has_nvidia_smi: AtomicBool::new(programs::which("nvidia-smi").is_some()),
            amd: find_amd_cards(),
        }
    }

    /// The latest readings, or none when they aren't `wanted`. Starts
    /// `nvidia-smi` when readings are wanted again, and stops it when not.
    pub fn sample(&self, wanted: bool) -> Vec<GpuSample> {
        let mut child = self.nvidia_smi.lock().unwrap();
        if !wanted {
            if let Some(mut c) = child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            return Vec::new();
        }
        if child.is_none() && self.has_nvidia_smi.load(Ordering::Relaxed) {
            // Readings from before the pause would show as current.
            self.nvidia.lock().unwrap().clear();
            *child = start_nvidia_smi(self.nvidia.clone());
            if child.is_none() {
                self.has_nvidia_smi.store(false, Ordering::Relaxed);
            }
        }
        drop(child);
        let mut out = self.nvidia.lock().unwrap().clone();
        out.extend(self.amd.iter().map(AmdCard::sample));
        out
    }
}

const NVIDIA_FIELDS: &str =
    "index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,power.limit,clocks.gr,fan.speed";

/// Starts `nvidia-smi` and a thread that keeps `out` filled from it. The
/// thread ends when `nvidia-smi` does.
fn start_nvidia_smi(out: Arc<Mutex<Vec<GpuSample>>>) -> Option<Child> {
    let mut command = Command::new("nvidia-smi");
    command
        .args([&format!("--query-gpu={NVIDIA_FIELDS}"), "--format=csv,noheader,nounits", "-lms", "1000"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Stop nvidia-smi when this app exits, even if it crashes.
    unsafe {
        command.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    let mut child = command.spawn().ok()?;
    let stdout = child.stdout.take()?;
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            if f.len() < 10 {
                continue;
            }
            let num = |s: &str| s.parse::<f32>().ok();
            let sample = GpuSample {
                id: format!("nvidia{}", f[0]),
                name: f[1].to_string(),
                vendor: "NVIDIA",
                source: "nvidia-smi",
                util: num(f[2]),
                mem_used: num(f[3]).map(|m| (m as u64) << 20),
                mem_total: num(f[4]).map(|m| (m as u64) << 20),
                temp: num(f[5]),
                power: num(f[6]),
                power_limit: num(f[7]),
                clock_mhz: num(f[8]),
                fan: num(f[9]),
            };
            let mut gpus = out.lock().unwrap();
            match gpus.iter_mut().find(|g| g.id == sample.id) {
                Some(slot) => *slot = sample,
                None => gpus.push(sample),
            }
        }
    });
    Some(child)
}

fn find_amd_cards() -> Vec<AmdCard> {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else { return Vec::new() };
    let mut cards: Vec<AmdCard> = entries
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            // card0, card1… but not connectors like card0-DP-1.
            if !id.starts_with("card") || id.contains('-') {
                return None;
            }
            let device = e.path().join("device");
            if read_trim(&device.join("vendor")).as_deref() != Some("0x1002")
                || !device.join("gpu_busy_percent").exists()
            {
                return None;
            }
            let hwmon = fs::read_dir(device.join("hwmon")).ok()?.flatten().next().map(|h| h.path());
            Some(AmdCard { name: amd_name(&device), id, device, hwmon })
        })
        .collect();
    cards.sort_by(|a, b| a.id.cmp(&b.id));
    cards
}

/// The marketing name from lspci (pciutils), e.g. "Granite Ridge [Radeon 610M]".
fn amd_name(device: &Path) -> String {
    let slot = fs::read_to_string(device.join("uevent"))
        .ok()
        .and_then(|u| u.lines().find_map(|l| l.strip_prefix("PCI_SLOT_NAME=").map(str::to_string)));
    let from_lspci = slot.and_then(|slot| {
        let out = programs::run("lspci", &["-mm", "-s", &slot]).ok()?;
        // Quoted fields: class, vendor, device, …
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.split('"').nth(5).map(str::to_string)
    });
    format!("AMD {}", from_lspci.unwrap_or_else(|| "Radeon".into()))
}

impl AmdCard {
    fn sample(&self) -> GpuSample {
        let num = |p: &Path| read_trim(p).and_then(|s| s.parse::<f64>().ok());
        let hw = |file: &str| self.hwmon.as_ref().and_then(|h| num(&h.join(file)));
        GpuSample {
            id: self.id.clone(),
            name: self.name.clone(),
            vendor: "AMD",
            source: "amdgpu",
            util: num(&self.device.join("gpu_busy_percent")).map(|v| v as f32),
            mem_used: num(&self.device.join("mem_info_vram_used")).map(|v| v as u64),
            mem_total: num(&self.device.join("mem_info_vram_total")).map(|v| v as u64),
            temp: hw("temp1_input").map(|m| (m / 1000.0) as f32),
            power: hw("power1_average").or_else(|| hw("power1_input")).map(|uw| (uw / 1e6) as f32),
            power_limit: hw("power1_cap").map(|uw| (uw / 1e6) as f32),
            clock_mhz: hw("freq1_input").map(|hz| (hz / 1e6) as f32),
            fan: None,
        }
    }
}

fn read_trim(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}
