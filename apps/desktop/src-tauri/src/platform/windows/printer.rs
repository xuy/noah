use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use itman_tools::{ChangeRecord, SafetyTier, Tool, ToolResult};

// ── WinPrinterList ────────────────────────────────────────────────────

pub struct WinPrinterList;

#[async_trait]
impl Tool for WinPrinterList {
    fn name(&self) -> &str {
        "win_printer_list"
    }

    fn description(&self) -> &str {
        "List all configured printers and their status."
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
        let output = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                "Get-Printer | Select-Object Name, DriverName, PortName, PrinterStatus, Shared | Format-Table -AutoSize",
            ])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if stdout.trim().is_empty() {
                    "No printers found.".to_string()
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|e| format!("Get-Printer failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output.trim() }),
        ))
    }
}

// ── WinPrintQueue ─────────────────────────────────────────────────────

pub struct WinPrintQueue;

#[async_trait]
impl Tool for WinPrintQueue {
    fn name(&self) -> &str {
        "win_print_queue"
    }

    fn description(&self) -> &str {
        "Show all pending print jobs across all printers."
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
        let output = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                "Get-Printer | ForEach-Object { \
                    $printer = $_.Name; \
                    Get-PrintJob -PrinterName $printer -ErrorAction SilentlyContinue | \
                    Select-Object @{Name='Printer';Expression={$printer}}, Id, DocumentName, UserName, SubmittedTime, JobStatus \
                } | Format-Table -AutoSize",
            ])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if stdout.trim().is_empty() {
                    "No pending print jobs.".to_string()
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|e| format!("Get-PrintJob failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output.trim() }),
        ))
    }
}

// ── WinCancelPrintJobs ────────────────────────────────────────────────

pub struct WinCancelPrintJobs;

#[async_trait]
impl Tool for WinCancelPrintJobs {
    fn name(&self) -> &str {
        "win_cancel_print_jobs"
    }

    fn description(&self) -> &str {
        "Cancel all pending print jobs across all printers. Requires user approval."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        let output = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                "Get-Printer | ForEach-Object { \
                    Get-PrintJob -PrinterName $_.Name -ErrorAction SilentlyContinue | \
                    Remove-PrintJob -ErrorAction SilentlyContinue \
                }; \
                'All print jobs cancelled.'",
            ])
            .output()
            .map(|o| {
                if o.status.success() {
                    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    if stdout.trim().is_empty() {
                        "All print jobs cancelled successfully.".to_string()
                    } else {
                        stdout.trim().to_string()
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("Cancel command completed with errors: {}", stderr.trim())
                }
            })
            .unwrap_or_else(|e| format!("Failed to cancel print jobs: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({ "status": output }),
            vec![ChangeRecord {
                description: "Cancelled all pending print jobs".to_string(),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}

// ── WinRestartSpooler ─────────────────────────────────────────────────

pub struct WinRestartSpooler;

#[async_trait]
impl Tool for WinRestartSpooler {
    fn name(&self) -> &str {
        "win_restart_spooler"
    }

    fn description(&self) -> &str {
        "Restart the Windows Print Spooler service. This can fix stuck print queues."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        let output = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                "Restart-Service -Name Spooler -Force -ErrorAction Stop; \
                'Print Spooler service restarted successfully.'",
            ])
            .output()
            .map(|o| {
                if o.status.success() {
                    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    if stdout.trim().is_empty() {
                        "Print Spooler service restarted successfully.".to_string()
                    } else {
                        stdout.trim().to_string()
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!(
                        "Failed to restart Print Spooler. You may need to run as administrator.\n{}",
                        stderr.trim()
                    )
                }
            })
            .unwrap_or_else(|e| format!("Restart-Service failed: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({ "status": output }),
            vec![ChangeRecord {
                description: "Restarted Print Spooler service".to_string(),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}
