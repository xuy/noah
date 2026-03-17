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
            // Some commands write to stderr even on "success" (e.g. sysadminctl).
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
                Some("security".to_string()),
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

    // Firewall
    let fw = run_cmd(
        "/usr/libexec/ApplicationFirewall/socketfilterfw",
        &["--getglobalstate"],
        "",
    );
    checks.push(RawCheck {
        id: "security.firewall",
        label: "Firewall",
        status: if fw.to_lowercase().contains("enabled") { "pass" } else { "fail" },
        detail: fw,
    });

    // FileVault
    let fv = run_cmd("fdesetup", &["status"], "");
    checks.push(RawCheck {
        id: "security.filevault",
        label: "FileVault Encryption",
        status: if fv.contains("On") { "pass" } else { "fail" },
        detail: fv,
    });

    // System Integrity Protection
    let sip = run_cmd("csrutil", &["status"], "");
    checks.push(RawCheck {
        id: "security.sip",
        label: "System Integrity Protection",
        status: if sip.to_lowercase().contains("enabled") { "pass" } else { "fail" },
        detail: sip,
    });

    // Gatekeeper
    let gk = run_cmd("spctl", &["--status"], "");
    checks.push(RawCheck {
        id: "security.gatekeeper",
        label: "Gatekeeper",
        status: if gk.to_lowercase().contains("enabled") { "pass" } else { "fail" },
        detail: gk,
    });

    // Screen Lock — parse delay from sysadminctl output.
    // Output format: "screenLock delay is 900 seconds" or "screenLock is off"
    let sl = run_cmd("sysadminctl", &["-screenLock", "status"], "");
    let sl_lower = sl.to_lowercase();
    let delay_secs = sl_lower
        .split("delay is ")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse::<u32>().ok());

    let (sl_status, sl_detail) = if sl_lower.contains("off") || sl_lower.contains("not") {
        ("fail", "Screen lock is not enabled".to_string())
    } else if let Some(secs) = delay_secs {
        if secs <= 300 {
            ("pass", format!("Requires password after {} seconds", secs))
        } else {
            let mins = secs / 60;
            ("warn", format!("Requires password after {} minutes — consider reducing to 5 minutes or less", mins))
        }
    } else if sl_lower.contains("immediate") {
        ("pass", "Requires password immediately".to_string())
    } else {
        ("pass", sl.clone())
    };

    checks.push(RawCheck {
        id: "security.screen_lock",
        label: "Screen Lock",
        status: sl_status,
        detail: sl_detail,
    });

    // XProtect
    let xp_exists = std::path::Path::new(
        "/Library/Apple/System/Library/CoreServices/XProtect.bundle",
    )
    .exists();
    checks.push(RawCheck {
        id: "security.xprotect",
        label: "XProtect",
        status: if xp_exists { "pass" } else { "fail" },
        detail: if xp_exists {
            "XProtect.bundle present".to_string()
        } else {
            "XProtect.bundle missing".to_string()
        },
    });

    checks
}

// ── Windows checks ──────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn run_windows_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // Defender
    let defender = ps("(Get-MpComputerStatus).RealTimeProtectionEnabled");
    checks.push(RawCheck {
        id: "security.defender",
        label: "Windows Defender",
        status: if defender.trim().eq_ignore_ascii_case("true") { "pass" } else { "fail" },
        detail: format!("RealTimeProtection: {}", defender.trim()),
    });

    // BitLocker
    let bl = ps("(Get-BitLockerVolume -MountPoint 'C:').ProtectionStatus");
    checks.push(RawCheck {
        id: "security.bitlocker",
        label: "BitLocker Encryption",
        status: if bl.trim() == "On" || bl.trim() == "1" { "pass" } else { "fail" },
        detail: format!("BitLocker C: {}", bl.trim()),
    });

    // Firewall
    let fw = ps("(Get-NetFirewallProfile | Where-Object {$_.Enabled -eq $false}).Count");
    let disabled_count: i32 = fw.trim().parse().unwrap_or(-1);
    checks.push(RawCheck {
        id: "security.firewall",
        label: "Firewall",
        status: if disabled_count == 0 { "pass" } else { "fail" },
        detail: if disabled_count == 0 {
            "All firewall profiles enabled".to_string()
        } else {
            format!("{} firewall profile(s) disabled", disabled_count)
        },
    });

    // UAC
    let uac = ps(
        "(Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System' -Name EnableLUA).EnableLUA",
    );
    checks.push(RawCheck {
        id: "security.uac",
        label: "User Account Control",
        status: if uac.trim() == "1" { "pass" } else { "fail" },
        detail: format!("EnableLUA: {}", uac.trim()),
    });

    // Screen Lock
    let sl = ps(
        "(Get-ItemProperty -Path 'HKCU:\\Control Panel\\Desktop' -Name ScreenSaverIsSecure -ErrorAction SilentlyContinue).ScreenSaverIsSecure",
    );
    checks.push(RawCheck {
        id: "security.screen_lock",
        label: "Screen Lock",
        status: if sl.trim() == "1" { "pass" } else { "warn" },
        detail: format!("ScreenSaverIsSecure: {}", sl.trim()),
    });

    checks
}

// ── Linux checks ────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn run_linux_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // Firewall — try UFW first, fall back to iptables/nftables
    let fw = run_cmd("ufw", &["status"], "");
    if !fw.is_empty() {
        checks.push(RawCheck {
            id: "security.firewall",
            label: "Firewall (UFW)",
            status: if fw.to_lowercase().contains("active") && !fw.to_lowercase().contains("inactive") {
                "pass"
            } else {
                "fail"
            },
            detail: fw,
        });
    } else {
        // Check iptables — if there are rules beyond the default ACCEPT policies, firewall is active
        let ipt = run_cmd("sh", &["-c", "iptables -L -n 2>/dev/null | grep -cE '^(ACCEPT|DROP|REJECT|LOG)' || echo 0"], "0");
        let rule_count: i32 = ipt.trim().parse().unwrap_or(0);
        checks.push(RawCheck {
            id: "security.firewall",
            label: "Firewall",
            status: if rule_count > 3 { "pass" } else { "fail" },
            detail: if rule_count > 3 {
                format!("iptables: {} rules active", rule_count)
            } else {
                "No firewall detected (UFW/iptables)".to_string()
            },
        });
    }

    // AppArmor or SELinux
    let aa = run_cmd("sh", &["-c", "cat /sys/module/apparmor/parameters/enabled 2>/dev/null"], "");
    let se = run_cmd("sh", &["-c", "getenforce 2>/dev/null"], "");
    if aa.trim() == "Y" {
        checks.push(RawCheck {
            id: "security.mac",
            label: "AppArmor",
            status: "pass",
            detail: "AppArmor is enabled".to_string(),
        });
    } else if se.trim().eq_ignore_ascii_case("enforcing") {
        checks.push(RawCheck {
            id: "security.mac",
            label: "SELinux",
            status: "pass",
            detail: "SELinux is enforcing".to_string(),
        });
    } else if se.trim().eq_ignore_ascii_case("permissive") {
        checks.push(RawCheck {
            id: "security.mac",
            label: "SELinux",
            status: "warn",
            detail: "SELinux is permissive (not enforcing)".to_string(),
        });
    } else {
        checks.push(RawCheck {
            id: "security.mac",
            label: "Mandatory Access Control",
            status: "fail",
            detail: "Neither AppArmor nor SELinux detected".to_string(),
        });
    }

    // SSH root login
    let sshd_config = run_cmd("sh", &["-c", "grep -i '^PermitRootLogin' /etc/ssh/sshd_config 2>/dev/null"], "");
    let sshd_running = run_cmd("sh", &["-c", "systemctl is-active sshd 2>/dev/null || systemctl is-active ssh 2>/dev/null"], "");
    if sshd_running.trim() == "active" {
        let root_ok = sshd_config.to_lowercase();
        let status = if root_ok.contains("no") || root_ok.contains("prohibit-password") {
            "pass"
        } else if root_ok.is_empty() {
            // Default varies by distro but is often prohibit-password
            "warn"
        } else {
            "fail"
        };
        checks.push(RawCheck {
            id: "security.ssh_root",
            label: "SSH Root Login",
            status,
            detail: if sshd_config.is_empty() {
                "SSH active, PermitRootLogin using default".to_string()
            } else {
                sshd_config
            },
        });
    }

    // Unattended upgrades (Debian/Ubuntu)
    let ua = std::path::Path::new("/etc/apt/apt.conf.d/20auto-upgrades").exists();
    let ua_enabled = if ua {
        let content = run_cmd("sh", &["-c", "grep -i 'Unattended-Upgrade.*1' /etc/apt/apt.conf.d/20auto-upgrades 2>/dev/null"], "");
        !content.is_empty()
    } else {
        false
    };
    // Only show this check on apt-based systems
    if std::path::Path::new("/usr/bin/apt").exists() {
        checks.push(RawCheck {
            id: "security.auto_updates",
            label: "Automatic Security Updates",
            status: if ua_enabled { "pass" } else { "warn" },
            detail: if ua_enabled {
                "Unattended-upgrades enabled".to_string()
            } else {
                "Automatic security updates not configured".to_string()
            },
        });
    }

    checks
}

// ── SecurityScanner ─────────────────────────────────────────────────

pub struct SecurityScanner;

impl Scanner for SecurityScanner {
    fn scan_type(&self) -> &str {
        "security"
    }

    fn display_name(&self) -> &str {
        "Security Check"
    }

    fn is_system_idle(&self) -> bool {
        // Security checks are lightweight — always run.
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
        journal::upsert_scan_results(conn, "security", &results)?;

        Ok(ScanProgress {
            progress_pct: 100,
            detail: format!("{}/{} checks passed", pass_count, count),
            done: true,
        })
    }
}
