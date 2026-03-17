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
                Some("performance".to_string()),
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
fn run_macos_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // Uptime
    let boottime = run_cmd("sysctl", &["-n", "kern.boottime"], "");
    // Format: { sec = 1741234567, usec = 0 }
    let boot_epoch = boottime
        .split("sec = ")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<i64>().ok());

    if let Some(epoch) = boot_epoch {
        let elapsed = chrono::Utc::now().timestamp() - epoch;
        let days = elapsed / 86400;
        let status = if days < 14 {
            "pass"
        } else if days < 30 {
            "warn"
        } else {
            "fail"
        };
        checks.push(RawCheck {
            id: "performance.uptime",
            label: "System Uptime",
            status,
            detail: format!("Last restart: {} days ago", days),
        });
    }

    // Disk free
    let df_output = run_cmd("df", &["-Ph", "/"], "");
    for line in df_output.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 5 {
            let capacity_str = cols[4].trim_end_matches('%');
            let avail = cols[3];
            if let Ok(used_pct) = capacity_str.parse::<u32>() {
                let free_pct = 100u32.saturating_sub(used_pct);
                let status = if used_pct < 80 {
                    "pass"
                } else if used_pct < 90 {
                    "warn"
                } else {
                    "fail"
                };
                checks.push(RawCheck {
                    id: "performance.disk_free",
                    label: "Disk Space",
                    status,
                    detail: format!("{}% free ({} available)", free_pct, avail),
                });
            }
            break;
        }
    }

    // Startup items
    let home = std::env::var("HOME").unwrap_or_default();
    let home = std::path::PathBuf::from(&home);
    let user_agents = count_plist_files(&home.join("Library/LaunchAgents"));
    let system_agents = count_plist_files(std::path::Path::new("/Library/LaunchAgents"));
    let system_daemons = count_plist_files(std::path::Path::new("/Library/LaunchDaemons"));
    let total = user_agents + system_agents + system_daemons;
    let status = if total < 30 {
        "pass"
    } else if total < 50 {
        "warn"
    } else {
        "fail"
    };
    checks.push(RawCheck {
        id: "performance.startup_items",
        label: "Startup Items",
        status,
        detail: format!("{} startup items", total),
    });

    // Memory (RAM)
    let memsize = run_cmd("sysctl", &["-n", "hw.memsize"], "");
    if let Ok(bytes) = memsize.trim().parse::<u64>() {
        let gb = bytes / (1024 * 1024 * 1024);
        let status = if gb >= 8 {
            "pass"
        } else if gb >= 4 {
            "warn"
        } else {
            "fail"
        };
        checks.push(RawCheck {
            id: "performance.memory",
            label: "System Memory",
            status,
            detail: format!("{} GB RAM", gb),
        });
    }

    checks
}

// ── Windows checks ──────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn run_windows_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // Uptime
    let days_str = ps("((Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime).TotalDays");
    if let Ok(total_days) = days_str.trim().parse::<f64>() {
        let days = total_days as i64;
        let status = if days < 14 {
            "pass"
        } else if days < 30 {
            "warn"
        } else {
            "fail"
        };
        checks.push(RawCheck {
            id: "performance.uptime",
            label: "System Uptime",
            status,
            detail: format!("Last restart: {} days ago", days),
        });
    }

    // Disk free
    let disk = ps("$d = Get-PSDrive -Name C; $total = $d.Used + $d.Free; if ($total -gt 0) { [math]::Round($d.Used / $total * 100) } else { 0 }");
    if let Ok(used_pct) = disk.trim().parse::<u32>() {
        let free_pct = 100u32.saturating_sub(used_pct);
        let avail_gb = ps("[math]::Round((Get-PSDrive -Name C).Free / 1GB)");
        let status = if used_pct < 80 {
            "pass"
        } else if used_pct < 90 {
            "warn"
        } else {
            "fail"
        };
        checks.push(RawCheck {
            id: "performance.disk_free",
            label: "Disk Space",
            status,
            detail: format!("{}% free ({} GB available)", free_pct, avail_gb.trim()),
        });
    }

    // Startup items
    let count_str = ps("(Get-CimInstance Win32_StartupCommand | Measure-Object).Count");
    if let Ok(total) = count_str.trim().parse::<u32>() {
        let status = if total < 30 {
            "pass"
        } else if total < 50 {
            "warn"
        } else {
            "fail"
        };
        checks.push(RawCheck {
            id: "performance.startup_items",
            label: "Startup Items",
            status,
            detail: format!("{} startup items", total),
        });
    }

    // Memory
    let mem_str = ps("[math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)");
    if let Ok(gb) = mem_str.trim().parse::<u64>() {
        let status = if gb >= 8 {
            "pass"
        } else if gb >= 4 {
            "warn"
        } else {
            "fail"
        };
        checks.push(RawCheck {
            id: "performance.memory",
            label: "System Memory",
            status,
            detail: format!("{} GB RAM", gb),
        });
    }

    checks
}

// ── Linux checks ────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn run_linux_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // Uptime
    let raw = run_cmd("uptime", &["-s"], "");
    if let Ok(boot) = chrono::NaiveDateTime::parse_from_str(&raw, "%Y-%m-%d %H:%M:%S") {
        let elapsed = chrono::Utc::now().timestamp() - boot.and_utc().timestamp();
        let days = elapsed / 86400;
        let status = if days < 14 {
            "pass"
        } else if days < 30 {
            "warn"
        } else {
            "fail"
        };
        checks.push(RawCheck {
            id: "performance.uptime",
            label: "System Uptime",
            status,
            detail: format!("Last restart: {} days ago", days),
        });
    }

    // Disk free
    let df_output = run_cmd("df", &["-Ph", "/"], "");
    for line in df_output.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 5 {
            let capacity_str = cols[4].trim_end_matches('%');
            let avail = cols[3];
            if let Ok(used_pct) = capacity_str.parse::<u32>() {
                let free_pct = 100u32.saturating_sub(used_pct);
                let status = if used_pct < 80 {
                    "pass"
                } else if used_pct < 90 {
                    "warn"
                } else {
                    "fail"
                };
                checks.push(RawCheck {
                    id: "performance.disk_free",
                    label: "Disk Space",
                    status,
                    detail: format!("{}% free ({} available)", free_pct, avail),
                });
            }
            break;
        }
    }

    checks
}

// ── PerformanceScanner ──────────────────────────────────────────────

pub struct PerformanceScanner;

impl Scanner for PerformanceScanner {
    fn scan_type(&self) -> &str {
        "performance"
    }

    fn display_name(&self) -> &str {
        "Performance Check"
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
        journal::upsert_scan_results(conn, "performance", &results)?;

        Ok(ScanProgress {
            progress_pct: 100,
            detail: format!("{}/{} checks passed", pass_count, count),
            done: true,
        })
    }
}
