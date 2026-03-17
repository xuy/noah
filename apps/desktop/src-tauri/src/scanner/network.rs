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
                Some("network".to_string()),
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

// ── macOS / Linux checks ────────────────────────────────────────────

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn run_unix_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // DNS resolution
    let dns = run_cmd("nslookup", &["dns.google"], "");
    let dns_ok = dns.contains("Address") && !dns.to_lowercase().contains("can't find");
    checks.push(RawCheck {
        id: "network.dns",
        label: "DNS Resolution",
        status: if dns_ok { "pass" } else { "fail" },
        detail: if dns_ok {
            "DNS resolution working".to_string()
        } else {
            "DNS resolution failed".to_string()
        },
    });

    // Internet connectivity
    let ping = run_cmd("ping", &["-c", "1", "-W", "3", "1.1.1.1"], "");
    let ping_ok = ping.contains("1 packets received") || ping.contains("1 received");
    let latency = ping
        .lines()
        .find(|l| l.contains("time="))
        .and_then(|l| {
            l.split("time=")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .map(|s| s.to_string())
        });
    checks.push(RawCheck {
        id: "network.internet",
        label: "Internet Connectivity",
        status: if ping_ok { "pass" } else { "fail" },
        detail: if ping_ok {
            if let Some(ref ms) = latency {
                format!("Internet reachable ({}ms latency)", ms)
            } else {
                "Internet reachable".to_string()
            }
        } else {
            "No internet connectivity".to_string()
        },
    });

    // Default gateway
    #[cfg(target_os = "macos")]
    let gw_output = run_cmd("route", &["-n", "get", "default"], "");
    #[cfg(target_os = "linux")]
    let gw_output = run_cmd("ip", &["route"], "");

    #[cfg(target_os = "macos")]
    let gateway = gw_output
        .lines()
        .find(|l| l.contains("gateway"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string());

    #[cfg(target_os = "linux")]
    let gateway = gw_output
        .lines()
        .find(|l| l.starts_with("default"))
        .and_then(|l| l.split_whitespace().nth(2))
        .map(|s| s.to_string());

    checks.push(RawCheck {
        id: "network.gateway",
        label: "Default Gateway",
        status: if gateway.is_some() { "pass" } else { "fail" },
        detail: gateway.unwrap_or_else(|| "No default gateway found".to_string()),
    });

    checks
}

// ── Windows checks ──────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn run_windows_checks() -> Vec<RawCheck> {
    let mut checks = Vec::new();

    // DNS resolution
    let dns = ps("try { Resolve-DnsName dns.google -ErrorAction Stop | Out-Null; 'ok' } catch { 'fail' }");
    let dns_ok = dns.trim() == "ok";
    checks.push(RawCheck {
        id: "network.dns",
        label: "DNS Resolution",
        status: if dns_ok { "pass" } else { "fail" },
        detail: if dns_ok {
            "DNS resolution working".to_string()
        } else {
            "DNS resolution failed".to_string()
        },
    });

    // Internet connectivity
    let ping = run_cmd("ping", &["-n", "1", "-w", "3000", "1.1.1.1"], "");
    let ping_ok = ping.contains("Reply from") || ping.contains("(0% loss)");
    let latency = ping
        .lines()
        .find(|l| l.contains("time=") || l.contains("time<"))
        .and_then(|l| {
            l.split("time")
                .nth(1)
                .and_then(|s| {
                    let s = s.trim_start_matches('=').trim_start_matches('<');
                    s.split_whitespace().next().map(|v| v.trim_end_matches("ms").to_string())
                })
        });
    checks.push(RawCheck {
        id: "network.internet",
        label: "Internet Connectivity",
        status: if ping_ok { "pass" } else { "fail" },
        detail: if ping_ok {
            if let Some(ref ms) = latency {
                format!("Internet reachable ({}ms latency)", ms)
            } else {
                "Internet reachable".to_string()
            }
        } else {
            "No internet connectivity".to_string()
        },
    });

    // Default gateway
    let gw = ps("(Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Select-Object -First 1).NextHop");
    let gateway = if gw.trim().is_empty() { None } else { Some(gw.trim().to_string()) };
    checks.push(RawCheck {
        id: "network.gateway",
        label: "Default Gateway",
        status: if gateway.is_some() { "pass" } else { "fail" },
        detail: gateway.unwrap_or_else(|| "No default gateway found".to_string()),
    });

    checks
}

// ── NetworkScanner ──────────────────────────────────────────────────

pub struct NetworkScanner;

impl Scanner for NetworkScanner {
    fn scan_type(&self) -> &str {
        "network"
    }

    fn display_name(&self) -> &str {
        "Network Check"
    }

    fn is_system_idle(&self) -> bool {
        true
    }

    fn tick(&self, _budget: Duration, conn: &Connection) -> Result<ScanProgress> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let raw_checks = run_unix_checks();
        #[cfg(target_os = "windows")]
        let raw_checks = run_windows_checks();

        let count = raw_checks.len();
        let pass_count = raw_checks.iter().filter(|c| c.status == "pass").count();
        let results = checks_to_results(&raw_checks, 1);
        journal::upsert_scan_results(conn, "network", &results)?;

        Ok(ScanProgress {
            progress_pct: 100,
            detail: format!("{}/{} checks passed", pass_count, count),
            done: true,
        })
    }
}
