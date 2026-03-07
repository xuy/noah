use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

use itman_tools::{SafetyTier, Tool, ToolResult};

// ── DiskAudit ─────────────────────────────────────────────────────────

pub struct DiskAudit {
    /// Optional path to journal.db for reading cached scan results.
    db_path: Option<PathBuf>,
}

impl DiskAudit {
    /// Create a DiskAudit without DB access (used by proactive monitor).
    pub fn new() -> Self {
        Self { db_path: None }
    }

    /// Create a DiskAudit with DB access for cached scan results.
    pub fn with_db(db_path: PathBuf) -> Self {
        Self { db_path: Some(db_path) }
    }

    /// Try to get results from cached background scan data.
    /// Returns None if no fresh data (<24h) is available.
    fn try_cached_results(
        &self,
        target: Option<&str>,
        min_size_mb: Option<f64>,
    ) -> Option<(Vec<(String, String, u64)>, Option<String>)> {
        let db_path = self.db_path.as_ref()?;
        let conn = rusqlite::Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ).ok()?;

        // Check freshness: most recent scan result must be <24h old.
        let latest_ts: Option<String> = conn
            .prepare("SELECT MAX(scanned_at) FROM system_scan_results WHERE scan_type = 'disk'")
            .ok()?
            .query_map([], |row| row.get::<_, Option<String>>(0))
            .ok()?
            .next()?
            .ok()?;

        let latest_ts = latest_ts?;
        let scanned_at = chrono::DateTime::parse_from_rfc3339(&latest_ts).ok()?;
        let age = chrono::Utc::now() - scanned_at.to_utc();
        if age > chrono::Duration::hours(24) {
            return None;
        }

        // Build query.
        let min_kb = min_size_mb.map(|mb| mb * 1024.0).unwrap_or(0.0);

        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(prefix) = target {
            let expanded = expand_tilde(prefix);
            (
                "SELECT path, key, value_num FROM system_scan_results WHERE scan_type = 'disk' AND value_num >= ?1 AND path LIKE ?2 ORDER BY value_num DESC LIMIT 50".to_string(),
                vec![Box::new(min_kb), Box::new(format!("{}/%", expanded))],
            )
        } else {
            (
                "SELECT path, key, value_num FROM system_scan_results WHERE scan_type = 'disk' AND value_num >= ?1 ORDER BY value_num DESC LIMIT 50".to_string(),
                vec![Box::new(min_kb)],
            )
        };

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).ok()?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                ))
            })
            .ok()?;

        let mut results: Vec<(String, String, u64)> = Vec::new();
        for row in rows {
            if let Ok((path, label, size)) = row {
                let size_kb = size.unwrap_or(0.0) as u64;
                if size_kb > 0 {
                    let display_label = label.unwrap_or_else(|| {
                        path.rsplit('/').next().unwrap_or(&path).to_string()
                    });
                    results.push((display_label, path, size_kb));
                }
            }
        }

        if results.is_empty() {
            return None;
        }

        Some((results, Some(latest_ts)))
    }
}

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
    // Additional targets (from background scanner knowledge)
    (
        "~/Library/Developer/Xcode/iOS DeviceSupport",
        "Xcode iOS Device Support",
    ),
    (
        "~/Library/Developer/Xcode/Archives",
        "Xcode Archives",
    ),
    (
        "~/Library/Developer/CoreSimulator/Devices",
        "iOS Simulator Runtimes",
    ),
    ("~/.cargo/registry", "Cargo/Rust Cache"),
    ("~/.gradle/caches", "Gradle Cache"),
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
    if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return home;
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
        "Scan space-hogging directories and return a categorized breakdown sorted by size. If a background scan has run recently, returns comprehensive cached results instantly. Optional: pass 'target' to drill into a specific directory, or 'min_size_mb' to filter by minimum size."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Optional directory to drill into (e.g. '~/Library'). Shows children of this directory sorted by size."
                },
                "min_size_mb": {
                    "type": "number",
                    "description": "Optional minimum size in MB to include in results. Default: 0 (show all)."
                }
            },
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let target = input.get("target").and_then(|v| v.as_str());
        let min_size_mb = input.get("min_size_mb").and_then(|v| v.as_f64());

        // Try cached results first.
        if let Some((cached_results, scanned_at)) = self.try_cached_results(target, min_size_mb) {
            return build_output(cached_results, Some("(from background scan)"), scanned_at.as_deref());
        }

        // If target is specified but no cache, run du on that specific directory.
        if let Some(target_dir) = target {
            let expanded = expand_tilde(target_dir);
            let output = Command::new("nice")
                .args(["-n", "19", "du", "-d1", "-k", &expanded])
                .output()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut results: Vec<(String, String, u64)> = Vec::new();
            let min_kb = min_size_mb.map(|mb| (mb * 1024.0) as u64).unwrap_or(0);

            for line in stdout.lines() {
                let parts: Vec<&str> = line.splitn(2, |c: char| c == '\t' || c == ' ').collect();
                if parts.len() == 2 {
                    let size_str = parts[0].trim();
                    let path = parts[1].trim().to_string();
                    if let Ok(size_kb) = size_str.parse::<u64>() {
                        if path != expanded && size_kb > min_kb {
                            let label = path.rsplit('/').next().unwrap_or(&path).to_string();
                            results.push((label, path, size_kb));
                        }
                    }
                }
            }

            results.sort_by(|a, b| b.2.cmp(&a.2));
            return build_output(results, Some(&format!("(live scan of {})", target_dir)), None);
        }

        // Fallback: scan known directories (original behavior).
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

        results.sort_by(|a, b| b.2.cmp(&a.2));

        let total_kb: u64 = results.iter().map(|(_, _, kb)| kb).sum();

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

/// Build a ToolResult from a list of (label, path, size_kb) entries.
fn build_output(
    results: Vec<(String, String, u64)>,
    source_note: Option<&str>,
    scanned_at: Option<&str>,
) -> Result<ToolResult> {
    let total_kb: u64 = results.iter().map(|(_, _, kb)| kb).sum();

    let mut output_lines = Vec::new();
    let mut header = format!(
        "=== Disk Space Audit ===\nTotal: {}",
        format_size(total_kb)
    );
    if let Some(note) = source_note {
        header.push_str(&format!("  {}", note));
    }
    if let Some(ts) = scanned_at {
        header.push_str(&format!("  [scanned: {}]", ts));
    }
    output_lines.push(format!("{}\n", header));

    if results.is_empty() {
        output_lines.push("No significant space usage found.".to_string());
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
        }),
    ))
}
