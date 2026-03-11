use std::collections::VecDeque;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::safety::journal;

use super::{ScanProgress, Scanner};

// ── macOS TCC-protected directories ────────────────────────────────
// Accessing these triggers scary OS permission popups (e.g. "Noah wants
// to access your Music").  We skip them entirely during disk scans.

/// Directory names (lowercase) under $HOME that are TCC-protected on macOS.
#[cfg(target_os = "macos")]
const MACOS_PRIVATE_DIRS: &[&str] = &[
    "music",
    "pictures",
    "photos",
    "movies",
    "desktop",
    "documents",
];

/// Path components (lowercase) anywhere in a path that are TCC-protected.
#[cfg(target_os = "macos")]
const MACOS_PRIVATE_PATHS: &[&str] = &[
    "/library/mail",
    "/library/messages",
    "/library/calendars",
    "/library/contacts",
    "/library/safari",
    "/library/suggestions",
    "/library/homekit",
    "photos library.photoslibrary",
];

/// Returns true if `path` is a macOS TCC-protected directory that would
/// trigger a permission prompt.
fn is_macos_private(path: &str, home: &str) -> bool {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (path, home);
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        let lower = path.to_lowercase();
        let home_lower = home.to_lowercase();

        // Check top-level home dirs: ~/Music, ~/Pictures, etc.
        for dir_name in MACOS_PRIVATE_DIRS {
            let protected = format!("{}/{}", home_lower, dir_name);
            if lower == protected || lower.starts_with(&format!("{}/", protected)) {
                return true;
            }
        }

        // Check path components anywhere: ~/Library/Mail, etc.
        for component in MACOS_PRIVATE_PATHS {
            if lower.contains(component) {
                return true;
            }
        }

        false
    }
}

// ── Category heuristics ──────────────────────────────────────────────

fn categorize_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();

    if lower.contains("/caches/") || lower.contains("/.cache/") || lower.contains("/cache/") || lower.contains("/tmp/") || lower.contains("/var/folders/") {
        return "cache";
    }
    if lower.contains("node_modules") || lower.contains("/target/debug") || lower.contains("/target/release")
        || lower.contains("deriveddata") || lower.contains("/.gradle/") || lower.contains("/build/")
        || lower.contains("/.venv/") || lower.contains("__pycache__")
    {
        return "build_artifact";
    }
    if lower.contains("/downloads") {
        return "download";
    }
    if lower.contains("/.npm") || lower.contains("/.cargo/registry") || lower.contains("/.yarn/cache")
        || lower.contains("/homebrew/") || lower.contains("/.nuget/") || lower.contains("/pip/")
    {
        return "package_cache";
    }
    if lower.contains("mobilesync/backup") || lower.contains("/.trash") || lower.contains("/trash/") {
        return "backup";
    }
    if lower.contains("coresimulator") || lower.contains("ios devicesupport") || lower.contains("docker") {
        return "devtools";
    }
    if lower.contains("/movies/") || lower.contains("/music/") || lower.contains("/pictures/") || lower.contains("/photos") {
        return "media";
    }

    "other"
}

// ── Scan state persisted in scan_jobs.config ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskScanState {
    /// Queue of directories still to scan (breadth-first).
    queue: VecDeque<String>,
    /// Total top-level dirs we started with (for progress calculation).
    total_top_level: usize,
    /// How many top-level dirs we've finished.
    completed_top_level: usize,
    /// Current generation counter.
    generation: i64,
}

impl Default for DiskScanState {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            total_top_level: 0,
            completed_top_level: 0,
            generation: 0,
        }
    }
}

// ── DiskScanner ──────────────────────────────────────────────────────

pub struct DiskScanner;

impl DiskScanner {
    /// Get the size of immediate children of `dir` using `du -d1 -k`.
    /// Runs under `nice -n 19` for low priority.
    /// Returns Vec<(path, size_kb)> sorted by size descending.
    fn du_children(dir: &str) -> Result<Vec<(String, u64)>> {
        let output = Command::new("nice")
            .args(["-n", "19", "du", "-d1", "-k", dir])
            .output()
            .with_context(|| format!("Failed to run du on {}", dir))?;

        if !output.status.success() {
            // du often has permission errors on some dirs — still parse what we got.
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results: Vec<(String, u64)> = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(2, |c: char| c == '\t' || c == ' ').collect();
            if parts.len() == 2 {
                // du output is: SIZE\tPATH — but there may be extra spaces.
                let size_str = parts[0].trim();
                let path = parts[1].trim().to_string();
                if let Ok(size_kb) = size_str.parse::<u64>() {
                    // Skip the parent dir summary (same as `dir` itself).
                    if path != dir {
                        results.push((path, size_kb));
                    }
                }
            }
        }

        results.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(results)
    }

    /// Check macOS system load average.
    fn load_average() -> f64 {
        let output = Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .ok();

        if let Some(out) = output {
            let s = String::from_utf8_lossy(&out.stdout);
            // Format: "{ 1.23 4.56 7.89 }"
            let parts: Vec<&str> = s.trim().trim_matches(|c| c == '{' || c == '}').split_whitespace().collect();
            if let Some(first) = parts.first() {
                return first.parse::<f64>().unwrap_or(0.0);
            }
        }
        0.0
    }

    fn home_dir() -> String {
        std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".to_string())
    }

    /// Load or initialize scan state from the latest scan job's config.
    fn load_state(conn: &Connection) -> DiskScanState {
        if let Ok(Some(job)) = journal::get_latest_scan_job(conn, "disk") {
            if let Some(config) = &job.config {
                if let Ok(state) = serde_json::from_str::<DiskScanState>(config) {
                    if !state.queue.is_empty() {
                        return state;
                    }
                }
            }
        }
        DiskScanState::default()
    }

    /// Save state back to the scan job config field.
    fn save_state(conn: &Connection, state: &DiskScanState) {
        if let Ok(Some(mut job)) = journal::get_latest_scan_job(conn, "disk") {
            job.config = serde_json::to_string(state).ok();
            job.updated_at = Some(chrono::Utc::now().to_rfc3339());
            let _ = journal::upsert_scan_job(conn, &job);
        }
    }

    /// Check if a path looks stale (last accessed > 90 days ago).
    fn is_stale(path: &str) -> bool {
        if let Ok(metadata) = std::fs::metadata(path) {
            if let Ok(accessed) = metadata.accessed() {
                if let Ok(elapsed) = accessed.elapsed() {
                    return elapsed > Duration::from_secs(90 * 24 * 3600);
                }
            }
        }
        false
    }
}

impl Scanner for DiskScanner {
    fn scan_type(&self) -> &str {
        "disk"
    }

    fn display_name(&self) -> &str {
        "Disk Analysis"
    }

    fn is_system_idle(&self) -> bool {
        Self::load_average() < 4.0
    }

    fn tick(&self, budget: Duration, conn: &Connection) -> Result<ScanProgress> {
        let start = Instant::now();
        let mut state = Self::load_state(conn);

        // If queue is empty, this is a fresh scan — seed with home dir children.
        if state.queue.is_empty() {
            state.generation += 1;
            let home = Self::home_dir();
            eprintln!("[disk_scanner] starting fresh scan of {}", home);

            let children = Self::du_children(&home)?;

            // Filter out macOS TCC-protected directories.
            let children: Vec<_> = children
                .into_iter()
                .filter(|(path, _)| !is_macos_private(path, &home))
                .collect();

            // Store top-level results immediately.
            let results: Vec<_> = children
                .iter()
                .map(|(path, size_kb)| {
                    let cat = categorize_path(path);
                    let stale = Self::is_stale(path);
                    let label = path.rsplit('/').next().unwrap_or(path).to_string();
                    (
                        path.clone(),
                        Some(cat.to_string()),
                        Some(label),
                        Some(*size_kb as f64),
                        Some(format_size(*size_kb)),
                        None::<String>,
                        stale,
                        state.generation,
                    )
                })
                .collect();

            journal::upsert_scan_results(conn, "disk", &results)?;

            // Queue children >1GB for deeper scan.
            let threshold_kb = 1_048_576; // 1 GB
            state.queue = children
                .into_iter()
                .filter(|(_, size)| *size > threshold_kb)
                .map(|(path, _)| path)
                .collect();
            state.total_top_level = state.queue.len();
            state.completed_top_level = 0;

            Self::save_state(conn, &state);

            if state.queue.is_empty() {
                return Ok(ScanProgress {
                    progress_pct: 100,
                    detail: "Scan complete (no large directories to explore)".to_string(),
                    done: true,
                });
            }
        }

        // Process queue entries until budget exhausted.
        while !state.queue.is_empty() {
            // Check budget.
            if start.elapsed() >= budget {
                break;
            }

            // Check load every few dirs.
            if Self::load_average() > 4.0 {
                eprintln!("[disk_scanner] pausing: system load high");
                break;
            }

            let dir = state.queue.pop_front().unwrap();
            eprintln!("[disk_scanner] scanning {}", dir);

            match Self::du_children(&dir) {
                Ok(children) => {
                    let results: Vec<_> = children
                        .iter()
                        .map(|(path, size_kb)| {
                            let cat = categorize_path(path);
                            let stale = Self::is_stale(path);
                            let label = path.rsplit('/').next().unwrap_or(path).to_string();
                            (
                                path.clone(),
                                Some(cat.to_string()),
                                Some(label),
                                Some(*size_kb as f64),
                                Some(format_size(*size_kb)),
                                None::<String>,
                                stale,
                                state.generation,
                            )
                        })
                        .collect();

                    if !results.is_empty() {
                        let _ = journal::upsert_scan_results(conn, "disk", &results);
                    }

                    // Queue sub-children >1GB for even deeper scan.
                    let home = Self::home_dir();
                    let threshold_kb = 1_048_576;
                    for (path, size) in &children {
                        if *size > threshold_kb && !is_macos_private(path, &home) {
                            state.queue.push_back(path.clone());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[disk_scanner] du failed for {}: {}", dir, e);
                }
            }

            state.completed_top_level += 1;
        }

        let done = state.queue.is_empty();
        let progress_pct = if state.total_top_level > 0 {
            ((state.completed_top_level as f64 / state.total_top_level as f64) * 100.0) as i32
        } else {
            if done { 100 } else { 0 }
        };

        let detail = if done {
            "Scan complete".to_string()
        } else if let Some(next) = state.queue.front() {
            // Shorten path for display.
            let short = next.replace(&Self::home_dir(), "~");
            format!("Scanning {} ({} dirs remaining)", short, state.queue.len())
        } else {
            "Processing...".to_string()
        };

        Self::save_state(conn, &state);

        Ok(ScanProgress {
            progress_pct: progress_pct.min(100),
            detail,
            done,
        })
    }
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
