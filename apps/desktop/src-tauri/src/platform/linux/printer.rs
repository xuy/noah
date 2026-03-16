use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use noah_tools::{ChangeRecord, SafetyTier, Tool, ToolResult};

// ── LinuxPrinterList ──────────────────────────────────────────────────

pub struct LinuxPrinterList;

#[async_trait]
impl Tool for LinuxPrinterList {
    fn name(&self) -> &str {
        "linux_printer_list"
    }

    fn description(&self) -> &str {
        "List configured printers and the default printer using CUPS (lpstat)."
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
        let output = Command::new("lpstat")
            .args(["-p", "-d"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if stdout.is_empty() && !stderr.is_empty() {
                    if stderr.contains("No destinations added") || stderr.contains("lpstat: error") {
                        "No printers configured. CUPS may not be installed or running.".to_string()
                    } else {
                        format!("lpstat error: {}", stderr.trim())
                    }
                } else if stdout.trim().is_empty() {
                    "No printers found.".to_string()
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "lpstat not found. CUPS does not appear to be installed.".to_string()
                } else {
                    format!("lpstat failed: {}", e)
                }
            });

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output.trim() }),
        ))
    }
}

// ── LinuxPrintQueue ───────────────────────────────────────────────────

pub struct LinuxPrintQueue;

#[async_trait]
impl Tool for LinuxPrintQueue {
    fn name(&self) -> &str {
        "linux_print_queue"
    }

    fn description(&self) -> &str {
        "Show the current print queue (pending print jobs)."
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
        let output = Command::new("lpstat")
            .arg("-o")
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if stdout.trim().is_empty() {
                    "No pending print jobs.".to_string()
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|e| format!("lpstat failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output.trim() }),
        ))
    }
}

// ── LinuxCancelPrintJobs ──────────────────────────────────────────────

pub struct LinuxCancelPrintJobs;

#[async_trait]
impl Tool for LinuxCancelPrintJobs {
    fn name(&self) -> &str {
        "linux_cancel_print_jobs"
    }

    fn description(&self) -> &str {
        "Cancel all pending print jobs."
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
        let output = Command::new("cancel")
            .arg("-a")
            .output()
            .map(|o| {
                if o.status.success() {
                    "All print jobs cancelled successfully.".to_string()
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("Failed to cancel print jobs: {}", stderr.trim())
                }
            })
            .unwrap_or_else(|e| format!("cancel failed: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({}),
            vec![ChangeRecord {
                description: "Cancelled all pending print jobs".to_string(),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}

// ── LinuxRestartCups ──────────────────────────────────────────────────

pub struct LinuxRestartCups;

#[async_trait]
impl Tool for LinuxRestartCups {
    fn name(&self) -> &str {
        "linux_restart_cups"
    }

    fn description(&self) -> &str {
        "Restart the CUPS print service using systemctl."
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
        let output = Command::new("systemctl")
            .args(["restart", "cups"])
            .output()
            .map(|o| {
                if o.status.success() {
                    "CUPS print service restarted successfully.".to_string()
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    if stderr.contains("Access denied") || stderr.contains("Permission denied") {
                        format!(
                            "Failed to restart CUPS — insufficient privileges.\n{}",
                            stderr.trim()
                        )
                    } else {
                        format!("Failed to restart CUPS: {}", stderr.trim())
                    }
                }
            })
            .unwrap_or_else(|e| format!("systemctl failed: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({}),
            vec![ChangeRecord {
                description: "Restarted CUPS print service".to_string(),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}
