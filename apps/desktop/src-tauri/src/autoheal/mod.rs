use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::agent::llm_client::LlmClient;
use crate::safety::journal;

/// Payload emitted to the frontend via auto-heal events.
#[derive(Debug, Clone, Serialize)]
pub struct AutoHealPayload {
    pub check_id: String,
    pub playbook_slug: String,
    pub reason: String,
}

/// Payload emitted when auto-heal completes.
#[derive(Debug, Clone, Serialize)]
pub struct AutoHealCompletePayload {
    pub check_id: String,
    pub playbook_slug: String,
    pub success: bool,
    pub score_before: Option<i32>,
    pub score_after: Option<i32>,
    pub error: Option<String>,
}

/// Payload emitted when auto-heal issues are available but auto-heal is off.
#[derive(Debug, Clone, Serialize)]
pub struct AutoHealAvailablePayload {
    pub check_id: String,
    pub playbook_slug: String,
    pub reason: String,
}

/// Background monitor that evaluates failing health checks and runs playbooks.
pub struct AutoHealMonitor {
    llm: LlmClient,
    db: Arc<Mutex<rusqlite::Connection>>,
    app_handle: tauri::AppHandle,
    app_dir: PathBuf,
}

impl AutoHealMonitor {
    pub fn new(
        llm: LlmClient,
        db: Arc<Mutex<rusqlite::Connection>>,
        app_handle: tauri::AppHandle,
        app_dir: PathBuf,
    ) -> Self {
        Self { llm, db, app_handle, app_dir }
    }

    /// Run forever: 10 min initial delay (offset from scanner), then every 6 hours.
    pub async fn run_forever(self) {
        // Initial delay — let scanner finish its first cycle.
        tokio::time::sleep(std::time::Duration::from_secs(600)).await;

        loop {
            if let Err(e) = self.evaluate().await {
                eprintln!("[autoheal] evaluation error: {}", e);
            }
            tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
        }
    }

    /// Evaluate failing health checks and potentially run a playbook.
    async fn evaluate(&self) -> anyhow::Result<()> {
        // Skip if no auth configured.
        if !self.llm.has_auth() {
            return Ok(());
        }

        // Check if auto-heal is enabled.
        let auto_heal_on = {
            let conn = self.db.lock().await;
            let value = journal::get_setting(&conn, "auto_heal_enabled")?;
            value.as_deref() == Some("true")
        };

        // Check concurrency guard.
        {
            let conn = self.db.lock().await;
            let running = journal::get_setting(&conn, "autoheal_running")?;
            if running.as_deref() == Some("true") {
                eprintln!("[autoheal] already running, skipping");
                return Ok(());
            }
        }

        // Read failing checks from system_scan_results.
        let failing_checks = {
            let conn = self.db.lock().await;
            let mut failures = Vec::new();
            let scan_types = ["security", "updates", "backups", "performance", "network"];
            for scan_type in &scan_types {
                if let Ok(results) = journal::query_scan_results(&conn, scan_type, None, None, None, 100) {
                    for r in results {
                        if r.value_text.as_deref() != Some("pass") {
                            failures.push(r);
                        }
                    }
                }
            }
            failures
        };

        if failing_checks.is_empty() {
            eprintln!("[autoheal] no failing checks, nothing to do");
            return Ok(());
        }

        // Filter out checks attempted in last 24 hours.
        let now = chrono::Utc::now();
        let actionable: Vec<_> = {
            let conn = self.db.lock().await;
            failing_checks
                .into_iter()
                .filter(|check| {
                    let check_id = check.path.as_deref().unwrap_or("");
                    let key = format!("autoheal_attempted:{}", check_id);
                    match journal::get_setting(&conn, &key) {
                        Ok(Some(ts)) => {
                            match chrono::DateTime::parse_from_rfc3339(&ts) {
                                Ok(attempted_at) => {
                                    let elapsed = now - attempted_at.to_utc();
                                    elapsed >= chrono::Duration::hours(24)
                                }
                                Err(_) => true,
                            }
                        }
                        _ => true,
                    }
                })
                .collect()
        };

        if actionable.is_empty() {
            eprintln!("[autoheal] all failing checks on cooldown");
            return Ok(());
        }

        eprintln!("[autoheal] {} actionable failing checks", actionable.len());

        // Build failing checks JSON for triage.
        let checks_json: Vec<serde_json::Value> = actionable.iter().map(|c| {
            serde_json::json!({
                "check_id": c.path,
                "label": c.key,
                "status": c.value_text,
                "detail": c.metadata,
                "scan_type": c.scan_type,
            })
        }).collect();
        let failing_json = serde_json::to_string_pretty(&checks_json)?;

        // Build playbook list from knowledge dir.
        let playbooks_dir = self.app_dir
            .join("knowledge")
            .join("playbooks");
        let playbook_list = list_playbook_slugs(&playbooks_dir);
        let playbooks_json = serde_json::to_string_pretty(&playbook_list)?;

        if playbook_list.is_empty() {
            eprintln!("[autoheal] no playbooks available");
            return Ok(());
        }

        // Call LLM triage.
        let triage = self.llm.triage_health_issues(&failing_json, &playbooks_json).await?;

        if !triage.should_heal || triage.playbook_slug.is_empty() {
            eprintln!("[autoheal] triage decided not to heal");
            return Ok(());
        }

        eprintln!("[autoheal] triage picked: {} for check {}", triage.playbook_slug, triage.check_id);

        if !auto_heal_on {
            // Emit notification that auto-heal is available but off.
            let _ = self.app_handle.emit("auto-heal-available", AutoHealAvailablePayload {
                check_id: triage.check_id.clone(),
                playbook_slug: triage.playbook_slug.clone(),
                reason: triage.reason.clone(),
            });
            eprintln!("[autoheal] auto-heal is OFF, emitted availability notification");
            return Ok(());
        }

        // Run the playbook.
        self.run_playbook(&triage.playbook_slug, &triage.check_id, &triage.reason).await
    }

    /// Execute a playbook for a failing check.
    async fn run_playbook(&self, slug: &str, check_id: &str, reason: &str) -> anyhow::Result<()> {
        let run_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        // Get current health score.
        let score_before = {
            let conn = self.db.lock().await;
            journal::list_health_scores(&conn, 1)?
                .first()
                .map(|s| s.score)
        };

        // Set concurrency guard.
        {
            let conn = self.db.lock().await;
            journal::set_setting(&conn, "autoheal_running", "true")?;
        }

        // Record the run.
        {
            let conn = self.db.lock().await;
            journal::insert_auto_heal_run(&conn, &journal::AutoHealRun {
                id: run_id.clone(),
                check_id: check_id.to_string(),
                playbook_slug: slug.to_string(),
                session_id: None,
                triage_reason: Some(reason.to_string()),
                started_at: now.to_rfc3339(),
                completed_at: None,
                success: false,
                score_before,
                score_after: None,
                error_message: None,
            })?;
        }

        // Emit started event.
        let _ = self.app_handle.emit("auto-heal-started", AutoHealPayload {
            check_id: check_id.to_string(),
            playbook_slug: slug.to_string(),
            reason: reason.to_string(),
        });

        // Create session and run playbook via AppState.
        let state = self.app_handle.state::<crate::AppState>();
        let session_id;
        let result: Result<String, anyhow::Error> = {
            let mut orch = state.orchestrator.lock().await;
            session_id = orch.create_session();

            let message = format!(
                "activate_playbook {}\n\nRun all steps autonomously. The health check '{}' is failing. \
                Execute the playbook without asking questions — just run each step. \
                If a step requires user approval, present it normally.",
                slug, check_id,
            );

            orch.send_message(&session_id, &message, &self.app_handle, &self.db).await
        };

        let completed_at = chrono::Utc::now().to_rfc3339();
        let success = result.is_ok();
        let error_message = result.err().map(|e: anyhow::Error| e.to_string());

        // Mark check as attempted (24h cooldown).
        {
            let conn = self.db.lock().await;
            let key = format!("autoheal_attempted:{}", check_id);
            journal::set_setting(&conn, &key, &now.to_rfc3339())?;
        }

        // Clear concurrency guard.
        {
            let conn = self.db.lock().await;
            journal::set_setting(&conn, "autoheal_running", "false")?;
        }

        // Update the run record.
        {
            let conn = self.db.lock().await;
            journal::update_auto_heal_run(
                &conn,
                &run_id,
                Some(&session_id),
                &completed_at,
                success,
                None, // score_after computed later
                error_message.as_deref(),
            )?;
        }

        // Rescan health to get real score_after.
        let score_after: Option<i32> = if success {
            let rescan_db = self.db.clone();
            let rescan_app_dir = self.app_dir.clone();
            match tokio::task::spawn_blocking(move || -> Option<i32> {
                let conn = rescan_db.blocking_lock();
                let enabled = crate::commands::health::enabled_categories_from_config(&rescan_app_dir);
                let budget = std::time::Duration::from_secs(30);

                // Re-run scanners to get fresh results.
                use crate::scanner::security::SecurityScanner;
                use crate::scanner::updates::UpdateScanner;
                use crate::scanner::backups::BackupScanner;
                use crate::scanner::performance::PerformanceScanner;
                use crate::scanner::network::NetworkScanner;
                use crate::scanner::Scanner;
                use noah_health::Category;
                use crate::commands::health::should_scan;

                if should_scan(&enabled, Category::Security) { let _ = SecurityScanner.tick(budget, &conn); }
                if should_scan(&enabled, Category::Updates) { let _ = UpdateScanner.tick(budget, &conn); }
                if should_scan(&enabled, Category::Backups) { let _ = BackupScanner.tick(budget, &conn); }
                if should_scan(&enabled, Category::Performance) { let _ = PerformanceScanner.tick(budget, &conn); }
                if should_scan(&enabled, Category::Network) { let _ = NetworkScanner.tick(budget, &conn); }

                let mut all_checks = Vec::new();
                all_checks.extend(crate::commands::health::checks_from_scan_results(&conn, "security", Category::Security));
                all_checks.extend(crate::commands::health::checks_from_scan_results(&conn, "updates", Category::Updates));
                all_checks.extend(crate::commands::health::checks_from_scan_results(&conn, "backups", Category::Backups));
                all_checks.extend(crate::commands::health::checks_from_scan_results(&conn, "performance", Category::Performance));
                all_checks.extend(crate::commands::health::checks_from_scan_results(&conn, "network", Category::Network));

                if all_checks.is_empty() { return None; }
                let score = noah_health::compute_score(all_checks, None, enabled.as_deref());
                Some(score.overall_score as i32)
            }).await {
                Ok(s) => s,
                Err(_) => None,
            }
        } else {
            None
        };

        // Emit completed event.
        let _ = self.app_handle.emit("auto-heal-completed", AutoHealCompletePayload {
            check_id: check_id.to_string(),
            playbook_slug: slug.to_string(),
            success,
            score_before,
            score_after,
            error: error_message,
        });

        // Push auto-heal event to fleet if linked.
        if let Some(config) = crate::dashboard_link::DashboardConfig::load(&self.app_dir) {
            let sb = score_before.unwrap_or(0);
            let sa = score_after.unwrap_or(sb);
            let slug_owned = slug.to_string();
            let check_id_owned = check_id.to_string();
            let push_app_dir = self.app_dir.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::dashboard_link::push_auto_heal_event(
                    &config, &check_id_owned, &slug_owned, sb, sa,
                ).await {
                    eprintln!("[autoheal] fleet push failed: {}", e);
                }
            });
        }

        eprintln!("[autoheal] playbook {} completed (success={})", slug, success);
        Ok(())
    }
}

/// List playbook slugs from the playbooks directory.
fn list_playbook_slugs(playbooks_dir: &std::path::Path) -> Vec<serde_json::Value> {
    let mut list = Vec::new();
    let Ok(entries) = std::fs::read_dir(playbooks_dir) else { return list };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let filename = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            // Read first few lines to get description
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let description: String = content.lines().take(3).collect::<Vec<_>>().join(" ");
            list.push(serde_json::json!({
                "slug": filename,
                "description": description,
            }));
        }
    }
    list
}
