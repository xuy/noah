use tauri::State;
use uuid::Uuid;

use crate::AppState;
use crate::dashboard_link::DashboardConfig;
use crate::safety::journal;
use crate::scanner::backups::BackupScanner;
use crate::scanner::network::NetworkScanner;
use crate::scanner::performance::PerformanceScanner;
use crate::scanner::security::SecurityScanner;
use crate::scanner::updates::UpdateScanner;
use crate::scanner::Scanner;

use noah_health::{Category, CheckResult, CheckStatus, compute_score};

/// Convert stored category strings to Category enums, respecting fleet policy.
pub(crate) fn enabled_categories_from_config(app_dir: &std::path::Path) -> Option<Vec<Category>> {
    let config = DashboardConfig::load(app_dir)?;
    let cats = config.enabled_categories?;
    let parsed: Vec<Category> = cats.iter().filter_map(|s| match s.as_str() {
        "security" => Some(Category::Security),
        "updates" => Some(Category::Updates),
        "performance" => Some(Category::Performance),
        "backups" => Some(Category::Backups),
        "network" => Some(Category::Network),
        _ => None,
    }).collect();
    if parsed.is_empty() { None } else { Some(parsed) }
}

/// Check whether a category should be scanned given the enabled filter.
pub(crate) fn should_scan(enabled: &Option<Vec<Category>>, cat: Category) -> bool {
    match enabled {
        None => true,
        Some(cats) => cats.contains(&cat),
    }
}

/// Build CheckResults from the latest security scan results stored in the DB.
pub(crate) fn checks_from_scan_results(
    conn: &rusqlite::Connection,
    scan_type: &str,
    category: Category,
) -> Vec<CheckResult> {
    let results = journal::query_scan_results(conn, scan_type, None, None, None, 100);
    let Ok(results) = results else { return Vec::new() };

    results
        .iter()
        .map(|r| {
            let status = match r.value_text.as_deref() {
                Some("pass") => CheckStatus::Pass,
                Some("warn") => CheckStatus::Warn,
                _ => CheckStatus::Fail,
            };
            CheckResult {
                id: r.path.clone().unwrap_or_default(),
                category,
                label: r.key.clone().unwrap_or_default(),
                status,
                detail: r.metadata.clone().unwrap_or_default(),
            }
        })
        .collect()
}

/// Get the current health score based on latest scan results in the DB.
/// Returns null JSON if no scan results exist yet (avoids a fake F/0).
#[tauri::command]
pub async fn get_health_score(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().await;
    let enabled = enabled_categories_from_config(&state.app_dir);

    let mut all_checks = Vec::new();
    all_checks.extend(checks_from_scan_results(&conn, "security", Category::Security));
    all_checks.extend(checks_from_scan_results(&conn, "updates", Category::Updates));
    all_checks.extend(checks_from_scan_results(&conn, "backups", Category::Backups));
    all_checks.extend(checks_from_scan_results(&conn, "performance", Category::Performance));
    all_checks.extend(checks_from_scan_results(&conn, "network", Category::Network));

    // Don't fabricate a score when no checks have run yet.
    if all_checks.is_empty() {
        return Ok("null".to_string());
    }

    let score = compute_score(all_checks, None, enabled.as_deref());

    // Persist the score.
    let record = journal::HealthScoreRecord {
        id: Uuid::new_v4().to_string(),
        score: score.overall_score as i32,
        grade: score.overall_grade.to_string(),
        categories: serde_json::to_string(&score.categories).unwrap_or_default(),
        computed_at: score.computed_at.clone(),
        device_id: score.device_id.clone(),
    };
    let _ = journal::insert_health_score(&conn, &record);

    serde_json::to_string(&score).map_err(|e| e.to_string())
}

/// Run all health scanners directly, then compute and return the health score.
#[tauri::command]
pub async fn run_health_check(state: State<'_, AppState>, app_handle: tauri::AppHandle) -> Result<String, String> {
    // Run security + update scanners on a blocking thread, then read results
    // from the DB within the same function to avoid any stale-read issues.
    let db = state.db.clone();
    let health_app_dir = state.app_dir.clone();
    let result_json = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let conn = db.blocking_lock();
        let budget = std::time::Duration::from_secs(60);
        let enabled = enabled_categories_from_config(&health_app_dir);

        // Run scanners — these write fresh results to the DB.
        if should_scan(&enabled, Category::Security) {
            let security = SecurityScanner;
            if let Err(e) = security.tick(budget, &conn) {
                eprintln!("[health] security scan failed: {}", e);
            }
        }

        if should_scan(&enabled, Category::Updates) {
            let updates = UpdateScanner;
            if let Err(e) = updates.tick(budget, &conn) {
                eprintln!("[health] update scan failed: {}", e);
            }
        }

        if should_scan(&enabled, Category::Backups) {
            let backups = BackupScanner;
            if let Err(e) = backups.tick(budget, &conn) {
                eprintln!("[health] backup scan failed: {}", e);
            }
        }

        if should_scan(&enabled, Category::Performance) {
            let perf = PerformanceScanner;
            if let Err(e) = perf.tick(budget, &conn) {
                eprintln!("[health] performance scan failed: {}", e);
            }
        }

        if should_scan(&enabled, Category::Network) {
            let net = NetworkScanner;
            if let Err(e) = net.tick(budget, &conn) {
                eprintln!("[health] network scan failed: {}", e);
            }
        }

        // Read back results from DB immediately (same connection, guaranteed fresh).
        let mut all_checks = Vec::new();
        all_checks.extend(checks_from_scan_results(&conn, "security", Category::Security));
        all_checks.extend(checks_from_scan_results(&conn, "updates", Category::Updates));
        all_checks.extend(checks_from_scan_results(&conn, "backups", Category::Backups));
        all_checks.extend(checks_from_scan_results(&conn, "performance", Category::Performance));
        all_checks.extend(checks_from_scan_results(&conn, "network", Category::Network));

        if all_checks.is_empty() {
            return Ok("null".to_string());
        }

        let score = compute_score(all_checks, None, enabled.as_deref());

        // Persist the score.
        let record = journal::HealthScoreRecord {
            id: Uuid::new_v4().to_string(),
            score: score.overall_score as i32,
            grade: score.overall_grade.to_string(),
            categories: serde_json::to_string(&score.categories).unwrap_or_default(),
            computed_at: score.computed_at.clone(),
            device_id: score.device_id.clone(),
        };
        let _ = journal::insert_health_score(&conn, &record);

        serde_json::to_string(&score).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Scanner task failed: {}", e))??;

    // If linked to a fleet dashboard, push the score automatically.
    if result_json != "null" {
        let app_dir = state.app_dir.clone();
        let json_for_sync = result_json.clone();
        let handle = app_handle.clone();
        tokio::spawn(async move {
            if let Some(config) = DashboardConfig::load(&app_dir) {
                if let Ok(score) = serde_json::from_str::<serde_json::Value>(&json_for_sync) {
                    let s = score.get("overall_score").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let g = score.get("overall_grade").and_then(|v| v.as_str()).unwrap_or("F");
                    let cats = score.get("categories").map(|v| v.to_string()).unwrap_or_else(|| "[]".to_string());
                    match crate::dashboard_link::push_checkin(&config, s, g, &cats, Some(&app_dir)).await {
                        Ok(Some(new_cats)) => {
                            // Update enabled_categories from fleet policy.
                            if let Some(mut cfg) = DashboardConfig::load(&app_dir) {
                                cfg.enabled_categories = Some(new_cats);
                                let _ = cfg.save(&app_dir);
                            }
                            eprintln!("[health] fleet sync ok (policy updated)");
                        }
                        Ok(None) => {
                            eprintln!("[health] fleet sync ok");
                        }
                        Err(e) => {
                            let err_msg = e.to_string();
                            eprintln!("[health] fleet sync failed: {}", err_msg);
                            // Notify UI when device was removed from fleet
                            if err_msg.contains("Device removed from fleet") {
                                use tauri::Emitter;
                                let _ = handle.emit("fleet-disconnected", serde_json::json!({
                                    "reason": "Your device was removed from the fleet by the administrator."
                                }));
                            }
                        }
                    }
                }
            }
        });
    }

    Ok(result_json)
}

/// Open a system settings pane by check ID.
/// Uses macOS `x-apple.systempreferences:` deep links or Windows ms-settings: URIs.
#[tauri::command]
pub async fn open_health_fix(check_id: String) -> Result<(), String> {
    let target = match check_id.as_str() {
        // macOS
        #[cfg(target_os = "macos")]
        "security.firewall" => "x-apple.systempreferences:com.apple.Network-Settings.extension?Firewall",
        #[cfg(target_os = "macos")]
        "security.filevault" => "x-apple.systempreferences:com.apple.preference.security?FDE",
        #[cfg(target_os = "macos")]
        "security.screen_lock" => "x-apple.systempreferences:com.apple.Lock-Screen-Settings.extension",
        #[cfg(target_os = "macos")]
        "security.gatekeeper" => "x-apple.systempreferences:com.apple.preference.security?General",
        #[cfg(target_os = "macos")]
        "security.xprotect" => "x-apple.systempreferences:com.apple.Software-Update-Settings.extension",
        #[cfg(target_os = "macos")]
        "updates.os" => "x-apple.systempreferences:com.apple.Software-Update-Settings.extension",
        #[cfg(target_os = "macos")]
        "backups.timemachine" | "backups.timemachine_dest" => "x-apple.systempreferences:com.apple.Time-Machine-Settings.extension",
        #[cfg(target_os = "macos")]
        "security.sip" => {
            // SIP can't be toggled from user space — no settings pane to open.
            return Err("SIP must be changed from Recovery Mode (csrutil enable)".to_string());
        }

        // Windows
        #[cfg(target_os = "windows")]
        "security.defender" => "ms-settings:windowsdefender",
        #[cfg(target_os = "windows")]
        "security.bitlocker" => {
            // BitLocker lives in Control Panel, not ms-settings.
            std::process::Command::new("control")
                .args(["/name", "Microsoft.BitLockerDriveEncryption"])
                .spawn()
                .map_err(|e| format!("Failed to open BitLocker settings: {}", e))?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        "security.firewall" => "ms-settings:windowsdefender",
        #[cfg(target_os = "windows")]
        "security.uac" => {
            // UAC settings have their own executable, not an ms-settings URI.
            std::process::Command::new("UserAccountControlSettings.exe")
                .spawn()
                .map_err(|e| format!("Failed to open UAC settings: {}", e))?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        "security.screen_lock" => "ms-settings:lockscreen",
        #[cfg(target_os = "windows")]
        "updates.os" => "ms-settings:windowsupdate",
        #[cfg(target_os = "windows")]
        "backups.filehistory" => "ms-settings:backup",
        #[cfg(target_os = "windows")]
        "backups.restore_points" => "ms-settings:recovery",

        _ => return Err(format!("No settings pane for check: {}", check_id)),
    };

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open settings: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .map_err(|e| format!("Failed to open settings: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let _ = target;
        return Err("Settings deep links not available on Linux".to_string());
    }

    Ok(())
}

/// Get the last N health scores for history display.
#[tauri::command]
pub async fn get_health_history(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<String, String> {
    let conn = state.db.lock().await;
    let records = journal::list_health_scores(&conn, limit.unwrap_or(30))
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&records).map_err(|e| e.to_string())
}

/// Get pending fleet actions for this device.
#[tauri::command]
pub async fn get_fleet_actions(state: State<'_, AppState>) -> Result<String, String> {
    let config = DashboardConfig::load(&state.app_dir);
    let Some(config) = config else {
        return Ok("[]".to_string());
    };

    match crate::dashboard_link::poll_actions(&config, Some(&state.app_dir)).await {
        Ok(actions) => serde_json::to_string(&actions).map_err(|e| e.to_string()),
        Err(_) => Ok("[]".to_string()),
    }
}

/// Report a fleet action as completed or dismissed.
#[tauri::command]
pub async fn resolve_fleet_action(
    state: State<'_, AppState>,
    action_id: String,
    status: String,
) -> Result<(), String> {
    let config = DashboardConfig::load(&state.app_dir)
        .ok_or("Not connected to fleet")?;

    crate::dashboard_link::report_action_status(&config, &action_id, &status)
        .await
        .map_err(|e| e.to_string())
}

/// Run verification after a remediation: rescan health, compute new score, push to fleet.
#[tauri::command]
pub async fn verify_remediation(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    action_id: String,
) -> Result<String, String> {
    // Re-run health check (reuse the existing logic)
    let result_json = run_health_check(state.clone(), app_handle).await?;

    if result_json == "null" {
        return Err("No health data after rescan".to_string());
    }

    // Parse score and push verification to fleet
    if let Some(config) = DashboardConfig::load(&state.app_dir) {
        if let Ok(score_val) = serde_json::from_str::<serde_json::Value>(&result_json) {
            let score_after = score_val.get("overall_score").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            if let Err(e) = crate::dashboard_link::push_verification(&config, &action_id, score_after).await {
                eprintln!("[health] verification push failed: {}", e);
            }
        }
    }

    Ok(result_json)
}

/// Start a fleet playbook by creating a new session.
/// Returns the session ID and playbook slug so the frontend can navigate and send the activate message.
#[tauri::command]
pub async fn start_fleet_playbook(
    state: State<'_, AppState>,
    action_id: String,
    playbook_slug: String,
) -> Result<String, String> {
    // Create a new session
    let session_id = {
        let mut orch = state.orchestrator.lock().await;
        orch.create_session()
    };

    // Persist the session to the DB
    let conn = state.db.lock().await;
    let now = chrono::Utc::now().to_rfc3339();
    let _ = journal::create_session_record(&conn, &session_id, &now);
    drop(conn);

    // Mark the fleet action as completed (the playbook will handle the rest)
    if let Some(config) = DashboardConfig::load(&state.app_dir) {
        let _ = crate::dashboard_link::report_action_status(&config, &action_id, "completed").await;
    }

    Ok(serde_json::json!({
        "session_id": session_id,
        "playbook_slug": playbook_slug,
    }).to_string())
}

/// Generate a plain-text health report for compliance/audit purposes.
#[tauri::command]
pub async fn export_health_report(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().await;
    let enabled = enabled_categories_from_config(&state.app_dir);

    let mut all_checks = Vec::new();
    all_checks.extend(checks_from_scan_results(&conn, "security", Category::Security));
    all_checks.extend(checks_from_scan_results(&conn, "updates", Category::Updates));
    all_checks.extend(checks_from_scan_results(&conn, "backups", Category::Backups));
    all_checks.extend(checks_from_scan_results(&conn, "performance", Category::Performance));
    all_checks.extend(checks_from_scan_results(&conn, "network", Category::Network));

    if all_checks.is_empty() {
        return Err("No health data available. Run a health check first.".to_string());
    }

    let score = compute_score(all_checks, None, enabled.as_deref());

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "Unknown".to_string());

    let os_name = if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    };

    let mut report = String::new();
    report.push_str("╔══════════════════════════════════════════╗\n");
    report.push_str("║         NOAH HEALTH REPORT               ║\n");
    report.push_str("╚══════════════════════════════════════════╝\n\n");
    report.push_str(&format!("Device:     {}\n", hostname));
    report.push_str(&format!("OS:         {}\n", os_name));
    report.push_str(&format!("Generated:  {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")));
    report.push_str(&format!("\nOverall Score: {} ({})\n", score.overall_score, score.overall_grade));
    report.push_str(&format!("{}\n", "─".repeat(44)));

    for cat in &score.categories {
        report.push_str(&format!("\n▸ {} — {} ({})\n",
            cat.category.label(), cat.score, cat.grade));
        for check in &cat.checks {
            let icon = match check.status {
                noah_health::CheckStatus::Pass => "✓",
                noah_health::CheckStatus::Warn => "⚠",
                noah_health::CheckStatus::Fail => "✗",
            };
            report.push_str(&format!("  {} {} — {}\n", icon, check.label, check.detail));
        }
    }

    report.push_str(&format!("\n{}\n", "─".repeat(44)));
    report.push_str("Generated by Noah (https://onnoah.app)\n");
    report.push_str("Health data collected locally on this device.\n");

    Ok(report)
}
