use tauri::State;
use uuid::Uuid;

use crate::AppState;
use crate::safety::journal;
use crate::scanner::security::SecurityScanner;
use crate::scanner::updates::UpdateScanner;
use crate::scanner::Scanner;

use noah_health::{Category, CheckResult, CheckStatus, compute_score};

/// Build CheckResults from the latest security scan results stored in the DB.
fn checks_from_scan_results(
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

    let mut all_checks = Vec::new();
    all_checks.extend(checks_from_scan_results(&conn, "security", Category::Security));
    all_checks.extend(checks_from_scan_results(&conn, "updates", Category::Updates));

    // Don't fabricate a score when no checks have run yet.
    if all_checks.is_empty() {
        return Ok("null".to_string());
    }

    let score = compute_score(all_checks, None);

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
pub async fn run_health_check(state: State<'_, AppState>) -> Result<String, String> {
    // Run security + update scanners on a blocking thread, then read results
    // from the DB within the same function to avoid any stale-read issues.
    let db = state.db.clone();
    let result_json = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let conn = db.blocking_lock();
        let budget = std::time::Duration::from_secs(60);

        // Run scanners — these write fresh results to the DB.
        let security = SecurityScanner;
        if let Err(e) = security.tick(budget, &conn) {
            eprintln!("[health] security scan failed: {}", e);
        }

        let updates = UpdateScanner;
        if let Err(e) = updates.tick(budget, &conn) {
            eprintln!("[health] update scan failed: {}", e);
        }

        // Read back results from DB immediately (same connection, guaranteed fresh).
        let mut all_checks = Vec::new();
        all_checks.extend(checks_from_scan_results(&conn, "security", Category::Security));
        all_checks.extend(checks_from_scan_results(&conn, "updates", Category::Updates));

        if all_checks.is_empty() {
            return Ok("null".to_string());
        }

        let score = compute_score(all_checks, None);

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
        "security.sip" => {
            // SIP can't be toggled from user space — no settings pane to open.
            return Err("SIP must be changed from Recovery Mode (csrutil enable)".to_string());
        }

        // Windows
        #[cfg(target_os = "windows")]
        "security.defender" => "ms-settings:windowsdefender",
        #[cfg(target_os = "windows")]
        "security.bitlocker" => "ms-settings:about", // BitLocker is in Control Panel
        #[cfg(target_os = "windows")]
        "security.firewall" => "ms-settings:windowsdefender",
        #[cfg(target_os = "windows")]
        "security.uac" => "ms-settings:signinoptions",
        #[cfg(target_os = "windows")]
        "security.screen_lock" => "ms-settings:lockscreen",
        #[cfg(target_os = "windows")]
        "updates.os" => "ms-settings:windowsupdate",

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
            .args(["/C", "start", target])
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
