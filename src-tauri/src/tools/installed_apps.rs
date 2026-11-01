//! Installed software, from the package managers on this system: pacman,
//! dpkg, rpm and Flatpak.

use std::collections::HashMap;

use serde::Serialize;

use crate::programs;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    name: String,
    version: String,
    description: String,
    size: u64,
    source: &'static str,
    /// Installed on purpose rather than pulled in by something else.
    explicit: bool,
    url: String,
    installed: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppList {
    apps: Vec<App>,
    sources: Vec<&'static str>,
}

#[tauri::command]
pub async fn installed_apps() -> Result<AppList, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut apps = Vec::new();
        let mut sources = Vec::new();
        if programs::which("pacman").is_some() {
            apps.extend(pacman()?);
            sources.push("pacman");
        }
        if programs::which("flatpak").is_some() {
            apps.extend(flatpak());
            sources.push("flatpak");
        }
        if programs::which("dpkg-query").is_some() && programs::which("pacman").is_none() {
            apps.extend(dpkg());
            sources.push("dpkg");
        }
        // Arch and Debian can have the rpm tool without an rpm database in use.
        let rpm_db = ["/usr/lib/sysimage/rpm", "/var/lib/rpm"].iter().any(|d| {
            std::path::Path::new(d).join("rpmdb.sqlite").exists() || std::path::Path::new(d).join("Packages").exists()
        });
        if rpm_db && programs::which("rpm").is_some() && sources.iter().all(|s| *s == "flatpak") {
            apps.extend(rpm());
            sources.push("rpm");
        }
        apps.sort_by_key(|a| a.name.to_lowercase());
        Ok(AppList { apps, sources })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Runs a tool with English output so the labels can be parsed.
fn run_c(program: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("Could not run {program}: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn pacman() -> Result<Vec<App>, String> {
    let text = run_c("pacman", &["-Qi"])?;
    Ok(text
        .split("\n\n")
        .filter_map(|record| {
            let mut fields = HashMap::new();
            for line in record.lines() {
                // "Name            : bash"; continuation lines start with spaces.
                if let Some((key, value)) = line.split_once(" : ")
                    && !key.starts_with(' ')
                {
                    fields.insert(key.trim(), value.trim());
                }
            }
            let get = |k: &str| fields.get(k).copied().unwrap_or_default().to_string();
            let name = get("Name");
            (!name.is_empty()).then(|| App {
                version: get("Version"),
                description: get("Description"),
                size: parse_size(&get("Installed Size")),
                source: "pacman",
                explicit: get("Install Reason").starts_with("Explicitly"),
                url: get("URL").replace("None", ""),
                installed: get("Install Date"),
                name,
            })
        })
        .collect())
}

fn flatpak() -> Vec<App> {
    let text =
        run_c("flatpak", &["list", "--app", "--columns=application,name,version,size,origin"]).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            (f.len() >= 5).then(|| App {
                name: f[1].to_string(),
                version: f[2].to_string(),
                description: f[0].to_string(),
                size: parse_size(f[3]),
                source: "flatpak",
                explicit: true,
                url: format!("https://flathub.org/apps/{}", f[0]),
                installed: format!("from {}", f[4]),
            })
        })
        .collect()
}

fn dpkg() -> Vec<App> {
    let text =
        run_c("dpkg-query", &["-W", "-f=${Package}\t${Version}\t${Installed-Size}\t${binary:Summary}\t${Homepage}\n"])
            .unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            (f.len() >= 5).then(|| App {
                name: f[0].to_string(),
                version: f[1].to_string(),
                description: f[3].to_string(),
                size: f[2].parse::<u64>().unwrap_or(0) * 1024,
                source: "dpkg",
                explicit: true,
                url: f[4].to_string(),
                installed: String::new(),
            })
        })
        .collect()
}

fn rpm() -> Vec<App> {
    let text = run_c(
        "rpm",
        &["-qa", "--qf", "%{NAME}\t%{VERSION}-%{RELEASE}\t%{SIZE}\t%{SUMMARY}\t%{URL}\t%{INSTALLTIME:date}\n"],
    )
    .unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            // GPG keys are listed as packages too.
            (f.len() >= 6 && f[0] != "gpg-pubkey").then(|| App {
                name: f[0].to_string(),
                version: f[1].to_string(),
                description: f[3].to_string(),
                size: f[2].parse().unwrap_or(0),
                source: "rpm",
                explicit: true,
                url: f[4].replace("(none)", ""),
                installed: f[5].to_string(),
            })
        })
        .collect()
}

/// "9.68 MiB", "747.2 MB", "12 KiB" → bytes.
fn parse_size(s: &str) -> u64 {
    let mut parts = s.split_whitespace();
    let Some(value) = parts.next().and_then(|v| v.replace(',', ".").parse::<f64>().ok()) else { return 0 };
    let unit = match parts.next().unwrap_or("B") {
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "kB" | "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        _ => 1.0,
    };
    (value * unit) as u64
}

#[cfg(test)]
mod tests {
    use super::parse_size;

    #[test]
    fn parses_sizes_from_both_tools() {
        assert_eq!(parse_size("9.68 MiB"), (9.68 * 1024.0 * 1024.0) as u64);
        assert_eq!(parse_size("747.2 MB"), 747_200_000);
        assert_eq!(parse_size("0.00 B"), 0);
    }
}
