use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use noah_tools::{ChangeRecord, SafetyTier, Tool, ToolResult};

// ── MacAppList ─────────────────────────────────────────────────────────

pub struct MacAppList;

#[async_trait]
impl Tool for MacAppList {
    fn name(&self) -> &str {
        "mac_app_list"
    }

    fn description(&self) -> &str {
        "List installed applications in /Applications with their version numbers."
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
        // Use system_profiler for a clean list with versions
        let output = Command::new("system_profiler")
            .args(["SPApplicationsDataType", "-json"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                // Try to parse and provide a summarised view
                if let Ok(parsed) = serde_json::from_str::<Value>(&stdout) {
                    if let Some(apps) = parsed["SPApplicationsDataType"].as_array() {
                        let mut lines: Vec<String> = apps
                            .iter()
                            .filter_map(|app| {
                                let name = app["_name"].as_str()?;
                                let version = app["version"].as_str().unwrap_or("unknown");
                                let path = app["path"].as_str().unwrap_or("");
                                if path.starts_with("/Applications") {
                                    Some(format!("  {} (v{})", name, version))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        lines.sort();
                        return format!("Installed Applications:\n{}", lines.join("\n"));
                    }
                }
                // Fallback to simple ls
                "Failed to parse application list. Use ls /Applications for a basic listing."
                    .to_string()
            })
            .unwrap_or_else(|e| format!("system_profiler failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output }),
        ))
    }
}

// ── MacAppLogs ─────────────────────────────────────────────────────────

pub struct MacAppLogs;

#[async_trait]
impl Tool for MacAppLogs {
    fn name(&self) -> &str {
        "mac_app_logs"
    }

    fn description(&self) -> &str {
        "Show recent system logs for a specific application using the unified log system."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "Application name (or process name) to filter logs for"
                },
                "duration": {
                    "type": "string",
                    "description": "How far back to look, e.g. '1h', '30m', '1d'. Default: '1h'",
                    "default": "1h"
                }
            },
            "required": ["app_name"],
            "additionalProperties": false
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

        let predicate = format!("process == \"{}\"", app_name);

        let output = Command::new("log")
            .args([
                "show",
                "--predicate",
                &predicate,
                "--last",
                duration,
                "--style",
                "compact",
            ])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if stdout.trim().is_empty() {
                    format!("No log entries found for '{}' in the last {}.", app_name, duration)
                } else {
                    // Limit output to last 100 lines
                    let lines: Vec<&str> = stdout.lines().collect();
                    let start = if lines.len() > 100 { lines.len() - 100 } else { 0 };
                    let truncated = lines[start..].join("\n");
                    if start > 0 {
                        format!(
                            "... (showing last 100 of {} log entries)\n{}",
                            lines.len(),
                            truncated
                        )
                    } else {
                        truncated
                    }
                }
            })
            .unwrap_or_else(|e| format!("log show failed: {}", e));

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

// ── MacAppSupportLs ────────────────────────────────────────────────────

pub struct MacAppSupportLs;

#[async_trait]
impl Tool for MacAppSupportLs {
    fn name(&self) -> &str {
        "mac_app_support_ls"
    }

    fn description(&self) -> &str {
        "List contents of an application's support directory in ~/Library/Application Support/."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "Application name (subdirectory of Application Support)"
                }
            },
            "required": ["app_name"],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let app_name = input["app_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: app_name"))?;

        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let support_dir = format!("{}/Library/Application Support/{}", home, app_name);

        let output = Command::new("ls")
            .args(["-la", &support_dir])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if stdout.trim().is_empty() && !stderr.trim().is_empty() {
                    format!("Directory not found or empty: {}\n{}", support_dir, stderr.trim())
                } else {
                    format!("Contents of {}:\n{}", support_dir, stdout)
                }
            })
            .unwrap_or_else(|e| format!("ls failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({
                "app_name": app_name,
                "path": support_dir,
                "raw_output": output,
            }),
        ))
    }
}

// ── MacClearAppCache ───────────────────────────────────────────────────

pub struct MacClearAppCache;

#[async_trait]
impl Tool for MacClearAppCache {
    fn name(&self) -> &str {
        "mac_clear_app_cache"
    }

    fn description(&self) -> &str {
        "Clear cache files for a specific application from ~/Library/Caches/. Requires user approval."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "Application name (or bundle identifier) whose caches to clear"
                }
            },
            "required": ["app_name"],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let app_name = input["app_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: app_name"))?;

        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let cache_dir = format!("{}/Library/Caches/{}", home, app_name);

        // Check if directory exists
        let exists = std::path::Path::new(&cache_dir).exists();
        if !exists {
            return Ok(ToolResult::read_only(
                format!("Cache directory not found: {}", cache_dir),
                json!({ "error": "directory_not_found", "path": cache_dir }),
            ));
        }

        // Get size before clearing
        let before_size = Command::new("du")
            .args(["-sh", &cache_dir])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // Create a backup path for undo
        let backup_dir = format!("{}/Library/Caches/.noah_backup_{}", home, app_name);

        // Move to backup instead of deleting (for undo)
        let output = Command::new("mv")
            .args([&cache_dir, &backup_dir])
            .output()
            .map(|o| {
                if o.status.success() {
                    format!(
                        "Cleared cache for '{}' (was {}).\nBackup saved to {}",
                        app_name, before_size, backup_dir
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("Failed to clear cache: {}", stderr.trim())
                }
            })
            .unwrap_or_else(|e| format!("Failed to clear cache: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({
                "app_name": app_name,
                "cache_dir": cache_dir,
                "before_size": before_size,
                "backup_dir": backup_dir,
            }),
            vec![ChangeRecord {
                description: format!("Cleared cache for '{}' from {}", app_name, cache_dir),
                undo_tool: "mac_move_file".to_string(),
                undo_input: json!({
                    "source": backup_dir,
                    "destination": cache_dir,
                    "operation": "move"
                }),
            }],
        ))
    }
}

// ── MacMoveFile ────────────────────────────────────────────────────────

pub struct MacMoveFile;

#[async_trait]
impl Tool for MacMoveFile {
    fn name(&self) -> &str {
        "mac_move_file"
    }

    fn description(&self) -> &str {
        "Move or copy a file or directory."
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
            "required": ["source", "destination"],
            "additionalProperties": false
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

        let (cmd, flag) = match operation {
            "copy" => ("cp", "-R"),
            _ => ("mv", "-f"),
        };

        let output = Command::new(cmd)
            .args([flag, source, destination])
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
            .unwrap_or_else(|e| format!("{} failed: {}", cmd, e));

        // Build undo record for move operations
        let changes = if operation == "move" {
            vec![ChangeRecord {
                description: format!("Moved '{}' to '{}'", source, destination),
                undo_tool: "mac_move_file".to_string(),
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
