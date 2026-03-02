use std::sync::Arc;

use tauri::State;
use tokio::sync::Mutex;

use crate::safety::journal;
use crate::AppState;

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    session_id: String,
    message: String,
) -> Result<String, String> {
    // Persist the user message for session history replay.
    {
        let conn = state.db.lock().await;
        let _ = journal::save_message(&conn, &session_id, "user", &message);
    }

    // Check if this session needs a title (first message). If so, spawn a
    // background Haiku call to generate one while the main agent loop runs.
    let needs_title = {
        let conn = state.db.lock().await;
        journal::session_needs_title(&conn, &session_id).unwrap_or(false)
    };

    let title_handle = if needs_title {
        let llm = {
            let orch = state.orchestrator.lock().await;
            orch.llm_clone()
        };
        let msg = message.clone();
        let sid = session_id.clone();
        let db: Arc<Mutex<rusqlite::Connection>> = Arc::clone(&state.db);
        Some(tokio::spawn(async move {
            if let Ok(title) = llm.generate_title(&msg).await {
                let conn = db.lock().await;
                let _ = journal::update_session_title(&conn, &sid, &title);
            }
        }))
    } else {
        None
    };

    let mut orchestrator = state.orchestrator.lock().await;

    let result = orchestrator
        .send_message(&session_id, &message, &app_handle, &state.db)
        .await
        .map_err(|e| format!("Agent error: {}", e))?;

    // Persist the assistant response and update message count.
    {
        let conn = state.db.lock().await;
        let _ = journal::save_message(&conn, &session_id, "assistant", &result);
    }
    if let Some(session) = orchestrator.get_session(&session_id) {
        let count = session.messages.len() as i32;
        let conn = state.db.lock().await;
        let _ = journal::update_session_message_count(&conn, &session_id, count);
    }

    // Wait for the title generation to finish (best-effort, don't fail the whole call).
    if let Some(handle) = title_handle {
        let _ = handle.await;
    }

    Ok(result)
}

#[tauri::command]
pub async fn approve_action(
    state: State<'_, AppState>,
    approval_id: String,
) -> Result<bool, String> {
    let mut pending = state.pending_approvals.lock().await;
    if let Some(sender) = pending.remove(&approval_id) {
        let _ = sender.send(true);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn deny_action(
    state: State<'_, AppState>,
    approval_id: String,
) -> Result<bool, String> {
    let mut pending = state.pending_approvals.lock().await;
    if let Some(sender) = pending.remove(&approval_id) {
        let _ = sender.send(false);
        Ok(true)
    } else {
        Ok(false)
    }
}
