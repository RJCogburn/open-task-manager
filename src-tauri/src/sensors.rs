//! Temperatures, fans, voltages and power from lm-sensors (`sensors -j`), and
//! CPU package power from the kernel's RAPL counters when they are readable.

use std::fs;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::AppState;
use crate::tools;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chip {
    id: String,
    name: String,
    adapter: String,
    readings: Vec<Reading>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    label: String,
    kind: &'static str,
    value: f64,
    unit: &'static str,
    max: Option<f64>,
    crit: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorReport {
    chips: Vec<Chip>,
    /// CPU package power in watts, from RAPL.
    cpu_power: Option<f64>,
    /// Why CPU power is missing, when it is.
    cpu_power_note: Option<String>,
}

#[derive(Default)]
pub struct RaplState {
    last: Mutex<Option<(u64, Instant)>>,
}

#[tauri::command]
pub async fn sensors(state: State<'_, AppState>) -> Result<SensorReport, String> {
    let (cpu_power, cpu_power_note) = rapl_watts(&state.rapl);
    let chips = tauri::async_runtime::spawn_blocking(read_lm_sensors).await.map_err(|e| e.to_string())??;
    Ok(SensorReport { chips, cpu_power, cpu_power_note })
}

fn read_lm_sensors() -> Result<Vec<Chip>, String> {
    let out = tools::run("sensors", &["-j"])?;
    // sensors exits non-zero when one chip fails to read but still prints the rest.
    let json: Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("Could not read sensors output: {e}"))?;
    let Value::Object(chips) = json else { return Ok(Vec::new()) };
    let mut out: Vec<Chip> = chips
        .into_iter()
        .filter_map(|(id, chip)| {
            let Value::Object(features) = chip else { return None };
            let adapter = features.get("Adapter").and_then(Value::as_str).unwrap_or_default().to_string();
            let readings: Vec<Reading> =
                features.iter().filter_map(|(label, sub)| reading(label, sub.as_object()?)).collect();
            (!readings.is_empty()).then(|| Chip { name: chip_name(&id), id, adapter, readings })
        })
        .collect();
    out.sort_by_key(|c| (chip_rank(&c.id), c.id.clone()));
    Ok(out)
}

/// One feature, e.g. "Tctl": {"temp1_input": 52.8}.
fn reading(label: &str, sub: &serde_json::Map<String, Value>) -> Option<Reading> {
    let find = |suffix: &str| sub.iter().find(|(k, _)| k.ends_with(suffix)).and_then(|(_, v)| v.as_f64());
    let (key, _) =
        sub.iter().find(|(k, _)| k.ends_with("_input") || k.ends_with("_average") || k.starts_with("pwm"))?;
    let (kind, unit) = match key.trim_start_matches(|c: char| !c.is_ascii_alphabetic()) {
        k if k.starts_with("temp") => ("temperature", "°C"),
        k if k.starts_with("fan") => ("fan", "RPM"),
        k if k.starts_with("in") => ("voltage", "V"),
        k if k.starts_with("power") => ("power", "W"),
        k if k.starts_with("curr") => ("current", "A"),
        k if k.starts_with("freq") => ("frequency", "MHz"),
        k if k.starts_with("pwm") => ("pwm", "%"),
        k if k.starts_with("humidity") => ("humidity", "%"),
        _ => return None,
    };
    let mut value =
        if kind == "pwm" { sub.get(key.as_str())?.as_f64()? } else { find("_input").or_else(|| find("_average"))? };
    if kind == "frequency" {
        value /= 1e6;
    }
    // Unconnected temperature probes report placeholders like -55 or 127.
    if kind == "temperature" && !(-40.0..=150.0).contains(&value) {
        return None;
    }
    let limit = |v: Option<f64>| v.filter(|v| kind == "temperature" && (1.0..=150.0).contains(v));
    Some(Reading { label: label.to_string(), kind, value, unit, max: limit(find("_max")), crit: limit(find("_crit")) })
}

/// A readable name for a chip such as "k10temp-pci-00c3".
fn chip_name(id: &str) -> String {
    let driver = id.split('-').next().unwrap_or(id);
    let name = match driver {
        "k10temp" | "zenpower" => "AMD CPU",
        "coretemp" => "Intel CPU",
        "amdgpu" => "AMD GPU",
        "nouveau" => "NVIDIA GPU",
        "nvme" => "NVMe drive",
        "acpitz" => "ACPI thermal zone",
        "spd5118" | "jc42" => "Memory module",
        d if d.starts_with("it87") || d.starts_with("nct") || d.starts_with("asus") => "Motherboard",
        d if d.starts_with("en")
            || d.starts_with("eth")
            || d.starts_with("wl")
            || d.starts_with("ath")
            || d.starts_with("iwl") =>
        {
            "Network adapter"
        }
        d if d.contains("xhci") => "USB controller",
        d if d.contains("battery") => "Battery",
        _ => driver,
    };
    format!("{name} · {id}")
}

fn chip_rank(id: &str) -> u8 {
    match id.split('-').next().unwrap_or(id) {
        "k10temp" | "zenpower" | "coretemp" => 0,
        "amdgpu" | "nouveau" => 1,
        "nvme" => 2,
        d if d.starts_with("it87") || d.starts_with("nct") => 3,
        _ => 4,
    }
}

/// Average package power since the last call, from the RAPL energy counter.
fn rapl_watts(state: &RaplState) -> (Option<f64>, Option<String>) {
    let path = "/sys/class/powercap/intel-rapl:0/energy_uj";
    let energy = match fs::read_to_string(path) {
        Ok(s) => s.trim().parse::<u64>().ok(),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return (None, Some("The kernel only lets administrators read CPU energy counters (RAPL).".into()));
        }
        Err(_) => return (None, Some("This CPU does not expose energy counters (RAPL).".into())),
    };
    let Some(energy) = energy else { return (None, None) };
    let now = Instant::now();
    let mut last = state.last.lock().unwrap();
    let watts = last.and_then(|(prev, at)| {
        let dt = now.duration_since(at).as_secs_f64();
        (energy >= prev && dt > 0.05).then(|| (energy - prev) as f64 / 1e6 / dt)
    });
    *last = Some((energy, now));
    (watts, None)
}
