use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use itman_tools::{ChangeRecord, SafetyTier, Tool, ToolResult};

// ── LinuxSystemSummary ────────────────────────────────────────────────

pub struct LinuxSystemSummary;

#[async_trait]
impl Tool for LinuxSystemSummary {
    fn name(&self) -> &str {
        "linux_system_summary"
    }

    fn description(&self) -> &str {
        "One-shot system summary: distro, kernel, hardware, disk space, network status, and uptime."
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
        let hostname = Command::new("hostname")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("error: {}", e));

        let distro = std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|c| {
                c.lines()
                    .find(|l| l.starts_with("PRETTY_NAME="))
                    .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "Unknown Linux".to_string());

        let kernel = Command::new("uname")
            .arg("-r")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("error: {}", e));

        let uptime = Command::new("uptime")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("error: {}", e));

        // CPU — read /proc/cpuinfo directly (always available).
        let cpu = std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("model name"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());

        // Memory — read /proc/meminfo directly (always available).
        let memory = super::performance::read_mem_info();

        let disk = Command::new("df")
            .args(["-h", "/"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("error: {}", e));

        let ip = Command::new("ip")
            .args(["route", "get", "8.8.8.8"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("error: {}", e));

        let output = format!(
            "=== System Summary ===\n\
             Hostname: {}\n\
             Distribution: {}\n\
             Kernel: {}\n\
             CPU: {}\n\
             Memory: {}\n\
             Uptime: {}\n\n\
             === Disk (/) ===\n{}\n\n\
             === Network ===\n{}",
            hostname, distro, kernel, cpu, memory, uptime, disk, ip
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "hostname": hostname,
                "distro": distro,
                "kernel": kernel,
                "cpu": cpu,
                "memory": memory,
                "uptime": uptime,
                "disk": disk,
                "network": ip,
            }),
        ))
    }
}

// ── LinuxReadFile ─────────────────────────────────────────────────────

pub struct LinuxReadFile;

const FORBIDDEN_PATH_PREFIXES: &[&str] = &[
    "/proc/kcore",
    "/proc/kmem",
    "/dev/",
    "/boot/",
];

const ALLOWED_PATH_PREFIXES: &[&str] = &[
    "/home/",
    "/tmp/",
    "/var/log/",
    "/var/tmp/",
    "/etc/",
    "/usr/local/",
    "/opt/",
    "/proc/",
    "/sys/class/",
];

fn is_path_allowed(path: &str) -> bool {
    for prefix in FORBIDDEN_PATH_PREFIXES {
        if path.starts_with(prefix) {
            return false;
        }
    }
    for prefix in ALLOWED_PATH_PREFIXES {
        if path.starts_with(prefix) {
            return true;
        }
    }
    false
}

#[async_trait]
impl Tool for LinuxReadFile {
    fn name(&self) -> &str {
        "linux_read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. The path must be under a user-accessible location (/home/*, /var/log/*, /etc/*, /tmp/*, /opt/*, etc.). System-protected paths are rejected."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                }
            },
            "required": ["path"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: path"))?;

        let canonical = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string());

        if !is_path_allowed(&canonical) {
            return Ok(ToolResult::read_only(
                format!(
                    "Access denied: '{}' is outside the allowed scope. Allowed locations include /home/*, /var/log/*, /etc/*, /tmp/*, /opt/*.",
                    path
                ),
                json!({ "error": "access_denied", "path": path }),
            ));
        }

        match std::fs::read_to_string(&canonical) {
            Ok(contents) => {
                let lines: Vec<&str> = contents.lines().collect();
                let truncated = if lines.len() > 500 {
                    format!(
                        "... (showing first 500 of {} lines)\n{}",
                        lines.len(),
                        lines[..500].join("\n")
                    )
                } else {
                    contents.clone()
                };

                Ok(ToolResult::read_only(
                    truncated,
                    json!({
                        "path": canonical,
                        "lines": lines.len(),
                        "size_bytes": contents.len(),
                    }),
                ))
            }
            Err(e) => Ok(ToolResult::read_only(
                format!("Failed to read '{}': {}", path, e),
                json!({ "error": e.to_string(), "path": path }),
            )),
        }
    }
}

// ── LinuxReadLog ──────────────────────────────────────────────────────

pub struct LinuxReadLog;

#[async_trait]
impl Tool for LinuxReadLog {
    fn name(&self) -> &str {
        "linux_read_log"
    }

    fn description(&self) -> &str {
        "Read system logs using journalctl. Supports filtering by unit, priority, and time."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "unit": {
                    "type": "string",
                    "description": "Systemd unit to filter by (e.g. 'cups', 'NetworkManager', 'sshd')"
                },
                "priority": {
                    "type": "string",
                    "description": "Minimum priority: emerg, alert, crit, err, warning, notice, info, debug. Default: warning",
                    "default": "warning"
                },
                "since": {
                    "type": "string",
                    "description": "How far back to look, e.g. '1h ago', '30min ago', 'today'. Default: '1h ago'",
                    "default": "1h ago"
                }
            },
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let since = input["since"].as_str().unwrap_or("1h ago");
        let priority = input["priority"].as_str().unwrap_or("warning");

        let mut args = vec![
            "--no-pager".to_string(),
            "--since".to_string(),
            since.to_string(),
            "-p".to_string(),
            priority.to_string(),
            "-n".to_string(),
            "200".to_string(),
        ];

        if let Some(unit) = input["unit"].as_str() {
            args.push("-u".to_string());
            args.push(unit.to_string());
        }

        let output = Command::new("journalctl")
            .args(&args)
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if stdout.trim().is_empty() {
                    format!(
                        "No log entries found with priority '{}' since '{}'.",
                        priority, since
                    )
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|e| format!("journalctl failed: {}. This system may not use systemd.", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({
                "since": since,
                "priority": priority,
                "raw_output": output,
            }),
        ))
    }
}

// ── ShellRun (Linux) ──────────────────────────────────────────────────

pub struct ShellRun;

const DANGEROUS_COMMAND_PATTERNS: &[&str] = &[
    "rm ",
    "rm\t",
    "rmdir ",
    "sudo ",
    "dd ",
    "mkfs",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "> /dev/",
    "chmod -R",
    "chmod 777",
    "chown -R",
    "| sh",
    "| bash",
    "killall ",
    "pkill ",
    "truncate ",
    "systemctl disable",
    "systemctl mask",
];

pub fn is_dangerous_command(command: &str) -> bool {
    let lower = command.to_lowercase();
    if lower.starts_with("rm ") || lower.starts_with("rm\t") || lower == "rm" {
        return true;
    }
    for pattern in DANGEROUS_COMMAND_PATTERNS {
        if lower.contains(&pattern.to_lowercase()) {
            return true;
        }
    }
    false
}

#[async_trait]
impl Tool for ShellRun {
    fn name(&self) -> &str {
        "shell_run"
    }

    fn description(&self) -> &str {
        "Execute a shell command via /bin/bash -c. Auto-approved for safe commands; dangerous commands (rm, sudo, dd, etc.) require user approval."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "reason": {
                    "type": "string",
                    "description": "Plain-language explanation of what this command does and why, written for a non-technical user."
                }
            },
            "required": ["command", "reason"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    fn safety_tier_for_input(&self, input: &Value) -> SafetyTier {
        if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
            if is_dangerous_command(command) {
                return SafetyTier::NeedsApproval;
            }
        }
        SafetyTier::SafeAction
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: command"))?;

        let output = match tokio::time::timeout(
            std::time::Duration::from_secs(60),
            tokio::process::Command::new("/bin/bash")
                .args(["-c", command])
                .output(),
        )
        .await
        {
            Ok(Ok(o)) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                let exit_code = o.status.code().unwrap_or(-1);

                let mut result = String::new();
                if !stdout.is_empty() {
                    result.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !result.is_empty() {
                        result.push_str("\n--- stderr ---\n");
                    }
                    result.push_str(&stderr);
                }
                if result.is_empty() {
                    result = format!("(no output, exit code: {})", exit_code);
                } else {
                    result.push_str(&format!("\n\n[exit code: {}]", exit_code));
                }
                result
            }
            Ok(Err(e)) => format!("Failed to execute command: {}", e),
            Err(_) => "Command timed out after 60 seconds. The command was taking too long and has been stopped.".to_string(),
        };

        let truncated = if output.len() > 10_000 {
            format!("{}...\n\n(output truncated at 10000 chars)", &output[..10_000])
        } else {
            output.clone()
        };

        Ok(ToolResult::with_changes(
            truncated,
            json!({ "command": command }),
            vec![ChangeRecord {
                description: format!("Executed shell command: {}", command),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}
