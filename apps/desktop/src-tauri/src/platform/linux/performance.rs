use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use itman_tools::{ChangeRecord, SafetyTier, Tool, ToolResult};

// ── Helpers for /proc parsing ─────────────────────────────────────────

/// Extract CPU model and core count from /proc/cpuinfo.
fn read_cpu_info() -> (String, usize) {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default();

    let model = cpuinfo
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let cores = cpuinfo
        .lines()
        .filter(|l| l.starts_with("processor"))
        .count();

    (model, cores.max(1))
}

/// Parse /proc/meminfo into a human-readable summary.
pub(super) fn read_mem_info() -> String {
    let meminfo = match std::fs::read_to_string("/proc/meminfo") {
        Ok(s) => s,
        Err(e) => return format!("failed to read /proc/meminfo: {}", e),
    };

    let get_kb = |key: &str| -> Option<u64> {
        meminfo
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| {
                l.split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse::<u64>().ok())
            })
    };

    let total = get_kb("MemTotal:");
    let available = get_kb("MemAvailable:");
    let swap_total = get_kb("SwapTotal:");
    let swap_free = get_kb("SwapFree:");

    let fmt_gb = |kb: u64| -> String {
        if kb >= 1_048_576 {
            format!("{:.1} GB", kb as f64 / 1_048_576.0)
        } else {
            format!("{} MB", kb / 1024)
        }
    };

    let mut parts = Vec::new();
    if let (Some(t), Some(a)) = (total, available) {
        let used = t.saturating_sub(a);
        parts.push(format!(
            "Total: {}  Used: {}  Available: {}",
            fmt_gb(t), fmt_gb(used), fmt_gb(a)
        ));
    }
    if let (Some(st), Some(sf)) = (swap_total, swap_free) {
        if st > 0 {
            let su = st.saturating_sub(sf);
            parts.push(format!(
                "Swap:  Total: {}  Used: {}  Free: {}",
                fmt_gb(st), fmt_gb(su), fmt_gb(sf)
            ));
        }
    }

    if parts.is_empty() {
        "could not parse memory info".to_string()
    } else {
        parts.join("\n")
    }
}

// ── LinuxSystemInfo ───────────────────────────────────────────────────

pub struct LinuxSystemInfo;

#[async_trait]
impl Tool for LinuxSystemInfo {
    fn name(&self) -> &str {
        "linux_system_info"
    }

    fn description(&self) -> &str {
        "Get Linux distribution, kernel version, CPU model, core count, memory, and uptime."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        // Distribution — read directly, no external command.
        let os_release = std::fs::read_to_string("/etc/os-release")
            .unwrap_or_else(|_| "unknown distribution".to_string());
        let distro = os_release
            .lines()
            .find(|l| l.starts_with("PRETTY_NAME="))
            .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"'))
            .unwrap_or("Unknown Linux");

        // Kernel — uname is POSIX, always available.
        let kernel = Command::new("uname")
            .args(["-r"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("uname failed: {}", e));

        // CPU — read /proc/cpuinfo directly.
        let (cpu_model, core_count) = read_cpu_info();
        let cpu = format!("{} ({} cores)", cpu_model, core_count);

        // Memory — read /proc/meminfo directly.
        let memory = read_mem_info();

        // Uptime — POSIX.
        let uptime = Command::new("uptime")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("uptime failed: {}", e));

        let output = format!(
            "=== Distribution ===\n{}\nKernel: {}\n\n=== CPU ===\n{}\n\n=== Memory ===\n{}\n\n=== Uptime ===\n{}",
            distro, kernel, cpu, memory, uptime
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "distro": distro,
                "kernel": kernel,
                "cpu": cpu,
                "memory": memory,
                "uptime": uptime,
            }),
        ))
    }
}

// ── LinuxProcessList ──────────────────────────────────────────────────

pub struct LinuxProcessList;

#[async_trait]
impl Tool for LinuxProcessList {
    fn name(&self) -> &str {
        "linux_process_list"
    }

    fn description(&self) -> &str {
        "List running processes sorted by CPU or memory usage."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "sort_by": {
                    "type": "string",
                    "description": "Sort by 'cpu' or 'mem' (default: cpu)",
                    "enum": ["cpu", "mem"],
                    "default": "cpu"
                }
            },
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let sort_by = input["sort_by"].as_str().unwrap_or("cpu");
        let sort_key = if sort_by == "mem" { "-%mem" } else { "-%cpu" };

        // ps is POSIX. --sort is a GNU extension but available on all mainstream Linux.
        let output = Command::new("ps")
            .args(["aux", "--sort", sort_key])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let lines: Vec<&str> = stdout.lines().collect();
                if lines.len() > 26 {
                    lines[..26].join("\n")
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|e| format!("ps failed: {}", e));

        Ok(ToolResult::read_only(
            format!("=== Top Processes (sorted by {}) ===\n{}", sort_by, output),
            json!({
                "sort_by": sort_by,
                "ps_output": output.trim(),
            }),
        ))
    }
}

// ── LinuxDiskUsage ────────────────────────────────────────────────────

pub struct LinuxDiskUsage;

#[async_trait]
impl Tool for LinuxDiskUsage {
    fn name(&self) -> &str {
        "linux_disk_usage"
    }

    fn description(&self) -> &str {
        "Show disk usage for all mounted filesystems."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        // df is POSIX.
        let output = Command::new("df")
            .arg("-h")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("df failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output.trim() }),
        ))
    }
}

// ── LinuxKillProcess ──────────────────────────────────────────────────

pub struct LinuxKillProcess;

#[async_trait]
impl Tool for LinuxKillProcess {
    fn name(&self) -> &str {
        "linux_kill_process"
    }

    fn description(&self) -> &str {
        "Kill a process by PID. Use signal 15 (SIGTERM) for graceful or 9 (SIGKILL) for force kill."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pid": {
                    "type": "integer",
                    "description": "Process ID to kill"
                },
                "signal": {
                    "type": "integer",
                    "description": "Signal number: 15 for SIGTERM (graceful), 9 for SIGKILL (force). Default: 15",
                    "default": 15
                }
            },
            "required": ["pid"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let pid = input["pid"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: pid"))?;
        let signal = input["signal"].as_u64().unwrap_or(15);

        // ps is POSIX.
        let ps_info = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "pid,comm,%cpu,%mem"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        // kill is POSIX.
        let output = Command::new("kill")
            .args([&format!("-{}", signal), &pid.to_string()])
            .output()
            .map(|o| {
                if o.status.success() {
                    format!(
                        "Process {} killed with signal {}.\n\nProcess info:\n{}",
                        pid, signal, ps_info.trim()
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("Failed to kill process {}: {}", pid, stderr.trim())
                }
            })
            .unwrap_or_else(|e| format!("kill failed: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({ "pid": pid, "signal": signal }),
            vec![ChangeRecord {
                description: format!("Killed process {} with signal {}", pid, signal),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}
