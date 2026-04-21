use tauri::State;

use crate::agent::llm_client::{AuthMode, ProxyAuth};
use crate::consumer::{client, device, entitlement, session};
use crate::AppState;

/// Build the currently-active auth — session token if signed in,
/// otherwise the anonymous device id. Returns owned strings because
/// callers tend to `.await` across the borrow boundary.
fn current_auth() -> (Option<String>, Option<String>) {
    let session = session::get_session_token().ok().flatten();
    let device = device::ensure_device_id().ok();
    (session, device)
}

fn auth_ref<'a>(
    session: &'a Option<String>,
    device: &'a Option<String>,
) -> Option<client::Auth<'a>> {
    if let Some(t) = session.as_deref() {
        return Some(client::Auth::Session(t));
    }
    if let Some(d) = device.as_deref() {
        return Some(client::Auth::Device(d));
    }
    None
}

#[tauri::command]
pub async fn consumer_has_session() -> Result<bool, String> {
    Ok(session::has_session())
}

#[tauri::command]
pub async fn consumer_ensure_device_id() -> Result<String, String> {
    device::ensure_device_id()
}

#[tauri::command]
pub async fn consumer_request_magic_link(
    state: State<'_, AppState>,
    email: String,
) -> Result<Option<client::Entitlement>, String> {
    let resp = client::request_magic_link(email.trim())
        .await
        .map_err(|e| e.to_string())?;
    let Some(token) = resp.session_token else {
        return Ok(None);
    };
    let ent = client::fetch_entitlement(&client::Auth::Session(&token))
        .await
        .map_err(|e| e.to_string())?;
    session::set_session_token(&token)?;
    entitlement::save_cached(&state.app_dir, &ent)?;
    let mut orch = state.orchestrator.lock().await;
    orch.set_auth(AuthMode::Proxy {
        base_url: client::base_url(),
        auth: ProxyAuth::Session(token),
    });
    Ok(Some(ent))
}

#[tauri::command]
pub async fn consumer_complete_sign_in(
    state: State<'_, AppState>,
    token: String,
) -> Result<client::Entitlement, String> {
    let ent = client::fetch_entitlement(&client::Auth::Session(&token))
        .await
        .map_err(|e| e.to_string())?;
    session::set_session_token(&token)?;
    entitlement::save_cached(&state.app_dir, &ent)?;
    let mut orch = state.orchestrator.lock().await;
    orch.set_auth(AuthMode::Proxy {
        base_url: client::base_url(),
        auth: ProxyAuth::Session(token),
    });
    Ok(ent)
}

#[tauri::command]
pub async fn consumer_sign_out(state: State<'_, AppState>) -> Result<(), String> {
    session::delete_session_token()?;
    entitlement::clear_cache(&state.app_dir);
    // On sign-out, fall back to anonymous device auth so the app keeps
    // working (user just sees the device's trial state).
    let mut orch = state.orchestrator.lock().await;
    if let Ok(device_id) = device::ensure_device_id() {
        orch.set_auth(AuthMode::Proxy {
            base_url: client::base_url(),
            auth: ProxyAuth::Device(device_id),
        });
    } else {
        orch.set_auth(AuthMode::ApiKey(String::new()));
    }
    Ok(())
}

#[tauri::command]
pub async fn consumer_get_entitlement(
    state: State<'_, AppState>,
) -> Result<Option<client::Entitlement>, String> {
    let (session, device_id) = current_auth();
    let Some(auth) = auth_ref(&session, &device_id) else {
        return Ok(None);
    };
    match client::fetch_entitlement(&auth).await {
        Ok(ent) => {
            let _ = entitlement::save_cached(&state.app_dir, &ent);
            Ok(Some(ent))
        }
        Err(err) => {
            // 401 here is only meaningful for Session auth — it means the
            // server revoked the session. For Device auth the server
            // returns 401 only if the header is missing/malformed, which
            // shouldn't happen. Treat 401 as "drop session, stay signed
            // out" but don't wipe the device id.
            if err.to_string().contains("401") && session.is_some() {
                let _ = session::delete_session_token();
                entitlement::clear_cache(&state.app_dir);
                let mut orch = state.orchestrator.lock().await;
                if let Ok(did) = device::ensure_device_id() {
                    orch.set_auth(AuthMode::Proxy {
                        base_url: client::base_url(),
                        auth: ProxyAuth::Device(did),
                    });
                }
                return Ok(None);
            }
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
    let (session, device_id) = current_auth();
    let Some(auth) = auth_ref(&session, &device_id) else {
        return Ok(None);
    };
    let ent = client::notify_issue_started(&auth)
        .await
        .map_err(|e| e.to_string())?;
    let _ = entitlement::save_cached(&state.app_dir, &ent);
    Ok(Some(ent))
}

#[tauri::command]
pub async fn consumer_notify_fix_completed(
    state: State<'_, AppState>,
) -> Result<Option<client::FixCompletedResponse>, String> {
    let (session, device_id) = current_auth();
    let Some(auth) = auth_ref(&session, &device_id) else {
        return Ok(None);
    };
    let result = client::notify_fix_completed(&auth)
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
