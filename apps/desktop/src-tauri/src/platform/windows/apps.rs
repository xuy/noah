use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use itman_tools::{ChangeRecord, SafetyTier, Tool, ToolResult};

// ── WinAppList ────────────────────────────────────────────────────────

pub struct WinAppList;

#[async_trait]
impl Tool for WinAppList {
    fn name(&self) -> &str {
        "win_app_list"
    }

    fn description(&self) -> &str {
        "List installed applications with their version numbers from the Windows registry."
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
                "Get-ItemProperty \
                    'HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*', \
                    'HKLM:\\Software\\Wow6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*' \
                    -ErrorAction SilentlyContinue | \
                Where-Object { $_.DisplayName } | \
                Select-Object DisplayName, DisplayVersion, Publisher, InstallDate | \
                Sort-Object DisplayName | \
                Format-Table -AutoSize",
            ])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if stdout.trim().is_empty() {
                    "No applications found or unable to read registry.".to_string()
                } else {
                    format!("Installed Applications:\n{}", stdout.trim())
                }
            })
            .unwrap_or_else(|e| format!("Failed to list applications: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output }),
        ))
    }
}

// ── WinAppLogs ────────────────────────────────────────────────────────

pub struct WinAppLogs;

#[async_trait]
impl Tool for WinAppLogs {
    fn name(&self) -> &str {
        "win_app_logs"
    }

    fn description(&self) -> &str {
        "Show recent Windows Event Log entries for a specific application from the Application log."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "Application or source name to filter event logs for"
                },
                "duration": {
                    "type": "string",
                    "description": "How far back to look, e.g. '1h', '30m', '1d'. Default: '1h'",
                    "default": "1h"
                }
            },
            "required": ["app_name"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let app_name = input["app_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: app_name"))?;
        let duration = input["duration"].as_str().unwrap_or("1h");

        // Parse duration into hours for PowerShell
        let hours = parse_duration_hours(duration);

        let ps_cmd = format!(
            "$start = (Get-Date).AddHours(-{}); \
            Get-WinEvent -FilterHashtable @{{ \
                LogName='Application'; \
                ProviderName='{}'; \
                StartTime=$start \
            }} -MaxEvents 100 -ErrorAction SilentlyContinue | \
            Format-List TimeCreated, Id, LevelDisplayName, Message",
            hours, app_name
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_cmd])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if stdout.trim().is_empty() {
                    format!("No event log entries found for '{}' in the last {}.", app_name, duration)
                } else {
                    let lines: Vec<&str> = stdout.lines().collect();
                    if lines.len() > 100 {
                        format!(
                            "... (showing last 100 of {} entries)\n{}",
                            lines.len(),
                            lines[lines.len() - 100..].join("\n")
                        )
                    } else {
                        stdout
                    }
                }
            })
            .unwrap_or_else(|e| format!("Get-WinEvent failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({
                "app_name": app_name,
                "duration": duration,
                "raw_output": output,
            }),
        ))
    }
}

// ── WinAppDataLs ──────────────────────────────────────────────────────

pub struct WinAppDataLs;

#[async_trait]
impl Tool for WinAppDataLs {
    fn name(&self) -> &str {
        "win_app_data_ls"
    }

    fn description(&self) -> &str {
        "List contents of an application's data directories in %APPDATA% and %LOCALAPPDATA%."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "Application name (subdirectory of AppData)"
                }
            },
            "required": ["app_name"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let app_name = input["app_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: app_name"))?;

        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| String::new());
        let localappdata = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::new());

        let roaming_dir = format!("{}\\{}", appdata, app_name);
        let local_dir = format!("{}\\{}", localappdata, app_name);

        let roaming_ls = Command::new("cmd")
            .args(["/c", "dir", &roaming_dir])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if stdout.trim().is_empty() && !stderr.trim().is_empty() {
                    format!("Not found: {}", roaming_dir)
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|e| format!("dir failed: {}", e));

        let local_ls = Command::new("cmd")
            .args(["/c", "dir", &local_dir])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if stdout.trim().is_empty() && !stderr.trim().is_empty() {
                    format!("Not found: {}", local_dir)
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|e| format!("dir failed: {}", e));

        let output = format!(
            "=== Roaming AppData ({}) ===\n{}\n\n=== Local AppData ({}) ===\n{}",
            roaming_dir, roaming_ls.trim(),
            local_dir, local_ls.trim()
        );

        Ok(ToolResult::read_only(
            output.clone(),
            json!({
                "app_name": app_name,
                "roaming_path": roaming_dir,
                "local_path": local_dir,
                "raw_output": output,
            }),
        ))
    }
}

// ── WinClearAppCache ──────────────────────────────────────────────────

pub struct WinClearAppCache;

#[async_trait]
impl Tool for WinClearAppCache {
    fn name(&self) -> &str {
        "win_clear_app_cache"
    }

    fn description(&self) -> &str {
        "Clear cache files for a specific application from %LOCALAPPDATA%. Requires user approval."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "Application name whose local cache to clear"
                }
            },
            "required": ["app_name"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let app_name = input["app_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: app_name"))?;

        let localappdata = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
        let cache_dir = format!("{}\\{}", localappdata, app_name);

        // Check if directory exists
        let exists = std::path::Path::new(&cache_dir).exists();
        if !exists {
            return Ok(ToolResult::read_only(
                format!("App data directory not found: {}", cache_dir),
                json!({ "error": "directory_not_found", "path": cache_dir }),
            ));
        }

        // Get size before clearing
        let before_size = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                &format!(
                    "$size = (Get-ChildItem '{}' -Recurse -ErrorAction SilentlyContinue | \
                    Measure-Object -Property Length -Sum -ErrorAction SilentlyContinue).Sum; \
                    '{{0:N2}} MB' -f ($size / 1MB)",
                    cache_dir
                ),
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // Create backup and move
        let backup_dir = format!("{}\\..\\..\\..\\Temp\\.noah_backup_{}", localappdata, app_name);

        let output = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                &format!("Move-Item -Path '{}' -Destination '{}' -Force", cache_dir, backup_dir),
            ])
            .output()
            .map(|o| {
                if o.status.success() {
                    format!(
                        "Cleared app data for '{}' (was {}).\nBackup saved to {}",
                        app_name, before_size, backup_dir
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("Failed to clear app data: {}", stderr.trim())
                }
            })
            .unwrap_or_else(|e| format!("Failed to clear app data: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({
                "app_name": app_name,
                "cache_dir": cache_dir,
                "before_size": before_size,
                "backup_dir": backup_dir,
            }),
            vec![ChangeRecord {
                description: format!("Cleared app data for '{}' from {}", app_name, cache_dir),
                undo_tool: "win_move_file".to_string(),
                undo_input: json!({
                    "source": backup_dir,
                    "destination": cache_dir,
                    "operation": "move"
                }),
            }],
        ))
    }
}

// ── WinMoveFile ───────────────────────────────────────────────────────

pub struct WinMoveFile;

#[async_trait]
impl Tool for WinMoveFile {
    fn name(&self) -> &str {
        "win_move_file"
    }

    fn description(&self) -> &str {
        "Move or copy a file or directory using PowerShell."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "Source file or directory path"
                },
                "destination": {
                    "type": "string",
                    "description": "Destination path"
                },
                "operation": {
                    "type": "string",
                    "description": "Operation: 'move' or 'copy'. Default: 'move'",
                    "enum": ["move", "copy"],
                    "default": "move"
                }
            },
            "required": ["source", "destination"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let source = input["source"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: source"))?;
        let destination = input["destination"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: destination"))?;
        let operation = input["operation"].as_str().unwrap_or("move");

        let cmdlet = if operation == "copy" { "Copy-Item" } else { "Move-Item" };

        let output = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                &format!("{} -Path '{}' -Destination '{}' -Force -Recurse", cmdlet, source, destination),
            ])
            .output()
            .map(|o| {
                if o.status.success() {
                    format!(
                        "Successfully {}d '{}' to '{}'",
                        if operation == "copy" { "copie" } else { "move" },
                        source,
                        destination
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("Failed to {} file: {}", operation, stderr.trim())
                }
            })
            .unwrap_or_else(|e| format!("{} failed: {}", cmdlet, e));

        let changes = if operation == "move" {
            vec![ChangeRecord {
                description: format!("Moved '{}' to '{}'", source, destination),
                undo_tool: "win_move_file".to_string(),
                undo_input: json!({
                    "source": destination,
                    "destination": source,
                    "operation": "move"
                }),
            }]
        } else {
            vec![ChangeRecord {
                description: format!("Copied '{}' to '{}'", source, destination),
                undo_tool: String::new(),
                undo_input: json!(null),
            }]
        };

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({
                "operation": operation,
                "source": source,
                "destination": destination,
            }),
            changes,
        ))
    }
}

/// Parse a duration string like "1h", "30m", "1d" into fractional hours.
fn parse_duration_hours(duration: &str) -> f64 {
    let s = duration.trim().to_lowercase();
    if let Some(d) = s.strip_suffix('d') {
        d.parse::<f64>().unwrap_or(1.0) * 24.0
    } else if let Some(h) = s.strip_suffix('h') {
        h.parse::<f64>().unwrap_or(1.0)
    } else if let Some(m) = s.strip_suffix('m') {
        m.parse::<f64>().unwrap_or(30.0) / 60.0
    } else {
        1.0
    }
}
