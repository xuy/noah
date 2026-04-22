use std::path::Path;
use tauri::State;

use crate::agent::llm_client::{AuthMode, ProxyAuth};
use crate::consumer::{client, device, entitlement, session};
use crate::AppState;

fn current_auth(app_dir: &Path) -> (Option<String>, Option<String>) {
    let sess = session::get_session_token(app_dir).ok().flatten();
    let dev = device::ensure_device_id(app_dir).ok();
    (sess, dev)
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
pub async fn consumer_has_session(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(session::has_session(&state.app_dir))
}

#[tauri::command]
pub async fn consumer_ensure_device_id(state: State<'_, AppState>) -> Result<String, String> {
    device::ensure_device_id(&state.app_dir)
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
    session::set_session_token(&state.app_dir, &token)?;
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
    session::set_session_token(&state.app_dir, &token)?;
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
    session::delete_session_token(&state.app_dir)?;
    entitlement::clear_cache(&state.app_dir);
    let mut orch = state.orchestrator.lock().await;
    if let Ok(device_id) = device::ensure_device_id(&state.app_dir) {
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
    let (session_tok, device_id) = current_auth(&state.app_dir);
    let Some(auth) = auth_ref(&session_tok, &device_id) else {
        return Ok(None);
    };
    match client::fetch_entitlement(&auth).await {
        Ok(ent) => {
            let _ = entitlement::save_cached(&state.app_dir, &ent);
            Ok(Some(ent))
        }
        Err(err) => {
            if err.to_string().contains("401") && session_tok.is_some() {
                let _ = session::delete_session_token(&state.app_dir);
                entitlement::clear_cache(&state.app_dir);
                let mut orch = state.orchestrator.lock().await;
                if let Ok(did) = device::ensure_device_id(&state.app_dir) {
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
    let (session_tok, device_id) = current_auth(&state.app_dir);
    let Some(auth) = auth_ref(&session_tok, &device_id) else {
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
    let (session_tok, device_id) = current_auth(&state.app_dir);
    let Some(auth) = auth_ref(&session_tok, &device_id) else {
        return Ok(None);
    };
    let result = client::notify_fix_completed(&auth)
        .await
        .map_err(|e| e.to_string())?;
    let _ = entitlement::save_cached(&state.app_dir, &result.entitlement);
    Ok(Some(result))
}

#[tauri::command]
pub async fn consumer_billing_checkout_url(
    state: State<'_, AppState>,
    plan: String,
) -> Result<String, String> {
    let (session_tok, device_id) = current_auth(&state.app_dir);
    let Some(auth) = auth_ref(&session_tok, &device_id) else {
        return Err("no auth available".to_string());
    };
    client::billing_checkout_url(&auth, &plan)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn consumer_confirm_checkout(
    state: State<'_, AppState>,
    checkout_session_id: String,
) -> Result<Option<client::Entitlement>, String> {
    let trimmed = checkout_session_id.trim();
    if trimmed.is_empty() {
        return Err("missing checkout_session_id".to_string());
    }

    if trimmed.starts_with("mock-") {
        let (session_tok, device_id) = current_auth(&state.app_dir);
        let Some(auth) = auth_ref(&session_tok, &device_id) else {
            return Ok(None);
        };
        let ent = client::fetch_entitlement(&auth)
            .await
            .map_err(|e| e.to_string())?;
        let _ = entitlement::save_cached(&state.app_dir, &ent);
        return Ok(Some(ent));
    }

    let result = client::confirm_checkout(trimmed)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(token) = result.session_token {
        session::set_session_token(&state.app_dir, &token)?;
        let mut orch = state.orchestrator.lock().await;
        orch.set_auth(AuthMode::Proxy {
            base_url: client::base_url(),
            auth: ProxyAuth::Session(token),
        });
    }
    if let Some(ent) = &result.entitlement {
        let _ = entitlement::save_cached(&state.app_dir, ent);
    }
    Ok(result.entitlement)
}

#[tauri::command]
pub async fn consumer_billing_portal_url(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let token = session::get_session_token(&state.app_dir)?
        .ok_or_else(|| "not signed in".to_string())?;
    client::billing_portal_url(&token)
        .await
        .map_err(|e| e.to_string())
}
