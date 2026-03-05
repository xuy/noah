use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

use itman_tools::{SafetyTier, Tool, ToolResult};

// ── CrashLogReader ────────────────────────────────────────────────────

pub struct CrashLogReader;

/// Standard crash report directories on macOS.
const CRASH_DIRS: &[&str] = &[
    "~/Library/Logs/DiagnosticReports",
    "/Library/Logs/DiagnosticReports",
];

/// Expand ~ to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

/// Extract a concise summary from a crash report (.ips or .crash file).
fn summarize_crash_report(content: &str, filename: &str) -> String {
    let mut summary_lines = Vec::new();
    summary_lines.push(format!("--- {} ---", filename));

    // Try to extract key fields.
    let mut process_name = None;
    let mut exception_type = None;
    let mut exception_codes = None;
    let mut crashed_thread = None;
    let mut in_crashed_thread = false;
    let mut stack_frames: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(val) = trimmed.strip_prefix("Process:") {
            process_name = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("Exception Type:") {
            exception_type = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("Exception Codes:") {
            exception_codes = Some(val.trim().to_string());
        } else if trimmed.starts_with("Crashed Thread:") {
            crashed_thread = Some(trimmed.to_string());
            // Look for the crashed thread's stack trace.
        } else if trimmed.starts_with("Thread") && trimmed.contains("Crashed") {
            in_crashed_thread = true;
        } else if in_crashed_thread {
            if trimmed.is_empty() || (trimmed.starts_with("Thread") && !trimmed.contains("Crashed"))
            {
                in_crashed_thread = false;
            } else if stack_frames.len() < 10 {
                stack_frames.push(trimmed.to_string());
            }
        }
    }

    // Also try JSON-format .ips files (macOS 12+).
    if exception_type.is_none() && content.starts_with('{') {
        if let Ok(parsed) = serde_json::from_str::<Value>(content) {
            if let Some(exc) = parsed.get("exception").and_then(|e| e.get("type")) {
                exception_type = Some(exc.to_string());
            }
            if let Some(proc) = parsed.get("procName") {
                process_name = Some(proc.as_str().unwrap_or("unknown").to_string());
            }
        }
    }

    if let Some(proc) = &process_name {
        summary_lines.push(format!("Process: {}", proc));
    }
    if let Some(exc) = &exception_type {
        summary_lines.push(format!("Exception Type: {}", exc));
    }
    if let Some(codes) = &exception_codes {
        summary_lines.push(format!("Exception Codes: {}", codes));
    }
    if let Some(ct) = &crashed_thread {
        summary_lines.push(ct.clone());
    }
    if !stack_frames.is_empty() {
        summary_lines.push("Top stack frames:".to_string());
        for frame in &stack_frames {
            summary_lines.push(format!("  {}", frame));
        }
    }

    if process_name.is_none() && exception_type.is_none() {
        // Fallback: show first 20 non-empty lines.
        summary_lines.push("(Could not parse structured data, showing first lines:)".to_string());
        for line in content.lines().take(20) {
            if !line.trim().is_empty() {
                summary_lines.push(line.to_string());
            }
        }
    }

    summary_lines.join("\n")
}

#[async_trait]
impl Tool for CrashLogReader {
    fn name(&self) -> &str {
        "crash_log_reader"
    }

    fn description(&self) -> &str {
        "Read crash logs for an app or a specific log file. Scans DiagnosticReports for matching .ips/.crash files and extracts exception type, crashed thread, and top stack frames."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "Application name to search for in crash reports (e.g. 'Safari', 'cups')"
                },
                "log_path": {
                    "type": "string",
                    "description": "Specific log file path to read (e.g. '/var/log/cups/error_log'). If provided, reads this file directly instead of searching DiagnosticReports."
                }
            },
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let app_name = input["app_name"].as_str();
        let log_path = input["log_path"].as_str();

        // If a specific log path is given, read it directly.
        if let Some(path) = log_path {
            let expanded = expand_tilde(path);
            match std::fs::read_to_string(&expanded) {
                Ok(content) => {
                    // For large logs, return the last 100 lines.
                    let lines: Vec<&str> = content.lines().collect();
                    let tail = if lines.len() > 100 {
                        format!(
                            "(Showing last 100 of {} lines)\n{}",
                            lines.len(),
                            lines[lines.len() - 100..].join("\n")
                        )
                    } else {
                        content.clone()
                    };

                    return Ok(ToolResult::read_only(
                        tail,
                        json!({
                            "path": path,
                            "total_lines": lines.len(),
                        }),
                    ));
                }
                Err(e) => {
                    // Try with sudo for system logs.
                    let output = Command::new("tail")
                        .args(["-100", &expanded.to_string_lossy()])
                        .output();

                    match output {
                        Ok(o) if o.status.success() => {
                            let text = String::from_utf8_lossy(&o.stdout).to_string();
                            return Ok(ToolResult::read_only(
                                text,
                                json!({ "path": path }),
                            ));
                        }
                        _ => {
                            return Ok(ToolResult::read_only(
                                format!("Cannot read {}: {}", path, e),
                                json!({ "error": format!("{}", e) }),
                            ));
                        }
                    }
                }
            }
        }

        // Search for crash reports matching app_name.
        let search_name = app_name.unwrap_or("").to_lowercase();
        if search_name.is_empty() {
            anyhow::bail!(
                "Either 'app_name' or 'log_path' must be provided."
            );
        }

        let mut reports: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for dir_template in CRASH_DIRS {
            let dir = expand_tilde(dir_template);
            if !dir.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase();

                    // Match .ips, .crash, or .diag files containing the app name.
                    if (name.ends_with(".ips")
                        || name.ends_with(".crash")
                        || name.ends_with(".diag"))
                        && name.contains(&search_name)
                    {
                        let modified = entry
                            .metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .unwrap_or(std::time::UNIX_EPOCH);
                        reports.push((path, modified));
                    }
                }
            }
        }

        if reports.is_empty() {
            return Ok(ToolResult::read_only(
                format!("No crash reports found for '{}'.", search_name),
                json!({ "app_name": search_name, "reports_found": 0 }),
            ));
        }

        // Sort by modification time (newest first) and take up to 3.
        reports.sort_by(|a, b| b.1.cmp(&a.1));
        let reports = &reports[..reports.len().min(3)];

        let mut summaries = Vec::new();
        for (path, _) in reports {
            if let Ok(content) = std::fs::read_to_string(path) {
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                summaries.push(summarize_crash_report(&content, &filename));
            }
        }

        let output = format!(
            "Found {} crash report(s) for '{}':\n\n{}",
            reports.len(),
            search_name,
            summaries.join("\n\n")
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "app_name": search_name,
                "reports_found": reports.len(),
            }),
        ))
    }
}
