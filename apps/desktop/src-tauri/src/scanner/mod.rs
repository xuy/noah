pub mod backups;
pub mod disk;
pub mod network;
pub mod performance;
pub mod security;
pub mod updates;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::safety::journal;

// ── Scanner trait ────────────────────────────────────────────────────

/// Progress returned by a single scanner tick.
#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub progress_pct: i32,
    pub detail: String,
    pub done: bool,
}

/// Trait for background scanners. Each scanner writes to system_scan_results.
pub trait Scanner: Send + Sync {
    fn scan_type(&self) -> &str;
    fn display_name(&self) -> &str;

    /// Run one tick of scanning work within the given time budget.
    /// `conn` is provided for reading/writing scan state and results.
    fn tick(&self, budget: Duration, conn: &Connection) -> Result<ScanProgress>;

    /// Check if the system is idle enough to scan.
    fn is_system_idle(&self) -> bool;
}

// ── Scanner Manager ──────────────────────────────────────────────────

/// Event payload emitted to the frontend for scan progress updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgressEvent {
    pub scan_type: String,
    pub display_name: String,
    pub status: String,
    pub progress_pct: i32,
    pub progress_detail: String,
}

/// Manages registered scanners and runs them with time budgets.
pub struct ScannerManager {
    scanners: Vec<Box<dyn Scanner>>,
    db: Arc<Mutex<Connection>>,
    app_handle: Option<tauri::AppHandle>,
    /// If true, a scan was requested on-demand (higher budget).
    trigger_requested: Arc<std::sync::Mutex<Option<String>>>,
    /// If set, the scanner should pause.
    pause_requested: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

impl ScannerManager {
    pub fn new(db: Arc<Mutex<Connection>>, app_handle: Option<tauri::AppHandle>) -> Self {
        Self {
            scanners: Vec::new(),
            db,
            app_handle,
            trigger_requested: Arc::new(std::sync::Mutex::new(None)),
            pause_requested: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        }
    }

    pub fn register(&mut self, scanner: Box<dyn Scanner>) {
        self.scanners.push(scanner);
    }

    /// Get shared handles for trigger/pause control from Tauri commands.
    pub fn trigger_handle(&self) -> Arc<std::sync::Mutex<Option<String>>> {
        self.trigger_requested.clone()
    }

    pub fn pause_handle(&self) -> Arc<std::sync::Mutex<std::collections::HashSet<String>>> {
        self.pause_requested.clone()
    }

    /// Run one cycle: give each registered scanner a time budget.
    pub async fn run_cycle(&self, budget_per_scanner: Duration) {
        for scanner in &self.scanners {
            let scan_type = scanner.scan_type().to_string();

            // Check if paused.
            {
                let Ok(paused) = self.pause_requested.lock() else { continue };
                if paused.contains(&scan_type) {
                    self.emit_progress(&scan_type, scanner.display_name(), "paused", 0, "Paused by user");
                    continue;
                }
            }

            // Skip if a completed scan exists from less than 1 hour ago.
            {
                let conn = self.db.lock().await;
                if let Ok(Some(latest)) = journal::get_latest_scan_job(&conn, &scan_type) {
                    if latest.status == "completed" {
                        if let Some(ref ts) = latest.completed_at {
                            if let Ok(completed) = chrono::DateTime::parse_from_rfc3339(ts) {
                                let elapsed = chrono::Utc::now() - completed.to_utc();
                                if elapsed < chrono::Duration::hours(1) {
                                    eprintln!("[scanner] {} skipped: completed {}m ago", scan_type, elapsed.num_minutes());
                                    continue;
                                }
                            }
                        }
                    }
                }
            }

            // Check system load.
            if !scanner.is_system_idle() {
                eprintln!("[scanner] {} skipped: system busy", scan_type);
                self.emit_progress(&scan_type, scanner.display_name(), "skipped", 0, "System busy");
                continue;
            }

            // Create or update the scan job record.
            let job_id = {
                let conn = self.db.lock().await;
                let existing = journal::get_latest_scan_job(&conn, &scan_type).ok().flatten();
                let job_id = existing
                    .as_ref()
                    .filter(|j| j.status == "running" || j.status == "paused")
                    .map(|j| j.id.clone())
                    .unwrap_or_else(|| Uuid::new_v4().to_string());

                let now = chrono::Utc::now().to_rfc3339();
                let job = journal::ScanJobRecord {
                    id: job_id.clone(),
                    scan_type: scan_type.clone(),
                    status: "running".to_string(),
                    progress_pct: existing.as_ref().map(|j| j.progress_pct).unwrap_or(0),
                    progress_detail: Some("Starting...".to_string()),
                    budget_secs: Some(budget_per_scanner.as_secs() as i32),
                    started_at: existing.as_ref().and_then(|j| j.started_at.clone()).or_else(|| Some(now.clone())),
                    updated_at: Some(now),
                    completed_at: None,
                    config: existing.as_ref().and_then(|j| j.config.clone()),
                };
                let _ = journal::upsert_scan_job(&conn, &job);
                job_id
            };

            self.emit_progress(&scan_type, scanner.display_name(), "running", 0, "Starting...");

            // Run the tick.
            let progress = {
                let conn = self.db.lock().await;
                scanner.tick(budget_per_scanner, &conn)
            };

            // Update job record with result.
            let conn = self.db.lock().await;
            let now = chrono::Utc::now().to_rfc3339();
            match progress {
                Ok(p) => {
                    // A tick always runs to completion (budget exhausted or scan finished).
                    // Mark as "completed" either way — progress state is saved in config
                    // so the next cycle resumes where we left off.
                    let job = journal::ScanJobRecord {
                        id: job_id,
                        scan_type: scan_type.clone(),
                        status: "completed".to_string(),
                        progress_pct: p.progress_pct,
                        progress_detail: Some(p.detail.clone()),
                        budget_secs: Some(budget_per_scanner.as_secs() as i32),
                        started_at: None, // preserve existing
                        updated_at: Some(now.clone()),
                        completed_at: Some(now),
                        config: None,
                    };
                    let _ = journal::upsert_scan_job(&conn, &job);
                    self.emit_progress(&scan_type, scanner.display_name(), "completed", p.progress_pct, &p.detail);
                }
                Err(e) => {
                    eprintln!("[scanner] {} tick failed: {}", scan_type, e);
                    let job = journal::ScanJobRecord {
                        id: job_id,
                        scan_type: scan_type.clone(),
                        status: "failed".to_string(),
                        progress_pct: 0,
                        progress_detail: Some(format!("Error: {}", e)),
                        budget_secs: Some(budget_per_scanner.as_secs() as i32),
                        started_at: None,
                        updated_at: Some(now),
                        completed_at: None,
                        config: None,
                    };
                    let _ = journal::upsert_scan_job(&conn, &job);
                    self.emit_progress(&scan_type, scanner.display_name(), "failed", 0, &format!("Error: {}", e));
                }
            }
        }
    }

    /// Check if an on-demand scan was requested and run it with higher budget.
    pub async fn run_triggered(&self) {
        let requested = {
            let Ok(mut trigger) = self.trigger_requested.lock() else { return };
            trigger.take()
        };

        if let Some(scan_type) = requested {
            // Find the matching scanner and run with 5 min budget.
            for scanner in &self.scanners {
                if scanner.scan_type() == scan_type {
                    eprintln!("[scanner] on-demand scan triggered for {}", scan_type);
                    let budget = Duration::from_secs(300);
                    let conn = self.db.lock().await;

                    let job_id = Uuid::new_v4().to_string();
                    let now = chrono::Utc::now().to_rfc3339();
                    let job = journal::ScanJobRecord {
                        id: job_id.clone(),
                        scan_type: scan_type.clone(),
                        status: "running".to_string(),
                        progress_pct: 0,
                        progress_detail: Some("On-demand scan starting...".to_string()),
                        budget_secs: Some(budget.as_secs() as i32),
                        started_at: Some(now.clone()),
                        updated_at: Some(now),
                        completed_at: None,
                        config: None,
                    };
                    let _ = journal::upsert_scan_job(&conn, &job);
                    self.emit_progress(&scan_type, scanner.display_name(), "running", 0, "On-demand scan starting...");

                    match scanner.tick(budget, &conn) {
                        Ok(p) => {
                            let status = if p.done { "completed" } else { "running" };
                            let now2 = chrono::Utc::now().to_rfc3339();
                            let job = journal::ScanJobRecord {
                                id: job_id,
                                scan_type: scan_type.clone(),
                                status: status.to_string(),
                                progress_pct: p.progress_pct,
                                progress_detail: Some(p.detail.clone()),
                                budget_secs: Some(budget.as_secs() as i32),
                                started_at: None,
                                updated_at: Some(now2.clone()),
                                completed_at: if p.done { Some(now2) } else { None },
                                config: None,
                            };
                            let _ = journal::upsert_scan_job(&conn, &job);
                            self.emit_progress(&scan_type, scanner.display_name(), status, p.progress_pct, &p.detail);
                        }
                        Err(e) => {
                            eprintln!("[scanner] on-demand {} failed: {}", scan_type, e);
                            let now2 = chrono::Utc::now().to_rfc3339();
                            let job = journal::ScanJobRecord {
                                id: job_id,
                                scan_type: scan_type.clone(),
                                status: "failed".to_string(),
                                progress_pct: 0,
                                progress_detail: Some(format!("Error: {}", e)),
                                budget_secs: Some(budget.as_secs() as i32),
                                started_at: None,
                                updated_at: Some(now2.clone()),
                                completed_at: Some(now2),
                                config: None,
                            };
                            let _ = journal::upsert_scan_job(&conn, &job);
                            self.emit_progress(&scan_type, scanner.display_name(), "failed", 0, &format!("Error: {}", e));
                        }
                    }
                    break;
                }
            }
        }
    }

    fn emit_progress(&self, scan_type: &str, display_name: &str, status: &str, pct: i32, detail: &str) {
        if let Some(ref handle) = self.app_handle {
            let payload = ScanProgressEvent {
                scan_type: scan_type.to_string(),
                display_name: display_name.to_string(),
                status: status.to_string(),
                progress_pct: pct,
                progress_detail: detail.to_string(),
            };
            let _ = handle.emit("scanner-progress", &payload);
        }
    }
}
