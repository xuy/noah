use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use noah_tools::{ChangeRecord, SafetyTier, Tool, ToolResult};

// ── LinuxAppList ──────────────────────────────────────────────────────

pub struct LinuxAppList;

#[async_trait]
impl Tool for LinuxAppList {
    fn name(&self) -> &str {
        "linux_app_list"
    }

    fn description(&self) -> &str {
        "List installed packages. Tries dpkg (Debian/Ubuntu), rpm (Fedora/RHEL), and flatpak."
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
        let mut sections = Vec::new();

        // Try dpkg (Debian/Ubuntu)
        if let Ok(o) = Command::new("dpkg").args(["--list"]).output() {
            if o.status.success() {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let lines: Vec<&str> = stdout.lines().collect();
                let count = lines.len().saturating_sub(5); // skip header lines
                if lines.len() > 30 {
                    sections.push(format!(
                        "=== dpkg ({} packages) ===\n{}\n... ({} more, use shell_run 'dpkg --list' for full output)",
                        count,
                        lines[..30].join("\n"),
                        count.saturating_sub(25)
                    ));
                } else {
                    sections.push(format!("=== dpkg ({} packages) ===\n{}", count, stdout));
                }
            }
        }

        // Try rpm (Fedora/RHEL)
        if sections.is_empty() {
            if let Ok(o) = Command::new("rpm").args(["-qa", "--queryformat", "%{NAME} %{VERSION}-%{RELEASE}\n"]).output() {
                if o.status.success() {
                    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    let lines: Vec<&str> = stdout.lines().collect();
                    if lines.len() > 50 {
                        sections.push(format!(
                            "=== rpm ({} packages) ===\n{}\n... ({} more)",
                            lines.len(),
                            lines[..50].join("\n"),
                            lines.len() - 50
                        ));
                    } else {
                        sections.push(format!("=== rpm ({} packages) ===\n{}", lines.len(), stdout));
                    }
                }
            }
        }

        // Try flatpak
        if let Ok(o) = Command::new("flatpak").args(["list", "--columns=application,version"]).output() {
            if o.status.success() {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if !stdout.trim().is_empty() {
                    sections.push(format!("=== Flatpak ===\n{}", stdout.trim()));
                }
            }
        }

        // Try snap
        if let Ok(o) = Command::new("snap").args(["list"]).output() {
            if o.status.success() {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if !stdout.trim().is_empty() {
                    sections.push(format!("=== Snap ===\n{}", stdout.trim()));
                }
            }
        }

        let output = if sections.is_empty() {
            "No package manager found (tried dpkg, rpm, flatpak, snap).".to_string()
        } else {
            sections.join("\n\n")
        };

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output }),
        ))
    }
}

// ── LinuxAppDataLs ────────────────────────────────────────────────────

pub struct LinuxAppDataLs;

#[async_trait]
impl Tool for LinuxAppDataLs {
    fn name(&self) -> &str {
        "linux_app_data_ls"
    }

    fn description(&self) -> &str {
        "List contents of an application's data/config directory (~/.config/{app} or ~/.local/share/{app})."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "Application name (subdirectory name under ~/.config or ~/.local/share)"
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
        let mut sections = Vec::new();

        for base in &[
            format!("{}/.config/{}", home, app_name),
            format!("{}/.local/share/{}", home, app_name),
        ] {
            if std::path::Path::new(base).exists() {
                let listing = Command::new("ls")
                    .args(["-la", base])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .unwrap_or_else(|e| format!("ls failed: {}", e));
                sections.push(format!("=== {} ===\n{}", base, listing));
            }
        }

        let output = if sections.is_empty() {
            format!(
                "No data directory found for '{}'. Searched:\n  ~/.config/{}\n  ~/.local/share/{}",
                app_name, app_name, app_name
            )
        } else {
            sections.join("\n\n")
        };

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "app_name": app_name, "raw_output": output }),
        ))
    }
}

// ── LinuxClearAppCache ────────────────────────────────────────────────

pub struct LinuxClearAppCache;

#[async_trait]
impl Tool for LinuxClearAppCache {
    fn name(&self) -> &str {
        "linux_clear_app_cache"
    }

    fn description(&self) -> &str {
        "Clear cache files for a specific application from ~/.cache/{app_name}/."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "Application name (subdirectory of ~/.cache)"
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
        let cache_dir = format!("{}/.cache/{}", home, app_name);

        if !std::path::Path::new(&cache_dir).exists() {
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

        // Remove contents, keep directory
        let output = Command::new("find")
            .args([&cache_dir, "-mindepth", "1", "-maxdepth", "1", "-exec", "rm", "-rf", "{}", ";"])
            .output()
            .map(|o| {
                if o.status.success() {
                    format!("Cleared cache for '{}' (was {}).", app_name, before_size)
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("Some files cleared (some may be in use): {}", stderr.trim())
                }
            })
            .unwrap_or_else(|e| format!("Failed to clear cache: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({
                "app_name": app_name,
                "cache_dir": cache_dir,
                "before_size": before_size,
            }),
            vec![ChangeRecord {
                description: format!("Cleared cache for '{}' from {}", app_name, cache_dir),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}
