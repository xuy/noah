use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use itman_tools::{SafetyTier, Tool, ToolResult};

// ── DiskAudit ─────────────────────────────────────────────────────────

pub struct DiskAudit;

/// Directories to scan, with human-readable labels.
const SCAN_TARGETS: &[(&str, &str)] = &[
    ("~/Library/Caches", "System & App Caches"),
    ("~/Downloads", "Downloads"),
    ("~/.Trash", "Trash"),
    (
        "~/Library/Developer/Xcode/DerivedData",
        "Xcode DerivedData",
    ),
    (
        "~/Library/Application Support/MobileSync/Backup",
        "iOS Device Backups",
    ),
    (
        "~/Library/Containers/com.docker.docker",
        "Docker Data",
    ),
    ("~/Library/Caches/Homebrew", "Homebrew Cache"),
    ("~/.npm", "npm Cache"),
    ("~/.yarn/cache", "Yarn Cache"),
    ("~/Library/Caches/pip", "pip Cache"),
    (
        "~/Library/Application Support/Code/CachedData",
        "VS Code Cache",
    ),
    ("/Library/Updates", "macOS Update Downloads"),
];

/// Get the size of a directory in bytes using `du -sk`.
fn dir_size_kb(path: &str) -> Option<u64> {
    let output = Command::new("du")
        .args(["-sk", path])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let size_str = stdout.split_whitespace().next()?;
    size_str.parse::<u64>().ok()
}

/// Format kilobytes as a human-readable string.
fn format_size(kb: u64) -> String {
    if kb >= 1_048_576 {
        format!("{:.1} GB", kb as f64 / 1_048_576.0)
    } else if kb >= 1024 {
        format!("{:.1} MB", kb as f64 / 1024.0)
    } else {
        format!("{} KB", kb)
    }
}

/// Expand ~ to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/{}", home, rest);
        }
    }
    path.to_string()
}

#[async_trait]
impl Tool for DiskAudit {
    fn name(&self) -> &str {
        "disk_audit"
    }

    fn description(&self) -> &str {
        "Scan known space-hogging directories (caches, downloads, Xcode, Docker, iOS backups, etc.) and return a categorized breakdown sorted by size."
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
        let mut results: Vec<(String, String, u64)> = Vec::new();

        for (path_template, label) in SCAN_TARGETS {
            let path = expand_tilde(path_template);
            if let Some(size_kb) = dir_size_kb(&path) {
                if size_kb > 0 {
                    results.push((label.to_string(), path, size_kb));
                }
            }
        }

        // Check Time Machine local snapshots.
        let tm_output = Command::new("tmutil")
            .args(["listlocalsnapshots", "/"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        let snapshot_count = tm_output.lines().filter(|l| l.contains("com.apple.TimeMachine")).count();

        // Sort by size (largest first).
        results.sort_by(|a, b| b.2.cmp(&a.2));

        let total_kb: u64 = results.iter().map(|(_, _, kb)| kb).sum();

        // Build output.
        let mut output_lines = Vec::new();
        output_lines.push(format!(
            "=== Disk Space Audit ===\nTotal scannable space used: {}\n",
            format_size(total_kb)
        ));

        if results.is_empty() {
            output_lines.push("No significant space usage found in common locations.".to_string());
        } else {
            for (label, path, size_kb) in &results {
                output_lines.push(format!(
                    "  {:>10}  {}  ({})",
                    format_size(*size_kb),
                    label,
                    path
                ));
            }
        }

        if snapshot_count > 0 {
            output_lines.push(format!(
                "\nTime Machine local snapshots: {} snapshot(s) found (managed by macOS, auto-purged when space is needed)",
                snapshot_count
            ));
        }

        let output = output_lines.join("\n");

        let json_results: Vec<Value> = results
            .iter()
            .map(|(label, path, kb)| {
                json!({
                    "label": label,
                    "path": path,
                    "size_kb": kb,
                    "size_human": format_size(*kb),
                })
            })
            .collect();

        Ok(ToolResult::read_only(
            output,
            json!({
                "total_kb": total_kb,
                "total_human": format_size(total_kb),
                "entries": json_results,
                "time_machine_snapshots": snapshot_count,
            }),
        ))
    }
}
