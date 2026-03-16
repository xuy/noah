use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use noah_tools::{ChangeRecord, SafetyTier, Tool, ToolResult};

// ── MacSystemInfo ──────────────────────────────────────────────────────

pub struct MacSystemInfo;

#[async_trait]
impl Tool for MacSystemInfo {
    fn name(&self) -> &str {
        "mac_system_info"
    }

    fn description(&self) -> &str {
        "Get macOS version, CPU model, core count, and total memory."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        let sw_vers = Command::new("sw_vers")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("sw_vers failed: {}", e));

        let cpu_brand = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("sysctl failed: {}", e));

        let cpu_count = Command::new("sysctl")
            .args(["-n", "hw.ncpu"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("sysctl failed: {}", e));

        let mem_bytes = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("sysctl failed: {}", e));

        let mem_gb = mem_bytes
            .parse::<u64>()
            .map(|b| format!("{} GB", b / (1024 * 1024 * 1024)))
            .unwrap_or_else(|_| mem_bytes.clone());

        let output = format!(
            "=== macOS Version ===\n{}\n=== CPU ===\n{} ({} cores)\n\n=== Memory ===\n{}",
            sw_vers.trim(),
            cpu_brand,
            cpu_count,
            mem_gb
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "sw_vers": sw_vers.trim(),
                "cpu": cpu_brand,
                "cores": cpu_count,
                "memory": mem_gb,
            }),
        ))
    }
}

// ── MacProcessList ─────────────────────────────────────────────────────

pub struct MacProcessList;

#[async_trait]
impl Tool for MacProcessList {
    fn name(&self) -> &str {
        "mac_process_list"
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
            "required": [],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let sort_by = input["sort_by"].as_str().unwrap_or("cpu");
        let output = Command::new("ps")
            .args(["aux"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                // Take header + top 25 processes
                let mut lines: Vec<&str> = stdout.lines().collect();
                if lines.len() > 26 {
                    lines.truncate(26);
                }
                lines.join("\n")
            })
            .unwrap_or_else(|e| format!("ps failed: {}", e));

        // Also get top output for a sorted view
        let top_output = Command::new("top")
            .args(["-l", "1", "-n", "20", "-o", if sort_by == "mem" { "mem" } else { "cpu" }, "-s", "0"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                // Get the last section with the process list
                let lines: Vec<&str> = stdout.lines().collect();
                let start = lines.iter().position(|l| l.starts_with("PID")).unwrap_or(0);
                lines[start..].join("\n")
            })
            .unwrap_or_else(|e| format!("top failed: {}", e));

        let combined = format!(
            "=== Top Processes (sorted by {}) ===\n{}",
            sort_by, top_output
        );

        Ok(ToolResult::read_only(
            combined.clone(),
            json!({
                "sort_by": sort_by,
                "top_output": top_output.trim(),
                "ps_output": output.trim(),
            }),
        ))
    }
}

// ── MacDiskUsage ───────────────────────────────────────────────────────

pub struct MacDiskUsage;

#[async_trait]
impl Tool for MacDiskUsage {
    fn name(&self) -> &str {
        "mac_disk_usage"
    }

    fn description(&self) -> &str {
        "Show disk usage for all mounted volumes."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
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

// ── MacKillProcess ─────────────────────────────────────────────────────

pub struct MacKillProcess;

#[async_trait]
impl Tool for MacKillProcess {
    fn name(&self) -> &str {
        "mac_kill_process"
    }

    fn description(&self) -> &str {
        "Kill a process by PID. Use signal 15 (SIGTERM) for graceful or 9 (SIGKILL) for force kill. Requires user approval."
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
            "required": ["pid"],
            "additionalProperties": false
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

        // Get process info before killing
        let ps_info = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "pid,comm,%cpu,%mem"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

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

// ── MacClearCaches ─────────────────────────────────────────────────────

pub struct MacClearCaches;

#[async_trait]
impl Tool for MacClearCaches {
    fn name(&self) -> &str {
        "mac_clear_caches"
    }

    fn description(&self) -> &str {
        "Clear the user's ~/Library/Caches/ directory to free disk space."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let caches_dir = format!("{}/Library/Caches", home);

        // Get size before clearing
        let before_size = Command::new("du")
            .args(["-sh", &caches_dir])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // Remove contents of Caches directory (not the directory itself)
        let output = Command::new("find")
            .args([&caches_dir, "-mindepth", "1", "-maxdepth", "1", "-exec", "rm", "-rf", "{}", ";"])
            .output()
            .map(|o| {
                if o.status.success() {
                    format!(
                        "Caches cleared successfully.\nBefore: {}",
                        before_size
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("Some caches cleared (some may be in use): {}", stderr.trim())
                }
            })
            .unwrap_or_else(|e| format!("Failed to clear caches: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({
                "caches_dir": caches_dir,
                "before_size": before_size,
            }),
            vec![ChangeRecord {
                description: format!("Cleared contents of {}", caches_dir),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}
