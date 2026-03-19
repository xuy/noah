use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

const CACHE_FILE: &str = "system_snapshot.json";
const STALE_HOURS: i64 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    #[serde(default)]
    pub backup_status: String,
    #[serde(default)]
    pub security_posture: String,
    #[serde(default)]
    pub disk_free: String,
    #[serde(default)]
    pub startup_items: String,
    #[serde(default)]
    pub uptime: String,
    pub gathered_at: String,
}

/// Run a command and return trimmed stdout, or `fallback` on any failure.
fn run_cmd(program: &str, args: &[&str], fallback: &str) -> String {
    let mut cmd = Command::new(program);
    cmd.args(args);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.output() {
        Ok(output) if output.status.success() => {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if s.is_empty() {
                fallback.to_string()
            } else {
                s
            }
        }
        _ => fallback.to_string(),
    }
}

// ── macOS gatherers ──────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn gather_backup() -> String {
    let latest = run_cmd("tmutil", &["latestbackup"], "");
    if latest.is_empty() {
        return "Time Machine: not configured".to_string();
    }

    // Path looks like /Volumes/Backup/Backups.backupdb/host/2026-03-12-143022
    // Extract the date component from the last path segment.
    let date_part = latest
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();

    // Try to parse the date segment (YYYY-MM-DD-HHMMSS).
    let ago = chrono::NaiveDateTime::parse_from_str(&date_part, "%Y-%m-%d-%H%M%S")
        .ok()
        .and_then(|dt| dt.and_utc().signed_duration_since(Utc::now()).num_hours().checked_neg())
        .map(|h| {
            if h < 1 {
                "less than 1h ago".to_string()
            } else if h < 24 {
                format!("{}h ago", h)
            } else {
                format!("{}d ago", h / 24)
            }
        })
        .unwrap_or_default();

    // Get destination name.
    let dest_info = run_cmd("tmutil", &["destinationinfo"], "");
    let dest_name = dest_info
        .lines()
        .find_map(|l| l.strip_prefix("Name").map(|v| v.trim().trim_start_matches(':').trim().to_string()))
        .unwrap_or_default();

    let mut result = "Time Machine: last backup".to_string();
    if !ago.is_empty() {
        result.push_str(&format!(" {}", ago));
    }
    if !dest_name.is_empty() {
        result.push_str(&format!(" to \"{}\"", dest_name));
    }
    result
}

#[cfg(target_os = "macos")]
fn gather_security() -> String {
    let mut parts = Vec::new();

    // FileVault
    let fv = run_cmd("fdesetup", &["status"], "");
    if fv.contains("On") {
        parts.push("FileVault on");
    } else if fv.contains("Off") {
        parts.push("FileVault off");
    }

    // Firewall
    let fw = run_cmd(
        "/usr/libexec/ApplicationFirewall/socketfilterfw",
        &["--getglobalstate"],
        "",
    );
    if fw.contains("enabled") {
        parts.push("Firewall on");
    } else if fw.contains("disabled") {
        parts.push("Firewall off");
    }

    // SIP
    let sip = run_cmd("csrutil", &["status"], "");
    if sip.contains("enabled") {
        parts.push("SIP enabled");
    } else if sip.contains("disabled") {
        parts.push("SIP disabled");
    }

    if parts.is_empty() {
        String::new()
    } else {
        parts.join(", ")
    }
}

#[cfg(target_os = "macos")]
fn gather_disk_free() -> String {
    let df_output = run_cmd("df", &["-Ph", "/"], "");
    format_macos_main_disk_free(&df_output)
}

#[cfg(target_os = "macos")]
fn format_macos_main_disk_free(df_output: &str) -> String {
    for line in df_output.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        // df -Ph: Filesystem Size Used Avail Capacity Mounted
        if cols.len() < 6 {
            continue;
        }
        let mount = cols[5..].join(" ");
        let capacity_str = cols[4].trim_end_matches('%');
        let avail = cols[3];

        // The proactive snapshot should only reflect the boot volume.
        if mount != "/" {
            continue;
        }

        if let Ok(used_pct) = capacity_str.parse::<u32>() {
            let free_pct = 100u32.saturating_sub(used_pct);
            return format!("Macintosh HD {}% free ({} avail)", free_pct, avail);
        }
    }

    String::new()
}

#[cfg(target_os = "macos")]
fn count_plist_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map_or(false, |ext| ext == "plist")
                })
                .count()
        })
        .unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn gather_startup_items() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let home = std::path::PathBuf::from(&home);

    // Count launch agents (user + system).
    let user_agents = count_plist_files(&home.join("Library/LaunchAgents"));
    let system_agents = count_plist_files(std::path::Path::new("/Library/LaunchAgents"));

    // Count launch daemons (system-level).
    let system_daemons = count_plist_files(std::path::Path::new("/Library/LaunchDaemons"));

    let mut parts = Vec::new();
    let agents = user_agents + system_agents;
    if agents > 0 {
        parts.push(format!("{} launch agents", agents));
    }
    if system_daemons > 0 {
        parts.push(format!("{} launch daemons", system_daemons));
    }

    parts.join(", ")
}

#[cfg(target_os = "macos")]
fn gather_uptime() -> String {
    let boottime = run_cmd("sysctl", &["-n", "kern.boottime"], "");
    // Format: { sec = 1741234567, usec = 0 }
    let secs = boottime
        .split("sec = ")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<i64>().ok());

    let Some(boot_epoch) = secs else {
        return String::new();
    };

    let elapsed = Utc::now().timestamp() - boot_epoch;
    let days = elapsed / 86400;
    let hours = (elapsed % 86400) / 3600;

    if days > 0 {
        format!("{} days {} hours", days, hours)
    } else {
        format!("{} hours", hours)
    }
}

#[cfg(target_os = "macos")]
fn gather_snapshot() -> SystemSnapshot {
    SystemSnapshot {
        backup_status: gather_backup(),
        security_posture: gather_security(),
        disk_free: gather_disk_free(),
        startup_items: gather_startup_items(),
        uptime: gather_uptime(),
        gathered_at: Utc::now().to_rfc3339(),
    }
}

// ── Windows gatherers ────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn ps(script: &str) -> String {
    run_cmd("powershell", &["-NoProfile", "-Command", script], "")
}

#[cfg(target_os = "windows")]
fn gather_snapshot() -> SystemSnapshot {
    let backup_status = {
        let status = ps(
            "try { $h = Get-WmiObject -Namespace 'root\\Microsoft\\Windows\\Storage' -Class MSFT_FileHistory -ErrorAction Stop; 'File History: enabled' } catch { 'File History: not configured' }",
        );
        if status.is_empty() {
            "File History: not configured".to_string()
        } else {
            status
        }
    };

    let security_posture = {
        let mut parts = Vec::new();
        let defender = ps("try { (Get-MpComputerStatus).RealTimeProtectionEnabled } catch { '' }");
        match defender.to_lowercase().as_str() {
            "true" => parts.push("Defender on"),
            "false" => parts.push("Defender off"),
            _ => {}
        }
        let bitlocker = ps("try { (Get-BitLockerVolume -MountPoint 'C:').ProtectionStatus } catch { '' }");
        match bitlocker.trim() {
            "On" | "1" => parts.push("BitLocker on"),
            "Off" | "0" => parts.push("BitLocker off"),
            _ => {}
        }
        parts.join(", ")
    };

    let disk_free = {
        let raw = ps(
            "Get-PSDrive -PSProvider FileSystem | Where-Object { $_.Used -ne $null } | ForEach-Object { $total = $_.Used + $_.Free; $pct = if ($total -gt 0) { [math]::Round($_.Free / $total * 100) } else { 0 }; \"$($_.Name): $pct% free ($([math]::Round($_.Free / 1GB)) GB)\" }",
        );
        raw.lines().collect::<Vec<_>>().join(", ")
    };

    let startup_items = {
        let count = ps("(Get-CimInstance Win32_StartupCommand | Measure-Object).Count");
        if let Ok(n) = count.trim().parse::<u32>() {
            format!("{} startup items", n)
        } else {
            String::new()
        }
    };

    let uptime = {
        let hours_str = ps(
            "((Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime).TotalHours",
        );
        if let Ok(total_hours) = hours_str.trim().parse::<f64>() {
            let h = total_hours as i64;
            let days = h / 24;
            let hrs = h % 24;
            if days > 0 {
                format!("{} days {} hours", days, hrs)
            } else {
                format!("{} hours", hrs)
            }
        } else {
            String::new()
        }
    };

    SystemSnapshot {
        backup_status,
        security_posture,
        disk_free,
        startup_items,
        uptime,
        gathered_at: Utc::now().to_rfc3339(),
    }
}

// ── Linux gatherers ──────────────────────────────────────────────────

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn gather_snapshot() -> SystemSnapshot {
    let disk_free = {
        let df_output = run_cmd("df", &["-Ph"], "");
        let mut volumes = Vec::new();
        for line in df_output.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 6 {
                continue;
            }
            let mount = cols[5];
            let capacity_str = cols[4].trim_end_matches('%');
            let avail = cols[3];
            // Only show real mounts.
            if !cols[0].starts_with('/') {
                continue;
            }
            if let Ok(used_pct) = capacity_str.parse::<u32>() {
                let free_pct = 100u32.saturating_sub(used_pct);
                volumes.push(format!("{} {}% free ({} avail)", mount, free_pct, avail));
            }
        }
        volumes.join(", ")
    };

    let uptime = {
        let raw = run_cmd("uptime", &["-s"], "");
        if let Ok(boot) = chrono::NaiveDateTime::parse_from_str(&raw, "%Y-%m-%d %H:%M:%S") {
            let elapsed = Utc::now().timestamp() - boot.and_utc().timestamp();
            let days = elapsed / 86400;
            let hours = (elapsed % 86400) / 3600;
            if days > 0 {
                format!("{} days {} hours", days, hours)
            } else {
                format!("{} hours", hours)
            }
        } else {
            String::new()
        }
    };

    SystemSnapshot {
        backup_status: String::new(),
        security_posture: String::new(),
        disk_free,
        startup_items: String::new(),
        uptime,
        gathered_at: Utc::now().to_rfc3339(),
    }
}

// ── SystemSnapshot impl ─────────────────────────────────────────────

impl SystemSnapshot {
    /// Gather a fresh snapshot from the current system.
    pub fn gather() -> Self {
        gather_snapshot()
    }

    /// Load cached snapshot from disk.
    pub fn load(app_dir: &Path) -> Option<Self> {
        let path = app_dir.join(CACHE_FILE);
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Save snapshot to disk.
    pub fn save(&self, app_dir: &Path) {
        let path = app_dir.join(CACHE_FILE);
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    eprintln!("[warn] Failed to save system snapshot: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[warn] Failed to serialize system snapshot: {}", e);
            }
        }
    }

    /// True if gathered_at is >6h ago or unparseable.
    pub fn is_stale(&self) -> bool {
        match self.gathered_at.parse::<DateTime<Utc>>() {
            Ok(ts) => Utc::now().signed_duration_since(ts).num_hours() >= STALE_HOURS,
            Err(_) => true,
        }
    }

    /// Load from cache if available, otherwise gather and save.
    pub fn load_or_gather(app_dir: &Path) -> Self {
        if let Some(cached) = Self::load(app_dir) {
            return cached;
        }
        let snap = Self::gather();
        snap.save(app_dir);
        snap
    }

    /// Refresh the cache if stale. Call from a background thread.
    pub fn refresh_if_stale(app_dir: &Path) {
        if let Some(cached) = Self::load(app_dir) {
            if !cached.is_stale() {
                return;
            }
        }
        let snap = Self::gather();
        snap.save(app_dir);
    }

    /// Format for the system prompt. Omits fields that are empty.
    pub fn to_prompt_string(&self) -> String {
        let fields: &[(&str, &str)] = &[
            ("Backup", &self.backup_status),
            ("Security", &self.security_posture),
            ("Disk", &self.disk_free),
            ("Startup Items", &self.startup_items),
            ("Uptime", &self.uptime),
        ];

        let lines: Vec<String> = fields
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(label, value)| format!("{}: {}", label, value))
            .collect();

        lines.join("\n")
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn gather_always_has_gathered_at() {
        let snap = SystemSnapshot::gather();
        assert!(!snap.gathered_at.is_empty());
        assert!(snap.gathered_at.parse::<DateTime<Utc>>().is_ok());
    }

    #[test]
    fn prompt_string_omits_empty_fields() {
        let snap = SystemSnapshot {
            backup_status: "Time Machine: last backup 2h ago".to_string(),
            security_posture: String::new(),
            disk_free: "Macintosh HD 45% free (210 GB)".to_string(),
            startup_items: String::new(),
            uptime: String::new(),
            gathered_at: Utc::now().to_rfc3339(),
        };
        let prompt = snap.to_prompt_string();
        assert!(prompt.contains("Backup: Time Machine"));
        assert!(prompt.contains("Disk: Macintosh HD"));
        assert!(!prompt.contains("Security"));
        assert!(!prompt.contains("Startup"));
        assert!(!prompt.contains("Uptime"));
    }

    #[test]
    fn prompt_string_empty_when_all_unknown() {
        let snap = SystemSnapshot {
            backup_status: String::new(),
            security_posture: String::new(),
            disk_free: String::new(),
            startup_items: String::new(),
            uptime: String::new(),
            gathered_at: Utc::now().to_rfc3339(),
        };
        assert!(snap.to_prompt_string().is_empty());
    }

    #[test]
    fn round_trip_json_serialization() {
        let snap = SystemSnapshot::gather();
        let json = serde_json::to_string_pretty(&snap).unwrap();
        let deserialized: SystemSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap.gathered_at, deserialized.gathered_at);
        assert_eq!(snap.disk_free, deserialized.disk_free);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join("noah_test_system_snapshot");
        let _ = fs::create_dir_all(&dir);
        let snap = SystemSnapshot::gather();
        snap.save(&dir);
        let loaded = SystemSnapshot::load(&dir).expect("should load saved snapshot");
        assert_eq!(snap.gathered_at, loaded.gathered_at);
        assert_eq!(snap.backup_status, loaded.backup_status);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_none_for_missing_file() {
        let dir = std::env::temp_dir().join("noah_test_missing_snap");
        let _ = fs::remove_dir_all(&dir);
        assert!(SystemSnapshot::load(&dir).is_none());
    }

    #[test]
    fn fresh_snapshot_is_not_stale() {
        let snap = SystemSnapshot::gather();
        assert!(!snap.is_stale());
    }

    #[test]
    fn stale_after_threshold() {
        let old_time = Utc::now() - chrono::Duration::hours(7);
        let snap = SystemSnapshot {
            backup_status: String::new(),
            security_posture: String::new(),
            disk_free: String::new(),
            startup_items: String::new(),
            uptime: String::new(),
            gathered_at: old_time.to_rfc3339(),
        };
        assert!(snap.is_stale());
    }

    #[test]
    fn not_stale_within_threshold() {
        let recent = Utc::now() - chrono::Duration::hours(5);
        let snap = SystemSnapshot {
            backup_status: String::new(),
            security_posture: String::new(),
            disk_free: String::new(),
            startup_items: String::new(),
            uptime: String::new(),
            gathered_at: recent.to_rfc3339(),
        };
        assert!(!snap.is_stale());
    }

    #[test]
    fn print_live_snapshot() {
        let snap = SystemSnapshot::gather();
        let prompt = snap.to_prompt_string();
        eprintln!("\n=== LIVE SNAPSHOT PROMPT ===\n{}\n===========================\n", prompt);
        // Just ensure it doesn't panic and produces some output on a real system.
        assert!(!snap.gathered_at.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_disk_summary_only_reports_root_volume() {
        let df_output = "\
Filesystem        Size    Used   Avail Capacity  Mounted on
/dev/disk3s1s1   460Gi    15Gi    79Gi    17%    /
/dev/disk3s5     460Gi   347Gi    79Gi    82%    /System/Volumes/Data
/dev/disk4s1     749Mi   744Mi   5.1Mi   100%    /Volumes/Antigravity
";

        assert_eq!(
            format_macos_main_disk_free(df_output),
            "Macintosh HD 83% free (79Gi avail)"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_disk_summary_ignores_non_root_only_output() {
        let df_output = "\
Filesystem        Size    Used   Avail Capacity  Mounted on
/dev/disk4s1     749Mi   744Mi   5.1Mi   100%    /Volumes/Antigravity
";

        assert!(format_macos_main_disk_free(df_output).is_empty());
    }
}
