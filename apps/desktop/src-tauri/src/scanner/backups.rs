use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use rusqlite::Connection;

use crate::safety::journal;

use super::{ScanProgress, Scanner};

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
            if s.is_empty() { fallback.to_string() } else { s }
        }
        Ok(output) => {
            let combined = format!(
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let trimmed = combined.trim().to_string();
            if trimmed.is_empty() { fallback.to_string() } else { trimmed }
        }
        _ => fallback.to_string(),
    }
}

#[cfg(target_os = "windows")]
fn ps(script: &str) -> String {
    run_cmd("powershell", &["-NoProfile", "-Command", script], "")
}

// ── Check result helpers ────────────────────────────────────────────

struct RawCheck {
    id: &'static str,
    label: &'static str,
    status: &'static str, // "pass", "warn", or "fail"
    detail: String,
}

/// Convert raw checks into the scan_results tuple format used by journal.
fn checks_to_results(checks: &[RawCheck], generation: i64) -> Vec<(
    String,           // path (we use check id)
    Option<String>,   // category
    Option<String>,   // key (label)
    Option<f64>,      // value_num (100=pass, 50=warn, 0=fail)
    Option<String>,   // value_text (status string)
    Option<String>,   // metadata (detail)
    bool,             // stale
    i64,              // generation
)> {
    checks
        .iter()
        .map(|c| {
            let score = match c.status {
                "pass" => 100.0,
                "warn" => 50.0,
                _ => 0.0,
            };
            (
                c.id.to_string(),
                Some("backups".to_string()),
                Some(c.label.to_string()),
                Some(score),
                Some(c.status.to_string()),
                Some(c.detail.clone()),
                false,
                generation,
            )
        })
        .collect()
}

// ── macOS checks ────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn run_macos_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // Time Machine latest backup
    let latest = run_cmd("tmutil", &["latestbackup"], "");
    if latest.is_empty() || latest.contains("No backups") || latest.contains("error") {
        checks.push(RawCheck {
            id: "backups.timemachine",
            label: "Time Machine Backup",
            status: "fail",
            detail: "Not configured".to_string(),
        });
    } else {
        // Path looks like /Volumes/Backup/Backups.backupdb/host/2026-03-12-143022
        let date_part = latest
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_string();

        let hours_ago = chrono::NaiveDateTime::parse_from_str(&date_part, "%Y-%m-%d-%H%M%S")
            .ok()
            .and_then(|dt| {
                dt.and_utc()
                    .signed_duration_since(chrono::Utc::now())
                    .num_hours()
                    .checked_neg()
            });

        match hours_ago {
            Some(h) if h < 24 => {
                checks.push(RawCheck {
                    id: "backups.timemachine",
                    label: "Time Machine Backup",
                    status: "pass",
                    detail: format!("Last backup: {}h ago", h),
                });
            }
            Some(h) if h < 24 * 7 => {
                checks.push(RawCheck {
                    id: "backups.timemachine",
                    label: "Time Machine Backup",
                    status: "warn",
                    detail: format!("Last backup: {}d ago", h / 24),
                });
            }
            Some(h) => {
                checks.push(RawCheck {
                    id: "backups.timemachine",
                    label: "Time Machine Backup",
                    status: "fail",
                    detail: format!("Last backup: {}d ago", h / 24),
                });
            }
            None => {
                checks.push(RawCheck {
                    id: "backups.timemachine",
                    label: "Time Machine Backup",
                    status: "warn",
                    detail: "Could not parse backup date".to_string(),
                });
            }
        }
    }

    // Time Machine destination
    let dest = run_cmd("tmutil", &["destinationinfo"], "");
    let has_dest = !dest.is_empty()
        && !dest.contains("No destinations")
        && !dest.to_lowercase().contains("error");
    checks.push(RawCheck {
        id: "backups.timemachine_dest",
        label: "Backup Destination",
        status: if has_dest { "pass" } else { "fail" },
        detail: if has_dest {
            "Backup destination configured".to_string()
        } else {
            "No backup destination configured".to_string()
        },
    });

    checks
}

// ── Windows checks ──────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn run_windows_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // File History
    let fh = ps("try { $h = Get-WmiObject -Namespace 'root\\Microsoft\\Windows\\Storage' -Class MSFT_FileHistory -ErrorAction Stop; 'enabled' } catch { 'disabled' }");
    checks.push(RawCheck {
        id: "backups.filehistory",
        label: "File History",
        status: if fh.trim() == "enabled" { "pass" } else { "fail" },
        detail: if fh.trim() == "enabled" {
            "File History is enabled".to_string()
        } else {
            "File History is disabled".to_string()
        },
    });

    // Restore Points
    let rp = ps("(Get-ComputerRestorePoint | Measure-Object).Count");
    let count: i32 = rp.trim().parse().unwrap_or(0);
    checks.push(RawCheck {
        id: "backups.restore_points",
        label: "System Restore Points",
        status: if count > 0 { "pass" } else { "fail" },
        detail: if count > 0 {
            format!("{} restore point(s) available", count)
        } else {
            "No restore points found".to_string()
        },
    });

    checks
}

// ── Linux checks ────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn run_linux_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // Check for common backup tools: Timeshift, restic, borg, rsnapshot, duplicity
    let timeshift = run_cmd("sh", &["-c", "timeshift --list 2>/dev/null | head -5"], "");
    let has_timeshift = timeshift.contains("Snapshot") || timeshift.contains("snapshot");

    if has_timeshift {
        // Count snapshots
        let snap_count = run_cmd("sh", &["-c", "timeshift --list 2>/dev/null | grep -cE '^[0-9]' || echo 0"], "0");
        let count: i32 = snap_count.trim().parse().unwrap_or(0);
        checks.push(RawCheck {
            id: "backups.snapshots",
            label: "Timeshift Snapshots",
            status: if count > 0 { "pass" } else { "warn" },
            detail: if count > 0 {
                format!("{} snapshot(s) available", count)
            } else {
                "Timeshift installed but no snapshots found".to_string()
            },
        });
    } else {
        // Check if any common backup tool is installed
        let has_restic = std::path::Path::new("/usr/bin/restic").exists()
            || std::path::Path::new("/usr/local/bin/restic").exists();
        let has_borg = std::path::Path::new("/usr/bin/borg").exists()
            || std::path::Path::new("/usr/local/bin/borg").exists();
        let has_duplicity = std::path::Path::new("/usr/bin/duplicity").exists();
        let has_rsnapshot = std::path::Path::new("/usr/bin/rsnapshot").exists();
        let has_deja_dup = std::path::Path::new("/usr/bin/deja-dup").exists();

        if has_restic || has_borg || has_duplicity || has_rsnapshot || has_deja_dup {
            let tool = if has_borg { "Borg" }
                else if has_restic { "Restic" }
                else if has_deja_dup { "Deja Dup" }
                else if has_duplicity { "Duplicity" }
                else { "rsnapshot" };
            checks.push(RawCheck {
                id: "backups.tool",
                label: "Backup Tool",
                status: "pass",
                detail: format!("{} is installed", tool),
            });
        } else {
            checks.push(RawCheck {
                id: "backups.tool",
                label: "Backup Tool",
                status: "warn",
                detail: "No backup tool detected (Timeshift, Borg, Restic, Deja Dup)".to_string(),
            });
        }
    }

    checks
}

// ── BackupScanner ───────────────────────────────────────────────────

pub struct BackupScanner;

impl Scanner for BackupScanner {
    fn scan_type(&self) -> &str {
        "backups"
    }

    fn display_name(&self) -> &str {
        "Backup Check"
    }

    fn is_system_idle(&self) -> bool {
        true
    }

    fn tick(&self, _budget: Duration, conn: &Connection) -> Result<ScanProgress> {
        #[cfg(target_os = "macos")]
        let raw_checks = run_macos_checks();
        #[cfg(target_os = "windows")]
        let raw_checks = run_windows_checks();
        #[cfg(target_os = "linux")]
        let raw_checks = run_linux_checks();

        let count = raw_checks.len();
        let pass_count = raw_checks.iter().filter(|c| c.status == "pass").count();
        let results = checks_to_results(&raw_checks, 1);
        journal::upsert_scan_results(conn, "backups", &results)?;

        Ok(ScanProgress {
            progress_pct: 100,
            detail: format!("{}/{} checks passed", pass_count, count),
            done: true,
        })
    }
}
