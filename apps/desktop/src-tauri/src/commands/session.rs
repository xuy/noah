use serde::{Deserialize, Serialize};
use tauri::State;

use crate::dashboard_link::{self, DashboardConfig};
use crate::safety::journal::{self, MessageRecord, SessionRecord};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub created_at: String,
    pub message_count: usize,
}

#[tauri::command]
pub async fn create_session(state: State<'_, AppState>) -> Result<SessionInfo, String> {
    let mut orchestrator = state.orchestrator.lock().await;
    let id = orchestrator.create_session();

    let session = orchestrator
        .get_session(&id)
        .ok_or_else(|| "Failed to retrieve newly created session".to_string())?;

    let created_at = session.created_at.to_rfc3339();

    // Persist the session record to the database.
    {
        let conn = state.db.lock().await;
        if let Err(e) = journal::create_session_record(&conn, &id, &created_at) {
            eprintln!("[warn] Failed to persist session record: {}", e);
        }
    }

    Ok(SessionInfo {
        id: session.id.clone(),
        created_at,
        message_count: session.messages.len(),
    })
}

#[tauri::command]
pub async fn get_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionInfo, String> {
    let orchestrator = state.orchestrator.lock().await;

    let session = orchestrator
        .get_session(&session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    Ok(SessionInfo {
        id: session.id.clone(),
        created_at: session.created_at.to_rfc3339(),
        message_count: session.messages.len(),
    })
}

#[tauri::command]
pub async fn end_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<bool, String> {
    let mut orchestrator = state.orchestrator.lock().await;

    // Capture message count before ending (which removes the session from memory).
    let message_count = orchestrator
        .get_session(&session_id)
        .map(|s| s.messages.len() as i32)
        .unwrap_or(0);

    let removed = orchestrator.end_session(&session_id);

    if removed {
        let ended_at = chrono::Utc::now().to_rfc3339();
        let conn = state.db.lock().await;
        if let Err(e) = journal::end_session_record(&conn, &session_id, &ended_at, message_count) {
            eprintln!("[warn] Failed to persist session end: {}", e);
        }

        // Push session report to fleet if linked
        let session_record = journal::list_sessions(&conn)
            .ok()
            .and_then(|sessions| sessions.into_iter().find(|s| s.id == session_id));

        if let Some(config) = DashboardConfig::load(&state.app_dir) {
            let sid = session_id.clone();
            let title = session_record.as_ref().and_then(|s| s.title.clone());
            let summary = session_record.as_ref().and_then(|s| s.compressed_summary.clone());
            let resolved = session_record.as_ref().and_then(|s| s.resolved);
            let created_at = session_record.as_ref().map(|s| s.created_at.clone()).unwrap_or_default();
            let ended = ended_at.clone();
            tokio::spawn(async move {
                if let Err(e) = dashboard_link::push_session_report(
                    &config, &sid, title.as_deref(), summary.as_deref(),
                    message_count, resolved, &created_at, Some(&ended),
                ).await {
                    eprintln!("[fleet] Failed to push session report: {}", e);
                }
            });
        }
    }

    Ok(removed)
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionRecord>, String> {
    let conn = state.db.lock().await;
    journal::list_sessions(&conn).map_err(|e| format!("Failed to list sessions: {}", e))
}

#[tauri::command]
pub async fn get_session_messages(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<MessageRecord>, String> {
    let conn = state.db.lock().await;
    journal::get_messages(&conn, &session_id)
        .map_err(|e| format!("Failed to load messages: {}", e))
}

#[tauri::command]
pub async fn get_session_summary(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<String, String> {
    // Load messages from DB
    let messages = {
        let conn = state.db.lock().await;
        journal::get_messages(&conn, &session_id)
            .map_err(|e| format!("Failed to load messages: {}", e))?
    };

    if messages.is_empty() {
        return Ok("No messages in this session.".to_string());
    }

    // Build a condensed transcript for the LLM
    let transcript: String = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    // Truncate to ~2000 chars to keep Haiku call fast
    let truncated = if transcript.len() > 2000 {
        format!("{}...", &transcript[..2000])
    } else {
        transcript
    };

    let orchestrator = state.orchestrator.lock().await;
    orchestrator
        .generate_session_summary(&truncated)
        .await
        .map_err(|e| format!("Failed to generate summary: {}", e))
}

#[tauri::command]
pub async fn mark_resolved(
    state: State<'_, AppState>,
    session_id: String,
    resolved: bool,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    journal::mark_session_resolved(&conn, &session_id, resolved)
        .map_err(|e| format!("Failed to mark session: {}", e))?;

    // Push resolved status update to fleet if linked
    let session_record = journal::list_sessions(&conn)
        .ok()
        .and_then(|sessions| sessions.into_iter().find(|s| s.id == session_id));

    if let Some(config) = DashboardConfig::load(&state.app_dir) {
        let sid = session_id.clone();
        let title = session_record.as_ref().and_then(|s| s.title.clone());
        let summary = session_record.as_ref().and_then(|s| s.compressed_summary.clone());
        let message_count = session_record.as_ref().map(|s| s.message_count).unwrap_or(0);
        let created_at = session_record.as_ref().map(|s| s.created_at.clone()).unwrap_or_default();
        let ended_at = session_record.as_ref().and_then(|s| s.ended_at.clone());
        tokio::spawn(async move {
            if let Err(e) = dashboard_link::push_session_report(
                &config, &sid, title.as_deref(), summary.as_deref(),
                message_count, Some(resolved), &created_at, ended_at.as_deref(),
            ).await {
                eprintln!("[fleet] Failed to push session resolved update: {}", e);
            }
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn rename_session(
    state: State<'_, AppState>,
    session_id: String,
    title: String,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    journal::rename_session_title(&conn, &session_id, &title)
        .map_err(|e| format!("Failed to rename session: {}", e))
}

#[tauri::command]
pub async fn delete_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    journal::delete_session(&conn, &session_id)
        .map_err(|e| format!("Failed to delete session: {}", e))
}

#[tauri::command]
pub async fn export_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<String, String> {
    let conn = state.db.lock().await;

    // Get session metadata
    let sessions = journal::list_sessions(&conn)
        .map_err(|e| format!("Failed to list sessions: {}", e))?;
    let session = sessions.iter().find(|s| s.id == session_id);

    let title = session
        .and_then(|s| s.title.as_deref())
        .unwrap_or("Untitled Session");
    let created = session
        .map(|s| s.created_at.as_str())
        .unwrap_or("Unknown");

    // Get messages
    let messages = journal::get_messages(&conn, &session_id)
        .map_err(|e| format!("Failed to load messages: {}", e))?;

    // Get changes
    let changes = journal::get_changes(&conn, &session_id)
        .map_err(|e| format!("Failed to load changes: {}", e))?;

    // Build markdown
    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", title));
    md.push_str(&format!("**Date:** {}\n\n", created));
    md.push_str("---\n\n## Conversation\n\n");

    for msg in &messages {
        let role_label = match msg.role.as_str() {
            "user" => "**You**",
            "assistant" => "**Noah**",
            "system" => "*System*",
            _ => &msg.role,
        };
        md.push_str(&format!("{}: {}\n\n", role_label, msg.content));
    }

    if !changes.is_empty() {
        md.push_str("---\n\n## Changes Made\n\n");
        for change in &changes {
            let status = if change.undone { " (undone)" } else { "" };
            md.push_str(&format!(
                "- **{}**: {}{}\n",
                change.tool_name, change.description, status
            ));
        }
        md.push_str("\n");
    }

    md.push_str(&format!(
        "---\n\n*Exported from Noah v{}*\n",
        env!("CARGO_PKG_VERSION")
    ));

    Ok(md)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_info_json_keys() {
        // Ensures the JSON keys match the TS SessionInfo interface.
        let info = SessionInfo {
            id: "abc-123".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            message_count: 0,
        };
        let json = serde_json::to_value(&info).unwrap();
        let obj = json.as_object().unwrap();

        // TS expects: { id, created_at, message_count }
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("created_at"));
        assert!(obj.contains_key("message_count"));
        assert_eq!(obj.len(), 3, "Unexpected extra fields in SessionInfo");

        // Verify values roundtrip
        assert_eq!(obj["id"], "abc-123");
        assert_eq!(obj["message_count"], 0);
    }
}
