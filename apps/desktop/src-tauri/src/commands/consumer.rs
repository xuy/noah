use tauri::State;

use crate::agent::llm_client::AuthMode;
use crate::consumer::{client, entitlement, session};
use crate::AppState;

#[tauri::command]
pub async fn consumer_has_session() -> Result<bool, String> {
    Ok(session::has_session())
}

#[tauri::command]
pub async fn consumer_request_magic_link(email: String) -> Result<(), String> {
    client::request_magic_link(email.trim())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn consumer_complete_sign_in(
    state: State<'_, AppState>,
    token: String,
) -> Result<client::Entitlement, String> {
    // Verify token by fetching entitlement with it.
    let ent = client::fetch_entitlement(&token)
        .await
        .map_err(|e| e.to_string())?;
    session::set_session_token(&token)?;
    entitlement::save_cached(&state.app_dir, &ent)?;
    // Switch the in-memory LLM client to consumer-proxy mode.
    let mut orch = state.orchestrator.lock().await;
    orch.set_auth(AuthMode::Proxy {
        base_url: client::base_url(),
        token,
    });
    Ok(ent)
}

#[tauri::command]
pub async fn consumer_sign_out(state: State<'_, AppState>) -> Result<(), String> {
    session::delete_session_token()?;
    entitlement::clear_cache(&state.app_dir);
    let mut orch = state.orchestrator.lock().await;
    orch.set_auth(AuthMode::ApiKey(String::new()));
    Ok(())
}

#[tauri::command]
pub async fn consumer_get_entitlement(
    state: State<'_, AppState>,
) -> Result<Option<client::Entitlement>, String> {
    let token = match session::get_session_token()? {
        Some(t) => t,
        None => return Ok(None),
    };
    match client::fetch_entitlement(&token).await {
        Ok(ent) => {
            let _ = entitlement::save_cached(&state.app_dir, &ent);
            Ok(Some(ent))
        }
        Err(err) => {
            // 401 means the server definitively rejected the session
            // (missing/invalid/expired/revoked). Clear state so the next
            // app-launch gate routes the user back to SignInScreen.
            if err.to_string().contains("401") {
                let _ = session::delete_session_token();
                entitlement::clear_cache(&state.app_dir);
                let mut orch = state.orchestrator.lock().await;
                orch.set_auth(AuthMode::ApiKey(String::new()));
                return Ok(None);
            }
            // Any other error (network, 5xx) → fall back to cached entitlement
            // within the 72h offline grace window, so a flaky wifi doesn't
            // boot the user back to sign-in.
            if let Some(cached) = entitlement::load_cached(&state.app_dir) {
                if entitlement::is_within_offline_grace(&cached) {
                    return Ok(Some(cached.entitlement));
                }
            }
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn consumer_notify_issue_started(
    state: State<'_, AppState>,
) -> Result<Option<client::Entitlement>, String> {
    let token = match session::get_session_token()? {
        Some(t) => t,
        None => return Ok(None),
    };
    let ent = client::notify_issue_started(&token)
        .await
        .map_err(|e| e.to_string())?;
    let _ = entitlement::save_cached(&state.app_dir, &ent);
    Ok(Some(ent))
}

#[tauri::command]
pub async fn consumer_notify_fix_completed(
    state: State<'_, AppState>,
) -> Result<Option<client::FixCompletedResponse>, String> {
    let token = match session::get_session_token()? {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = client::notify_fix_completed(&token)
        .await
        .map_err(|e| e.to_string())?;
    let _ = entitlement::save_cached(&state.app_dir, &result.entitlement);
    Ok(Some(result))
}

#[tauri::command]
pub async fn consumer_billing_checkout_url(plan: String) -> Result<String, String> {
    let token = session::get_session_token()?
        .ok_or_else(|| "not signed in".to_string())?;
    client::billing_checkout_url(&token, &plan)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn consumer_billing_portal_url() -> Result<String, String> {
    let token = session::get_session_token()?
        .ok_or_else(|| "not signed in".to_string())?;
    client::billing_portal_url(&token)
        .await
        .map_err(|e| e.to_string())
}
