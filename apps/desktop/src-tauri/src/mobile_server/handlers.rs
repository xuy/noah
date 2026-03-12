use std::sync::Arc;

use axum::extract::{Path, State as AxumState};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::events::StderrEventEmitter;
use crate::safety::journal;

use super::{MobileServerState, PairedDevice, SseEvent};

// ── POST /pair ──

#[derive(Deserialize)]
pub struct PairRequest {
    token: String,
    device_name: String,
}

#[derive(Serialize)]
pub struct PairResponse {
    device_id: String,
    secret: String,
    desktop_name: String,
}

pub async fn pair(
    AxumState(state): AxumState<Arc<MobileServerState>>,
    Json(body): Json<PairRequest>,
) -> impl IntoResponse {
    let mut pairing = state.pairing.lock().await;

    // Check lockout
    if pairing.failed_attempts >= 5 {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many failed attempts. Regenerate QR code."})),
        );
    }

    // Validate token
    let valid = match &pairing.pending_token {
        Some(pt) => {
            pt.token == body.token
                && pt.created_at.elapsed() < std::time::Duration::from_secs(300)
        }
        None => false,
    };

    if !valid {
        pairing.failed_attempts += 1;
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid or expired token"})),
        );
    }

    // Generate secret and store pairing
    use rand::Rng;
    let secret: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();

    let device_id = uuid::Uuid::new_v4().to_string();
    let paired = PairedDevice {
        device_id: device_id.clone(),
        secret: secret.clone(),
        device_name: body.device_name.clone(),
        paired_at: chrono::Utc::now().to_rfc3339(),
    };

    pairing.paired_device = Some(paired.clone());
    pairing.pending_token = None;

    // Persist to DB
    {
        let conn = state.db.lock().await;
        let _ = journal::set_setting(&conn, "paired_device", &serde_json::to_string(&paired).unwrap_or_default());
    }

    eprintln!(
        "[mobile-server] Paired with device: {} ({})",
        body.device_name, device_id
    );

    (
        StatusCode::OK,
        Json(json!(PairResponse {
            device_id,
            secret,
            desktop_name: "Noah Desktop".to_string(),
        })),
    )
}

// ── GET /status ──

pub async fn status(
    AxumState(state): AxumState<Arc<MobileServerState>>,
) -> impl IntoResponse {
    let pairing = state.pairing.lock().await;
    let paired = pairing.paired_device.is_some();
    let device_name = pairing
        .paired_device
        .as_ref()
        .map(|d| d.device_name.clone())
        .unwrap_or_default();
    drop(pairing);

    let pending_count = {
        let approvals = state.pending_approvals.lock().await;
        approvals.len()
    };

    Json(json!({
        "online": true,
        "paired": paired,
        "device_name": device_name,
        "pending_approvals": pending_count,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ── POST /triage ──

#[derive(Deserialize)]
pub struct TriageRequest {
    /// Analysis text from mobile's Claude Vision
    analysis: String,
    /// Optional user caption/question
    caption: Option<String>,
}

#[derive(Serialize)]
pub struct TriageResponse {
    session_id: String,
    status: String,
}

pub async fn triage(
    AxumState(state): AxumState<Arc<MobileServerState>>,
    Json(body): Json<TriageRequest>,
) -> impl IntoResponse {
    // Create a session
    let session_id = {
        let mut orch = state.orchestrator.lock().await;
        orch.create_session()
    };

    // Persist session record
    let created_at = chrono::Utc::now().to_rfc3339();
    {
        let conn = state.db.lock().await;
        let _ = journal::create_session_record(&conn, &session_id, &created_at);
        let _ = journal::update_session_title(
            &conn,
            &session_id,
            &format!("Mobile Triage: {}", body.caption.as_deref().unwrap_or("Photo Analysis")),
        );
    }

    // Build the message the agent will see
    let message = format!(
        "[Mobile Photo Triage]\n\n\
         The user photographed an issue on their device and the mobile app analyzed it:\n\n\
         ## Mobile Analysis\n{}\n\n\
         {}\
         Please investigate this issue on the user's computer. Run diagnostic tools to confirm \
         the problem and attempt to fix it if possible.",
        body.analysis,
        body.caption
            .as_ref()
            .map(|c| format!("## User's Question\n{}\n\n", c))
            .unwrap_or_default(),
    );

    // Persist user message
    {
        let conn = state.db.lock().await;
        let _ = journal::save_message(&conn, &session_id, "user", &message);
    }

    // Run the agent turn in background
    let sid = session_id.clone();
    let db = state.db.clone();
    let orch = state.orchestrator.clone();
    let sse_tx = state.sse_tx.clone();
    let app_handle = state.app_handle.clone();

    tokio::spawn(async move {
        let emitter = if let Some(handle) = &app_handle {
            Box::new(crate::tauri_events::TauriEventEmitter::new(handle.clone()))
                as Box<dyn crate::events::EventEmitter>
        } else {
            Box::new(StderrEventEmitter) as Box<dyn crate::events::EventEmitter>
        };

        let result = {
            let mut orchestrator = orch.lock().await;
            orchestrator
                .send_message(&sid, &message, emitter.as_ref(), &db)
                .await
        };

        match result {
            Ok(response) => {
                // Persist response
                {
                    let conn = db.lock().await;
                    let _ = journal::save_message(&conn, &sid, "assistant", &response);
                }
                let _ = sse_tx.send(SseEvent {
                    event: "session_complete".to_string(),
                    data: json!({
                        "session_id": sid,
                        "response_preview": response.chars().take(200).collect::<String>(),
                    }),
                });
                eprintln!("[mobile-server] Triage session {} complete", sid);
            }
            Err(e) => {
                let _ = sse_tx.send(SseEvent {
                    event: "session_error".to_string(),
                    data: json!({
                        "session_id": sid,
                        "error": format!("{}", e),
                    }),
                });
                eprintln!("[mobile-server] Triage session {} error: {}", sid, e);
            }
        }
    });

    // Notify desktop frontend about the new session
    if let Some(handle) = &state.app_handle {
        let _ = tauri::Emitter::emit(
            handle,
            "mobile-triage",
            json!({ "session_id": session_id }),
        );
    }

    (
        StatusCode::ACCEPTED,
        Json(json!(TriageResponse {
            session_id,
            status: "processing".to_string(),
        })),
    )
}

// ── POST /approve/:id ──

#[derive(Deserialize)]
pub struct ApproveRequest {
    approve: bool,
}

pub async fn approve(
    AxumState(state): AxumState<Arc<MobileServerState>>,
    Path(id): Path<String>,
    Json(body): Json<ApproveRequest>,
) -> impl IntoResponse {
    let mut pending = state.pending_approvals.lock().await;
    if let Some(sender) = pending.remove(&id) {
        let _ = sender.send(body.approve);
        (StatusCode::OK, Json(json!({"id": id, "executed": true})))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"id": id, "error": "Approval not found"})),
        )
    }
}

// ── GET /sessions ──

pub async fn list_sessions(
    AxumState(state): AxumState<Arc<MobileServerState>>,
) -> impl IntoResponse {
    let conn = state.db.lock().await;
    match journal::list_sessions(&conn) {
        Ok(sessions) => (StatusCode::OK, Json(json!(sessions))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{}", e)})),
        ),
    }
}

// ── GET /sessions/:id/messages ──

pub async fn session_messages(
    AxumState(state): AxumState<Arc<MobileServerState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = state.db.lock().await;
    match journal::get_messages(&conn, &id) {
        Ok(messages) => (StatusCode::OK, Json(json!(messages))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{}", e)})),
        ),
    }
}
