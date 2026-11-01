//! Physical drives: what lsblk knows about each one and the filesystems on
//! it, plus the drive's health from its own SMART data when smartctl can
//! read it.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::AppState;
use crate::programs;

/// SMART results by drive name. smartctl is tried once per drive without
/// rights; reading it as administrator replaces the entry.
#[derive(Default)]
pub struct HealthCache(Mutex<HashMap<String, Health>>);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// "good", "warning", "failing" or "unknown".
    status: &'static str,
    note: String,
    needs_admin: bool,
    temp: Option<f64>,
    power_on_hours: Option<u64>,
    /// Percent of the rated write endurance used up (NVMe).
    wear: Option<f64>,
    /// Percent of spare blocks left (NVMe).
    spare: Option<f64>,
    media_errors: Option<u64>,
    /// Reallocated, pending and uncorrectable sectors (SATA).
    bad_sectors: Option<u64>,
}

impl Health {
    fn unknown(note: impl Into<String>, needs_admin: bool) -> Self {
        Self {
            status: "unknown",
            note: note.into(),
            needs_admin,
            temp: None,
            power_on_hours: None,
            wear: None,
            spare: None,
            media_errors: None,
            bad_sectors: None,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    name: String,
    model: String,
    vendor: String,
    serial: String,
    revision: String,
    size: u64,
    transport: String,
    rotational: bool,
    removable: bool,
    filesystems: Vec<Filesystem>,
    health: Health,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Filesystem {
    device: String,
    label: String,
    file_system: String,
    mount_points: Vec<String>,
    total: u64,
    used: Option<u64>,
    available: Option<u64>,
    encrypted: bool,
}

#[tauri::command]
pub async fn drives(state: State<'_, AppState>) -> Result<Vec<Drive>, String> {
    let cache = state.health.clone();
    tauri::async_runtime::spawn_blocking(move || list(&cache)).await.map_err(|e| e.to_string())?
}

fn list(cache: &Arc<HealthCache>) -> Result<Vec<Drive>, String> {
    let out = programs::run(
        "lsblk",
        &[
            "--json",
            "--bytes",
            "--output",
            "NAME,TYPE,MODEL,VENDOR,SERIAL,REV,SIZE,TRAN,ROTA,RM,FSTYPE,LABEL,MOUNTPOINTS,FSSIZE,FSUSED,FSAVAIL",
        ],
    )?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let json: Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("Could not read lsblk output: {e}"))?;
    let smartctl = programs::which("smartctl").is_some();
    let mut drives = Vec::new();
    for dev in json["blockdevices"].as_array().into_iter().flatten() {
        let name = text(&dev["name"]);
        if text(&dev["type"]) != "disk" || name.starts_with("zram") || name.starts_with("ram") {
            continue;
        }
        let mut filesystems = Vec::new();
        collect(dev, false, &mut filesystems);

        let mut health = {
            let mut cache = cache.0.lock().unwrap();
            cache
                .entry(name.clone())
                .or_insert_with(|| {
                    if smartctl {
                        read_smart(&name)
                    } else {
                        Health::unknown("Install smartmontools to read drive health.", false)
                    }
                })
                .clone()
        };
        // The kernel's own sensor is live; SMART is read once.
        if let Some(t) = sensor_temp(&name) {
            health.temp = Some(t);
        }

        drives.push(Drive {
            model: text(&dev["model"]),
            vendor: text(&dev["vendor"]),
            serial: text(&dev["serial"]),
            revision: text(&dev["rev"]),
            size: num(&dev["size"]).unwrap_or(0),
            transport: text(&dev["tran"]),
            rotational: flag(&dev["rota"]),
            removable: flag(&dev["rm"]),
            filesystems,
            health,
            name,
        });
    }
    drives.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(drives)
}

/// Every filesystem under a device, looking through partitions, LUKS and LVM.
fn collect(dev: &Value, encrypted: bool, out: &mut Vec<Filesystem>) {
    let fs_type = text(&dev["fstype"]);
    let container = matches!(fs_type.as_str(), "crypto_LUKS" | "LVM2_member" | "linux_raid_member" | "swap");
    if !fs_type.is_empty() && !container {
        let mount_points: Vec<String> = dev["mountpoints"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m.as_str().map(str::to_string))
            .collect();
        let mounted = !mount_points.is_empty();
        out.push(Filesystem {
            device: text(&dev["name"]),
            label: text(&dev["label"]),
            file_system: fs_type.clone(),
            total: num(&dev["fssize"]).filter(|_| mounted).or_else(|| num(&dev["size"])).unwrap_or(0),
            used: num(&dev["fsused"]).filter(|_| mounted),
            available: num(&dev["fsavail"]).filter(|_| mounted),
            mount_points,
            encrypted,
        });
    }
    for child in dev["children"].as_array().into_iter().flatten() {
        collect(child, encrypted || fs_type == "crypto_LUKS", out);
    }
}

fn text(v: &Value) -> String {
    v.as_str().map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Older lsblk versions print numbers and flags as strings.
fn num(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str()?.trim().parse().ok())
}

fn flag(v: &Value) -> bool {
    v.as_bool().unwrap_or_else(|| v.as_str() == Some("1"))
}

/// Whole drives the kernel lists, without loop, RAM and mapper devices.
pub(crate) fn disk_names() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir("/sys/block")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !["loop", "ram", "zram", "dm-", "sr", "md"].iter().any(|p| n.starts_with(p)))
        .filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric()))
        .collect();
    names.sort();
    names
}

/// The drive's temperature from the kernel: the nvme driver's hwmon, or
/// drivetemp for SATA drives.
pub(crate) fn sensor_temp(name: &str) -> Option<f64> {
    let device = Path::new("/sys/block").join(name).join("device");
    let dirs = [device.clone(), device.join("hwmon")];
    dirs.iter()
        .filter_map(|d| fs::read_dir(d).ok())
        .flat_map(|d| d.flatten())
        .filter(|e| e.file_name().to_string_lossy().starts_with("hwmon"))
        .find_map(|e| fs::read_to_string(e.path().join("temp1_input")).ok()?.trim().parse::<f64>().ok())
        .map(|m| m / 1000.0)
}

fn read_smart(name: &str) -> Health {
    match programs::run("smartctl", &["--json", "-H", "-A", "-i", &format!("/dev/{name}")]) {
        Ok(out) => match serde_json::from_slice::<Value>(&out.stdout) {
            Ok(json) => parse_smart(&json),
            Err(_) => Health::unknown("smartctl gave no readable answer for this drive.", false),
        },
        Err(e) => Health::unknown(e, false),
    }
}

fn parse_smart(json: &Value) -> Health {
    let exit = json["smartctl"]["exit_status"].as_u64().unwrap_or(0);
    let messages: Vec<&str> =
        json["smartctl"]["messages"].as_array().into_iter().flatten().filter_map(|m| m["string"].as_str()).collect();
    // Bit 1: the device could not be opened. Bit 2: a SMART command failed.
    if exit & 0b10 != 0 {
        let joined = messages.join(" ");
        return if joined.contains("Permission denied") || joined.contains("Operation not permitted") {
            Health::unknown("Reading SMART health needs administrator rights.", true)
        } else if joined.contains("USB bridge") || joined.contains("specify device type") {
            Health::unknown("This drive's USB adapter does not pass SMART health through.", false)
        } else {
            Health::unknown(messages.first().copied().unwrap_or("smartctl could not open this drive."), false)
        };
    }

    let nvme = &json["nvme_smart_health_information_log"];
    let passed = json["smart_status"]["passed"].as_bool();
    let wear = nvme["percentage_used"].as_f64();
    let spare = nvme["available_spare"].as_f64();
    let spare_floor = nvme["available_spare_threshold"].as_f64();
    let critical = nvme["critical_warning"].as_u64().unwrap_or(0);
    let media_errors = nvme["media_errors"].as_u64();
    let bad_sectors = json["ata_smart_attributes"]["table"].as_array().map(|table| {
        table
            .iter()
            .filter(|a| matches!(a["id"].as_u64(), Some(5 | 197 | 198)))
            .filter_map(|a| a["raw"]["value"].as_u64())
            .sum::<u64>()
    });

    let mut problems = Vec::new();
    if critical != 0 {
        problems.push("the drive reports a critical warning".to_string());
    }
    if let (Some(s), Some(floor)) = (spare, spare_floor)
        && s < floor
    {
        problems.push(format!("spare blocks are below the safe level ({s}%)"));
    }
    if let Some(w) = wear.filter(|&w| w >= 90.0) {
        problems.push(format!("{w}% of its rated endurance is used"));
    }
    if let Some(n) = media_errors.filter(|&n| n > 0) {
        problems.push(format!("{n} media errors"));
    }
    if let Some(n) = bad_sectors.filter(|&n| n > 0) {
        problems.push(format!("{n} bad or pending sectors"));
    }

    let (status, note) = match passed {
        Some(false) => ("failing", "The drive's own self-assessment says it is failing. Back up its data.".to_string()),
        _ if !problems.is_empty() => ("warning", format!("Worth watching: {}.", problems.join(", "))),
        Some(true) => ("good", "The drive's self-assessment passed.".to_string()),
        None => ("unknown", "This drive does not report a SMART self-assessment.".to_string()),
    };
    Health {
        status,
        note,
        needs_admin: false,
        temp: json["temperature"]["current"].as_f64(),
        power_on_hours: json["power_on_time"]["hours"].as_u64(),
        wear,
        spare,
        media_errors,
        bad_sectors,
    }
}

/// Reads SMART health for every drive as administrator, with one password
/// prompt from polkit.
#[tauri::command]
pub async fn drive_health_admin(state: State<'_, AppState>) -> Result<(), String> {
    let cache = state.health.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if programs::which("smartctl").is_none() {
            return Err(programs::missing("smartctl"));
        }
        // Only names the kernel lists as block devices reach the command line.
        let names = disk_names();
        if names.is_empty() {
            return Ok(());
        }
        let mut args = vec!["sh", "-c", r#"for d; do smartctl --json -H -A -i "/dev/$d"; done"#, "sh"];
        args.extend(names.iter().map(String::as_str));
        let out = programs::run("pkexec", &args)?;
        if out.stdout.is_empty() {
            return Err(match out.status.code() {
                Some(126) | Some(127) => "Cancelled.".into(),
                _ => String::from_utf8_lossy(&out.stderr).trim().to_string(),
            });
        }
        // smartctl prints one JSON document per drive, back to back.
        let mut cache = cache.0.lock().unwrap();
        for json in serde_json::Deserializer::from_slice(&out.stdout).into_iter::<Value>().flatten() {
            let Some(name) =
                json["smartctl"]["argv"].as_array().and_then(|a| a.last()?.as_str()?.strip_prefix("/dev/"))
            else {
                continue;
            };
            cache.insert(name.to_string(), parse_smart(&json));
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
