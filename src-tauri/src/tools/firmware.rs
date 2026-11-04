//! Drive firmware: what each drive runs, whether fwupd has an update for it,
//! whether that version has a known serious bug, and where its maker
//! publishes firmware otherwise.
//!
//! Updates go only through fwupd, which downloads firmware the maker signed
//! on the LVFS (fwupd.org), checks it and asks for the password itself. Most
//! makers of consumer SSDs publish through their own Windows tools or boot
//! disks instead; for those the app says where, and doesn't flash anything
//! itself: a bad flash can leave a drive unusable.

use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::programs;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareReport {
    /// Whether fwupd is installed.
    fwupd: bool,
    /// Days since fwupd last fetched the list of firmware, if it ever has.
    metadata_age_days: Option<u64>,
    /// By block device name, like nvme0n1.
    drives: HashMap<String, DriveFirmware>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DriveFirmware {
    version: String,
    /// fwupd's id for the drive, if it can update it.
    fwupd_id: Option<String>,
    /// A newer version fwupd has for it.
    update: Option<String>,
    issue: Option<KnownIssue>,
    maker: Option<Maker>,
}

#[derive(Serialize, Clone, Copy)]
pub struct KnownIssue {
    text: &'static str,
    source: &'static str,
}

#[derive(Serialize, Clone, Copy)]
pub struct Maker {
    name: &'static str,
    /// How that maker ships firmware.
    how: &'static str,
    url: &'static str,
}

/// Firmware versions with serious, documented bugs, by model text and version.
const ISSUES: &[(&str, &str, KnownIssue)] = &[
    (
        "980 pro",
        "3B2QGXA7",
        KnownIssue {
            text: "This firmware can make the drive fail suddenly into read-only mode. Samsung fixed it in \
                   5B2QGXA7. Update now with Samsung Magician or Samsung's firmware boot disk.",
            source: "https://www.neowin.net/amp/after-fixing-dying-980-pro-ssds-samsung-promises-firmware-for-990-pro-health-issue-as-well/",
        },
    ),
    (
        "990 pro",
        "0B2QJXD7",
        KnownIssue {
            text: "This firmware wears the drive out much faster than it should. Samsung fixed it in 1B2QJXD7; \
                   the update stops further wear but doesn't undo what has happened. Update with Samsung Magician \
                   or Samsung's firmware boot disk.",
            source: "https://www.tomshardware.com/news/samsung-990-pro-firmware-update-released-ssd-health",
        },
    ),
];

/// Where each maker publishes firmware, by words in the drive's model or vendor.
fn maker(text: &str) -> Option<Maker> {
    let t = text.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| t.contains(w));
    // Model numbers: CT4000P3SSD8 is Crucial, ST4000DM004 Seagate, WD40NMZW Western Digital.
    let numbered = |prefix: &str| {
        t.split_whitespace()
            .any(|w| w.strip_prefix(prefix).is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit())))
    };
    Some(if has(&["samsung"]) {
        Maker {
            name: "Samsung",
            how: "Samsung Magician on Windows, or a firmware boot disk for other systems.",
            url: "https://semiconductor.samsung.com/consumer-storage/support/tools/",
        }
    } else if has(&["wd_", "wdc", "western digital", "sandisk"]) || numbered("wd") {
        Maker {
            name: "Western Digital and SanDisk",
            how: "SanDisk Dashboard, on Windows.",
            url: "https://shop.sandisk.com/topics/upgrades/drive-monitoring-sandisk-dashboard",
        }
    } else if has(&["crucial", "micron"]) || numbered("ct") {
        Maker {
            name: "Crucial",
            how: "Crucial Storage Executive, on Windows.",
            url: "https://www.crucial.com/support/storage-executive",
        }
    } else if has(&["corsair", "force mp"]) {
        Maker {
            name: "Corsair",
            how: "Corsair's SSD Toolbox or iCUE, on Windows.",
            url: "https://www.corsair.com/us/en/s/downloads",
        }
    } else if has(&["kingston"]) {
        Maker { name: "Kingston", how: "Kingston SSD Manager, on Windows.", url: "https://www.kingston.com/SSDmanager" }
    } else if has(&["seagate", "firecuda", "barracuda", "ironwolf"]) || numbered("st") {
        Maker {
            name: "Seagate",
            how: "Seagate's support site and SeaTools.",
            url: "https://www.seagate.com/support/downloads/seatools/",
        }
    } else if has(&["solidigm", "intel"]) {
        Maker {
            name: "Solidigm (formerly Intel)",
            how: "Solidigm Storage Tool.",
            url: "https://www.solidigm.com/support-page/drivers-downloads.html",
        }
    } else if has(&["sk hynix", "hynix", "platinum p41", "hfs"]) {
        Maker { name: "SK hynix", how: "SK hynix Drive Manager, on Windows.", url: "https://ssd.skhynix.com/download/" }
    } else {
        return None;
    })
}

fn issue(model: &str, version: &str) -> Option<KnownIssue> {
    let m = model.to_lowercase();
    ISSUES.iter().find(|(words, v, _)| m.contains(words) && version.trim() == *v).map(|(_, _, i)| *i)
}

/// Known bugs in the drives' firmware, for the slowdown check: (drive, issue).
#[cfg_attr(not(feature = "slowdown"), allow(dead_code))]
pub(crate) fn known_issues() -> Vec<(String, KnownIssue)> {
    block_devices()
        .into_iter()
        .filter_map(|d| issue(&d.model, &d.rev).map(|i| (format!("{} ({})", d.model, d.name), i)))
        .collect()
}

#[cfg_attr(not(feature = "slowdown"), allow(dead_code))]
impl KnownIssue {
    pub(crate) fn text(&self) -> &'static str {
        self.text
    }
}

#[derive(Deserialize)]
struct Lsblk {
    blockdevices: Vec<BlockDevice>,
}

#[derive(Deserialize)]
struct BlockDevice {
    name: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    vendor: Option<String>,
    #[serde(default)]
    rev: Option<String>,
}

struct Disk {
    name: String,
    model: String,
    vendor: String,
    rev: String,
}

fn block_devices() -> Vec<Disk> {
    let Ok(out) = programs::run("lsblk", &["--json", "--nodeps", "--output", "NAME,MODEL,VENDOR,REV"]) else {
        return Vec::new();
    };
    let Ok(list) = serde_json::from_slice::<Lsblk>(&out.stdout) else { return Vec::new() };
    list.blockdevices
        .into_iter()
        .filter(|d| !d.name.starts_with("loop") && !d.name.starts_with("zram") && !d.name.starts_with("ram"))
        .map(|d| Disk {
            name: d.name,
            model: d.model.unwrap_or_default().trim().to_string(),
            vendor: d.vendor.unwrap_or_default().trim().to_string(),
            rev: d.rev.unwrap_or_default().trim().to_string(),
        })
        .collect()
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct FwupdList {
    #[serde(default)]
    devices: Vec<FwupdDevice>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct FwupdDevice {
    #[serde(default)]
    name: String,
    #[serde(default)]
    device_id: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    flags: Vec<String>,
    #[serde(default)]
    releases: Vec<FwupdRelease>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct FwupdRelease {
    #[serde(default)]
    version: String,
}

fn fwupd(args: &[&str]) -> Vec<FwupdDevice> {
    programs::run("fwupdmgr", args)
        .ok()
        .and_then(|o| serde_json::from_slice::<FwupdList>(&o.stdout).ok())
        .unwrap_or_default()
        .devices
}

/// Drive model names written the same way: fwupd says "WD BLACK SN850X",
/// lsblk "WD_BLACK SN850X".
fn norm(s: &str) -> String {
    s.to_lowercase().replace('_', " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The fwupd device for a drive: the same firmware version, and one name
/// containing the other. Two identical drives each take one.
fn matching<'a>(disk: &Disk, devices: &'a [FwupdDevice], taken: &mut Vec<String>) -> Option<&'a FwupdDevice> {
    let model = norm(&disk.model);
    if model.is_empty() {
        return None;
    }
    let found = devices.iter().find(|d| {
        let name = norm(&d.name);
        !name.is_empty()
            && d.version == disk.rev
            && (model.contains(&name) || name.contains(&model))
            && !taken.contains(&d.device_id)
    })?;
    taken.push(found.device_id.clone());
    Some(found)
}

fn metadata_age_days() -> Option<u64> {
    let newest = fs::read_dir("/var/lib/fwupd/metadata/lvfs")
        .ok()?
        .flatten()
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()?;
    Some(SystemTime::now().duration_since(newest).ok()?.as_secs() / 86_400)
}

#[tauri::command]
pub async fn firmware() -> Result<FirmwareReport, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let has_fwupd = programs::which("fwupdmgr").is_some();
        let (devices, updates) = if has_fwupd {
            (
                fwupd(&["get-devices", "--json", "--no-unreported-check"]),
                // Only what fwupd already knows: fetching the list is a separate, explicit step.
                fwupd(&["get-updates", "--json", "--no-unreported-check", "--no-metadata-check"]),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let mut taken = Vec::new();
        let drives = block_devices()
            .into_iter()
            .map(|disk| {
                let dev = matching(&disk, &devices, &mut taken);
                let fwupd_id = dev.filter(|d| d.flags.iter().any(|f| f == "updatable")).map(|d| d.device_id.clone());
                let update = dev.and_then(|d| {
                    updates.iter().find(|u| u.device_id == d.device_id)?.releases.first().map(|r| r.version.clone())
                });
                let info = DriveFirmware {
                    issue: issue(&disk.model, &disk.rev),
                    maker: maker(&format!("{} {}", disk.vendor, disk.model)),
                    version: disk.rev,
                    fwupd_id,
                    update,
                };
                (disk.name, info)
            })
            .collect();
        FirmwareReport { fwupd: has_fwupd, metadata_age_days: metadata_age_days(), drives }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Fetches fwupd's list of firmware from fwupd.org.
#[tauri::command]
pub async fn firmware_refresh() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        let out = programs::run("fwupdmgr", &["refresh", "--force"])?;
        if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Installs fwupd's update for one drive. fwupd asks for the password.
#[tauri::command]
pub async fn firmware_update(device_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // Only a device fwupd itself offers an update for.
        let offered = fwupd(&["get-updates", "--json", "--no-unreported-check", "--no-metadata-check"])
            .iter()
            .any(|d| d.device_id == device_id);
        if !offered {
            return Err("fwupd has no update for this drive.".into());
        }
        let out = programs::run("fwupdmgr", &["update", &device_id, "--assume-yes", "--no-reboot-check"])?;
        let text = |b: &[u8]| String::from_utf8_lossy(b).trim().to_string();
        if out.status.success() {
            let said = text(&out.stdout);
            Ok(said.lines().last().unwrap_or("The firmware was updated.").to_string())
        } else {
            Err(text(&out.stderr))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

// Western Digital and SanDisk NVMe drives, from the files their Dashboard
// uses. The community method (github.com/not-a-feature/wd_ssd_firmware_update,
// GPL-3.0) worked out how: WD's index lists each model's firmware versions;
// each version's properties name its file and which installed versions it can
// go on top of. The drive itself checks WD's signature on the firmware.
// WD doesn't support updating this way.

const WD_SERVER: &str = "https://sddashboarddownloads.sandisk.com";

/// An NVMe drive's controller, model and firmware, from sysfs. `device` is a
/// namespace like nvme2n1.
fn nvme(device: &str) -> Result<(String, String, String), String> {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let controller = device
        .strip_prefix("nvme")
        .and_then(|rest| rest.split_once('n'))
        .filter(|(c, n)| digits(c) && digits(n))
        .map(|(c, _)| format!("nvme{c}"))
        .ok_or_else(|| format!("{device} isn't an NVMe drive."))?;
    let read = |f: &str| {
        fs::read_to_string(format!("/sys/class/nvme/{controller}/{f}"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    let (model, version) = (read("model"), read("firmware_rev"));
    if model.is_empty() || version.is_empty() {
        return Err(format!("The model and firmware of {device} can't be read."));
    }
    Ok((controller, model, version))
}

fn fetch(url: &str) -> Result<String, String> {
    let out = programs::run("curl", &["-fsSL", "--max-time", "30", "--proto", "=https", url])?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("Western Digital's server couldn't be reached: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// The text of each `<tag>…</tag>`. WD's index isn't well-formed XML, so it
/// is read as text, like the community scripts do.
fn tags<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        let Some(gt) = after.find('>') else { break };
        let body = &after[gt + 1..];
        let Some(end) = body.find(&close) else { break };
        out.push(body[..end].trim());
        rest = &body[end + close.len()..];
    }
    out
}

/// Each firmware version WD lists for a model, oldest first, with its
/// properties file.
fn wd_versions(index: &str, model: &str) -> Vec<(String, String)> {
    let dir = format!("/{}/", model.replace(' ', "_"));
    tags(index, "url")
        .into_iter()
        .filter(|u| u.contains(&dir) && u.ends_with("/device_properties.xml"))
        .filter_map(|u| {
            let version = u.trim_end_matches("/device_properties.xml").rsplit('/').next()?;
            Some((version.to_string(), u.to_string()))
        })
        .collect()
}

struct WdRelease {
    version: String,
    file: String,
    url: String,
    /// The installed versions it can be put on top of.
    after: Vec<String>,
    mode: String,
}

fn wd_release(props_path: &str, model: &str) -> Result<WdRelease, String> {
    let xml = fetch(&format!("{WD_SERVER}/{props_path}"))?;
    let one = |tag: &str| tags(&xml, tag).first().map(|s| s.to_string()).unwrap_or_default();
    let file = one("fwfile");
    // It becomes part of a URL and a file name: nothing but a plain name.
    if !file.ends_with(".fluf") || !file.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) {
        return Err("Western Digital's firmware listing looks wrong; nothing was changed.".into());
    }
    let wanted = format!("model=\"{model}\"");
    let after = xml
        .lines()
        .filter(|l| l.contains("<dependency") && l.contains(&wanted))
        .flat_map(|l| tags(l, "dependency"))
        .map(String::from)
        .collect();
    Ok(WdRelease {
        version: one("fwversion"),
        url: format!("{WD_SERVER}/{}", props_path.replace("device_properties.xml", &file)),
        file,
        after,
        mode: one("fwmode"),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WdCheck {
    current: String,
    /// WD's newest version for this model.
    latest: Option<String>,
    /// A version that can be installed now.
    install: Option<String>,
    note: String,
}

/// What WD offers for a drive, and the newest release that can go on top of
/// what it runs now. Contacts WD's server.
fn wd_plan(device: &str) -> Result<(WdCheck, Option<WdRelease>), String> {
    let (_, model, current) = nvme(device)?;
    let index = fetch(&format!("{WD_SERVER}/wdDashboard/config/devices/lista_devices.xml"))?;
    let versions = wd_versions(&index, &model);
    let latest = versions.last().map(|(v, _)| v.clone());
    let check = |install: Option<String>, note: String| WdCheck {
        current: current.clone(),
        latest: latest.clone(),
        install,
        note,
    };
    if versions.is_empty() {
        return Ok((check(None, format!("Western Digital lists no firmware for the {model}.")), None));
    }
    if latest.as_deref() == Some(current.as_str()) {
        return Ok((check(None, format!("{current} is the newest firmware Western Digital lists for it.")), None));
    }
    // The newest one that can go on top of what it runs now.
    for (version, path) in versions.iter().rev() {
        if *version == current {
            break;
        }
        let release = wd_release(path, &model)?;
        if release.after.contains(&current) {
            if release.mode != "mode3" {
                let note =
                    format!("{} needs an update method this app doesn't support ({}).", release.version, release.mode);
                return Ok((check(None, note), None));
            }
            let note = format!("{} can be installed over {current}.", release.version);
            return Ok((check(Some(release.version.clone()), note), Some(release)));
        }
    }
    let note = format!(
        "Western Digital's newer firmware can't be installed directly over {current}. SanDisk Dashboard on Windows can \
         step through the versions in between."
    );
    Ok((check(None, note), None))
}

#[tauri::command]
pub async fn wd_firmware_check(device: String) -> Result<WdCheck, String> {
    tauri::async_runtime::spawn_blocking(move || wd_plan(&device).map(|(c, _)| c)).await.map_err(|e| e.to_string())?
}

/// Whether the computer runs on battery right now. Batteries in mice and
/// keyboards (scope Device) don't count.
fn on_battery() -> bool {
    let Ok(dir) = fs::read_dir("/sys/class/power_supply") else { return false };
    let read =
        |p: &std::path::Path, f: &str| fs::read_to_string(p.join(f)).map(|s| s.trim().to_string()).unwrap_or_default();
    let supplies: Vec<_> = dir.flatten().map(|e| e.path()).collect();
    let has_battery = supplies.iter().any(|p| read(p, "type") == "Battery" && read(p, "scope") != "Device");
    let mains_on = supplies.iter().any(|p| read(p, "type") == "Mains" && read(p, "online") == "1");
    has_battery && !mains_on
}

/// Downloads WD's firmware for a drive and installs it with nvme-cli, to take
/// over at the next restart. Everything is looked up again here rather than
/// taken from the window. Asks for the password once.
#[tauri::command]
pub async fn wd_firmware_install(device: String, version: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if on_battery() {
            return Err("Plug the computer in first: a power cut while installing firmware can ruin the drive.".into());
        }
        programs::which("nvme").ok_or_else(|| programs::missing("nvme"))?;
        let (controller, model, current) = nvme(&device)?;
        let (_, release) = wd_plan(&device)?;
        let release = release
            .filter(|r| r.version == version)
            .ok_or_else(|| format!("Firmware {version} can't be installed on the {model} now; check again."))?;
        if !release.after.contains(&current) || release.version == current {
            return Err("The drive's firmware changed since the check; check again.".into());
        }

        // A folder only this user can read, in memory.
        let dir = xdg_runtime().join("open-task-manager").join("firmware");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let _ = fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o700));
        let file = dir.join(&release.file);
        let out = programs::run(
            "curl",
            &["-fsSL", "--max-time", "300", "--proto", "=https", "-o", &file.to_string_lossy(), &release.url],
        )?;
        if !out.status.success() || fs::metadata(&file).map(|m| m.len()).unwrap_or(0) == 0 {
            let _ = fs::remove_file(&file);
            return Err("The firmware couldn't be downloaded; nothing was changed.".into());
        }

        // Both steps under one password: copy the image to the drive, then
        // commit it to slot 2, to take over at the next restart (action 1).
        let dev = format!("/dev/{controller}");
        let path = file.to_string_lossy().into_owned();
        let script = "nvme fw-download --fw=\"$1\" \"$2\" && nvme fw-commit --slot=2 --action=1 \"$2\"";
        let out = programs::run("pkexec", &["sh", "-c", script, "sh", &path, &dev]);
        let _ = fs::remove_file(&file);
        let out = out?;
        if out.status.code() == Some(126) {
            return Err("Cancelled; nothing was changed.".into());
        }
        let said = format!("{} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        if out.status.success() {
            Ok(format!(
                "Firmware {} is on the drive. Restart the computer to switch to it; the Disks page shows the new \
                 version afterwards.",
                release.version
            ))
        } else {
            Err(format!("The drive didn't accept the firmware: {}", said.trim()))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

fn xdg_runtime() -> std::path::PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(model: &str, rev: &str) -> Disk {
        Disk { name: "nvme0n1".into(), model: model.into(), vendor: String::new(), rev: rev.into() }
    }

    fn dev(name: &str, id: &str, version: &str) -> FwupdDevice {
        FwupdDevice {
            name: name.into(),
            device_id: id.into(),
            version: version.into(),
            flags: vec![],
            releases: vec![],
        }
    }

    const INDEX: &str = r#"<lista_devices>
    <!-- SN850X -->
	<lista_device model="WD_BLACK SN850X 2000GB">
        <url>wdDashboard/firmware/WD_BLACK_SN850X_2000GB/620241WD/device_properties.xml</url>
        <url>wdDashboard/firmware/WD_BLACK_SN850X_2000GB/620361WD/device_properties.xml</url>
	<lista_device model="WD_BLACK SN850X 8000GB">
        <url>wdDashboard/firmware/WD_BLACK_SN850X_8000GB/638211WD/device_properties.xml</url>
"#;

    #[test]
    fn reads_wd_index() {
        let v = wd_versions(INDEX, "WD_BLACK SN850X 2000GB");
        assert_eq!(v.iter().map(|(v, _)| v.as_str()).collect::<Vec<_>>(), ["620241WD", "620361WD"]);
        assert_eq!(wd_versions(INDEX, "WD_BLACK SN850X 8000GB").len(), 1);
        assert!(wd_versions(INDEX, "WD_BLACK SN850X 1000GB").is_empty());
    }

    #[test]
    fn reads_tags() {
        let xml = r#"<dependency model="X">620241WD</dependency>
<dependency model="X">620281WD</dependency>"#;
        assert_eq!(tags(xml, "dependency"), ["620241WD", "620281WD"]);
        assert!(tags("<fwfile>a.fluf", "fwfile").is_empty());
    }

    #[test]
    fn names_nvme_controllers() {
        assert!(nvme("sda").is_err());
        assert!(nvme("nvme1n").is_err());
        assert!(nvme("nvme../n1").is_err());
    }

    #[test]
    fn finds_known_bad_firmware() {
        assert!(issue("Samsung SSD 980 PRO 2TB", "3B2QGXA7").is_some());
        assert!(issue("Samsung SSD 980 PRO 2TB", "5B2QGXA7").is_none());
        assert!(issue("Samsung SSD 990 PRO 2TB", "0B2QJXD7").is_some());
        assert!(issue("Samsung SSD 990 PRO 2TB", "4B2QJXD7").is_none());
    }

    #[test]
    fn knows_makers() {
        assert_eq!(maker("Samsung SSD 980 PRO 2TB").unwrap().name, "Samsung");
        assert_eq!(maker(" WD_BLACK SN850X 8000GB").unwrap().name, "Western Digital and SanDisk");
        assert_eq!(maker(" WDC WD40NMZW-11GX6S1").unwrap().name, "Western Digital and SanDisk");
        assert_eq!(maker(" CT4000P3PSSD8").unwrap().name, "Crucial");
        assert_eq!(maker("Seagate Expansion").unwrap().name, "Seagate");
        assert_eq!(maker("ATA ST4000DM004-2CV104").unwrap().name, "Seagate");
        assert!(maker(" JMS583").is_none());
        assert!(maker("Storage Device").is_none());
    }

    #[test]
    fn matches_fwupd_devices() {
        let devices = [
            dev("SSD 980 PRO 2TB", "a", "5B2QGXA7"),
            dev("SSD 980 PRO 2TB", "b", "5B2QGXA7"),
            dev("WD BLACK SN850X 8000GB", "c", "638211WD"),
        ];
        let mut taken = Vec::new();
        let samsung = disk("Samsung SSD 980 PRO 2TB", "5B2QGXA7");
        assert_eq!(matching(&samsung, &devices, &mut taken).unwrap().device_id, "a");
        // Its twin gets the other one.
        assert_eq!(matching(&samsung, &devices, &mut taken).unwrap().device_id, "b");
        assert_eq!(matching(&disk("WD_BLACK SN850X 8000GB", "638211WD"), &devices, &mut taken).unwrap().device_id, "c");
        // A different firmware version is a different drive.
        assert!(matching(&disk("WD_BLACK SN850X 8000GB", "620311WD"), &devices, &mut Vec::new()).is_none());
    }
}
