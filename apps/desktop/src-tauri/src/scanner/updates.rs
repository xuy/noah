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
        _ => fallback.to_string(),
    }
}

#[cfg(target_os = "windows")]
fn ps(script: &str) -> String {
    run_cmd("powershell", &["-NoProfile", "-Command", script], "")
}

struct RawCheck {
    id: &'static str,
    label: &'static str,
    status: &'static str,
    detail: String,
}

fn checks_to_results(checks: &[RawCheck], generation: i64) -> Vec<(
    String, Option<String>, Option<String>, Option<f64>,
    Option<String>, Option<String>, bool, i64,
)> {
    checks.iter().map(|c| {
        let score = match c.status {
            "pass" => 100.0,
            "warn" => 50.0,
            _ => 0.0,
        };
        (
            c.id.to_string(),
            Some("updates".to_string()),
            Some(c.label.to_string()),
            Some(score),
            Some(c.status.to_string()),
            Some(c.detail.clone()),
            false,
            generation,
        )
    }).collect()
}

// ── macOS checks ────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn run_macos_checks(budget: Duration) -> Vec<RawCheck> {
    let start = std::time::Instant::now();
    let mut checks = Vec::new();

    // OS updates — softwareupdate -l can be slow (10-30s).
    // Run with a timeout so we don't blow the budget.
    let su_output = {
        let mut cmd = Command::new("softwareupdate");
        cmd.args(["-l"]);
        match cmd.output() {
            Ok(output) if start.elapsed() < budget => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                format!("{}\n{}", stdout, stderr)
            }
            _ => String::new(),
        }
    };

    if su_output.is_empty() {
        checks.push(RawCheck {
            id: "updates.os",
            label: "macOS Updates",
            status: "warn",
            detail: "Could not check for updates".to_string(),
        });
    } else {
        let lower = su_output.to_lowercase();
        let has_security = lower.contains("security") || lower.contains("critical");
        let has_updates = lower.contains("* label:");

        if has_security {
            checks.push(RawCheck {
                id: "updates.os",
                label: "macOS Updates",
                status: "fail",
                detail: "Security update available".to_string(),
            });
        } else if has_updates {
            checks.push(RawCheck {
                id: "updates.os",
                label: "macOS Updates",
                status: "warn",
                detail: "Updates available".to_string(),
            });
        } else {
            checks.push(RawCheck {
                id: "updates.os",
                label: "macOS Updates",
                status: "pass",
                detail: "Up to date".to_string(),
            });
        }
    }

    // Homebrew — only if installed.
    if start.elapsed() < budget {
        let brew_path = if std::path::Path::new("/opt/homebrew/bin/brew").exists() {
            "/opt/homebrew/bin/brew"
        } else if std::path::Path::new("/usr/local/bin/brew").exists() {
            "/usr/local/bin/brew"
        } else {
            ""
        };

        if !brew_path.is_empty() {
            let outdated = run_cmd(brew_path, &["outdated", "--json=v2"], "");
            if outdated.is_empty() || outdated == "{}" {
                checks.push(RawCheck {
                    id: "updates.brew",
                    label: "Homebrew Packages",
                    status: "pass",
                    detail: "All packages up to date".to_string(),
                });
            } else if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&outdated) {
                let formula_count = parsed.get("formulae")
                    .and_then(|f| f.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let cask_count = parsed.get("casks")
                    .and_then(|c| c.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let total = formula_count + cask_count;

                if total == 0 {
                    checks.push(RawCheck {
                        id: "updates.brew",
                        label: "Homebrew Packages",
                        status: "pass",
                        detail: "All packages up to date".to_string(),
                    });
                } else {
                    checks.push(RawCheck {
                        id: "updates.brew",
                        label: "Homebrew Packages",
                        status: if total >= 10 { "fail" } else { "warn" },
                        detail: format!("{} outdated package{}", total, if total == 1 { "" } else { "s" }),
                    });
                }
            } else {
                checks.push(RawCheck {
                    id: "updates.brew",
                    label: "Homebrew Packages",
                    status: "warn",
                    detail: "Could not parse brew output".to_string(),
                });
            }
        }
    }

    checks
}

// ── Windows checks ──────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn run_windows_checks(_budget: Duration) -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // Last hotfix date
    let hotfix = ps(
        "(Get-HotFix | Sort-Object InstalledOn -Descending | Select-Object -First 1).InstalledOn.ToString('yyyy-MM-dd')"
    );
    let trimmed = hotfix.trim();
    if trimmed.is_empty() || trimmed.starts_with("Get-") {
        checks.push(RawCheck {
            id: "updates.os",
            label: "Windows Updates",
            status: "warn",
            detail: "Could not determine last update".to_string(),
        });
    } else {
        // Parse date and check if recent (< 30 days)
        let days_old = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
            .ok()
            .map(|d| (chrono::Utc::now().date_naive() - d).num_days())
            .unwrap_or(999);

        if days_old <= 30 {
            checks.push(RawCheck {
                id: "updates.os",
                label: "Windows Updates",
                status: "pass",
                detail: format!("Last update: {} ({} days ago)", trimmed, days_old),
            });
        } else if days_old <= 90 {
            checks.push(RawCheck {
                id: "updates.os",
                label: "Windows Updates",
                status: "warn",
                detail: format!("Last update: {} ({} days ago)", trimmed, days_old),
            });
        } else {
            checks.push(RawCheck {
                id: "updates.os",
                label: "Windows Updates",
                status: "fail",
                detail: format!("Last update: {} ({} days ago)", trimmed, days_old),
            });
        }
    }

    checks
}

// ── Linux checks ────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn run_linux_checks(_budget: Duration) -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // Try apt (Debian/Ubuntu), then dnf (Fedora/RHEL), then pacman (Arch)
    let (count, pkg_mgr) = if std::path::Path::new("/usr/bin/apt").exists() {
        let out = run_cmd("sh", &["-c", "apt list --upgradable 2>/dev/null | grep -c upgradable"], "");
        (out.trim().parse::<i32>().unwrap_or(-1), "apt")
    } else if std::path::Path::new("/usr/bin/dnf").exists() {
        let out = run_cmd("sh", &["-c", "dnf check-update --quiet 2>/dev/null | grep -cE '^[a-zA-Z]' || echo 0"], "0");
        (out.trim().parse::<i32>().unwrap_or(-1), "dnf")
    } else if std::path::Path::new("/usr/bin/pacman").exists() {
        let out = run_cmd("sh", &["-c", "pacman -Qu 2>/dev/null | wc -l"], "");
        (out.trim().parse::<i32>().unwrap_or(-1), "pacman")
    } else {
        (-1, "unknown")
    };

    if count >= 0 {
        checks.push(RawCheck {
            id: "updates.os",
            label: "System Updates",
            status: if count == 0 { "pass" } else if count < 10 { "warn" } else { "fail" },
            detail: if count == 0 {
                format!("Up to date ({})", pkg_mgr)
            } else {
                format!("{} updates available ({})", count, pkg_mgr)
            },
        });
    } else {
        checks.push(RawCheck {
            id: "updates.os",
            label: "System Updates",
            status: "warn",
            detail: "Could not detect package manager (apt/dnf/pacman)".to_string(),
        });
    }

    checks
}

// ── UpdateScanner ───────────────────────────────────────────────────

pub struct UpdateScanner;

impl Scanner for UpdateScanner {
    fn scan_type(&self) -> &str {
        "updates"
    }

    fn display_name(&self) -> &str {
        "Update Check"
    }

    fn is_system_idle(&self) -> bool {
        true
    }

    fn tick(&self, budget: Duration, conn: &Connection) -> Result<ScanProgress> {
        #[cfg(target_os = "macos")]
        let raw_checks = run_macos_checks(budget);
        #[cfg(target_os = "windows")]
        let raw_checks = run_windows_checks(budget);
        #[cfg(target_os = "linux")]
        let raw_checks = run_linux_checks(budget);

        let count = raw_checks.len();
        let pass_count = raw_checks.iter().filter(|c| c.status == "pass").count();
        let results = checks_to_results(&raw_checks, 1);
        journal::upsert_scan_results(conn, "updates", &results)?;

        Ok(ScanProgress {
            progress_pct: 100,
            detail: format!("{}/{} checks passed", pass_count, count),
            done: true,
        })
    }
}
