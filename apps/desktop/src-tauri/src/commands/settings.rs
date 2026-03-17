use serde::{Deserialize, Serialize};
use tauri::State;

use crate::agent::llm_client::AuthMode;
use crate::safety::journal;
use crate::AppState;

#[tauri::command]
pub async fn has_api_key(state: State<'_, AppState>) -> Result<bool, String> {
    let orch = state.orchestrator.lock().await;
    Ok(orch.has_api_key())
}

#[tauri::command]
pub async fn set_api_key(state: State<'_, AppState>, api_key: String) -> Result<(), String> {
    // Save to disk so it persists across restarts.
    crate::save_api_key(&state.app_dir, &api_key)?;

    // Update the in-memory LLM client.
    let mut orch = state.orchestrator.lock().await;
    orch.set_api_key(api_key);

    Ok(())
}

#[derive(Debug, Deserialize)]
struct RedeemResponse {
    token: Option<String>,
    error: Option<String>,
}

#[tauri::command]
pub async fn redeem_invite_code(
    state: State<'_, AppState>,
    proxy_url: String,
    invite_code: String,
) -> Result<(), String> {
    // POST to the proxy's /auth/redeem endpoint
    let client = reqwest::Client::new();
    let url = format!("{}/auth/redeem", proxy_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "invite_code": invite_code }))
        .send()
        .await
        .map_err(|e| format!("Failed to reach the Noah server: {}", e))?;

    if !resp.status().is_success() {
        let body: RedeemResponse = resp.json().await.unwrap_or(RedeemResponse {
            token: None,
            error: Some("Unknown error".to_string()),
        });
        return Err(body.error.unwrap_or_else(|| "Invalid invite code".to_string()));
    }

    let body: RedeemResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid response from server: {}", e))?;

    let token = body
        .token
        .ok_or_else(|| "No token in server response".to_string())?;

    // Save proxy config to disk
    crate::save_proxy_config(&state.app_dir, &proxy_url, &token)?;

    // Update the in-memory LLM client
    let mut orch = state.orchestrator.lock().await;
    orch.set_auth(AuthMode::Proxy {
        base_url: proxy_url,
        token,
    });

    Ok(())
}

#[tauri::command]
pub async fn get_auth_mode(state: State<'_, AppState>) -> Result<String, String> {
    let orch = state.orchestrator.lock().await;
    Ok(orch.auth_mode_name().to_string())
}

#[tauri::command]
pub async fn clear_auth(state: State<'_, AppState>) -> Result<(), String> {
    crate::clear_auth_files(&state.app_dir);
    let mut orch = state.orchestrator.lock().await;
    orch.set_auth(AuthMode::ApiKey(String::new()));
    Ok(())
}

#[tauri::command]
pub async fn get_app_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
pub async fn get_telemetry_consent(state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state.db.lock().await;
    let value = journal::get_setting(&conn, "telemetry_consent")
        .map_err(|e| format!("Failed to get setting: {}", e))?;
    Ok(value.as_deref() == Some("true"))
}

#[tauri::command]
pub async fn set_telemetry_consent(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    journal::set_setting(&conn, "telemetry_consent", if enabled { "true" } else { "false" })
        .map_err(|e| format!("Failed to save setting: {}", e))
}

#[tauri::command]
pub async fn track_event(
    state: State<'_, AppState>,
    event_type: String,
    data: String,
) -> Result<(), String> {
    // Only record if telemetry is opted-in
    let conn = state.db.lock().await;
    let consent = journal::get_setting(&conn, "telemetry_consent")
        .map_err(|e| format!("{}", e))?;
    if consent.as_deref() != Some("true") {
        return Ok(());
    }
    journal::record_telemetry_event(&conn, &event_type, &data)
        .map_err(|e| format!("Failed to track event: {}", e))
}

#[tauri::command]
pub async fn get_proactive_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state.db.lock().await;
    let value = journal::get_setting(&conn, "proactive_enabled")
        .map_err(|e| format!("Failed to get setting: {}", e))?;
    // Default is enabled (None or "true").
    Ok(value.as_deref() != Some("false"))
}

#[tauri::command]
pub async fn set_proactive_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    journal::set_setting(&conn, "proactive_enabled", if enabled { "true" } else { "false" })
        .map_err(|e| format!("Failed to save setting: {}", e))
}

#[tauri::command]
pub async fn get_auto_heal_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state.db.lock().await;
    let value = journal::get_setting(&conn, "auto_heal_enabled")
        .map_err(|e| format!("Failed to get setting: {}", e))?;
    // Default is disabled (None or "false" → false, "true" → true).
    Ok(value.as_deref() == Some("true"))
}

#[tauri::command]
pub async fn set_auto_heal_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    journal::set_setting(&conn, "auto_heal_enabled", if enabled { "true" } else { "false" })
        .map_err(|e| format!("Failed to save setting: {}", e))
}

#[tauri::command]
pub async fn dismiss_proactive_suggestion(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    journal::dismiss_proactive_suggestion(&conn, &id)
        .map_err(|e| format!("Failed to dismiss suggestion: {}", e))
}

#[tauri::command]
pub async fn act_on_proactive_suggestion(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    journal::mark_suggestion_acted_on(&conn, &id)
        .map_err(|e| format!("Failed to mark suggestion: {}", e))
}

#[tauri::command]
pub async fn set_locale(state: State<'_, AppState>, session_id: String, locale: String) -> Result<(), String> {
    let mut orch = state.orchestrator.lock().await;
    orch.set_locale(&session_id, &locale);
    Ok(())
}

#[tauri::command]
pub async fn set_session_mode(state: State<'_, AppState>, session_id: String, mode: String) -> Result<(), String> {
    match mode.as_str() {
        "default" | "learn" => {}
        _ => return Err(format!("Invalid session mode: {}", mode)),
    }
    let mut orch = state.orchestrator.lock().await;
    orch.set_mode(&session_id, &mode);
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct FeedbackContext {
    pub version: String,
    pub os: String,
    pub traces: Vec<TraceSummary>,
}

#[derive(Debug, Serialize)]
pub struct TraceSummary {
    pub timestamp: String,
    pub request: String,
    pub response: String,
}

#[tauri::command]
pub async fn get_feedback_context(state: State<'_, AppState>) -> Result<FeedbackContext, String> {
    let conn = state.db.lock().await;
    let traces = journal::get_recent_traces(&conn, 5)
        .map_err(|e| format!("Failed to get traces: {}", e))?;

    let trace_summaries: Vec<TraceSummary> = traces
        .into_iter()
        .map(|(ts, req, resp)| TraceSummary {
            timestamp: ts,
            request: req,
            response: resp,
        })
        .collect();

    Ok(FeedbackContext {
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        traces: trace_summaries,
    })
}

/// Link this device to a web dashboard using a 6-char code.
#[tauri::command]
pub async fn link_dashboard(
    state: State<'_, AppState>,
    enrollment_url: String,
) -> Result<String, String> {
    use crate::dashboard_link::{self, DashboardConfig};

    let (base_url, token) = dashboard_link::parse_enrollment_url(&enrollment_url)
        .map_err(|e| e.to_string())?;

    let (device_id, device_token, fleet_name) = dashboard_link::enroll_device(&base_url, &token)
        .await
        .map_err(|e| e.to_string())?;

    let config = DashboardConfig {
        dashboard_url: base_url,
        device_token,
        device_id: device_id.clone(),
        fleet_name,
        linked_at: chrono::Utc::now().to_rfc3339(),
    };
    config.save(&state.app_dir).map_err(|e| e.to_string())?;

    Ok(device_id)
}

/// Unlink this device from the web dashboard.
#[tauri::command]
pub async fn unlink_dashboard(state: State<'_, AppState>) -> Result<(), String> {
    use crate::dashboard_link::DashboardConfig;
    DashboardConfig::remove(&state.app_dir);
    Ok(())
}

/// Get current dashboard link status.
#[tauri::command]
pub async fn get_dashboard_status(state: State<'_, AppState>) -> Result<String, String> {
    use crate::dashboard_link::DashboardConfig;
    match DashboardConfig::load(&state.app_dir) {
        Some(config) => {
            let status = serde_json::json!({
                "linked": true,
                "url": config.dashboard_url,
                "device_id": config.device_id,
                "fleet_name": config.fleet_name,
                "linked_at": config.linked_at,
            });
            serde_json::to_string(&status).map_err(|e| e.to_string())
        }
        None => Ok(r#"{"linked":false}"#.to_string()),
    }
}
