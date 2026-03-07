use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::safety::journal;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssistantActionType {
    RunStep,
    OpenclawSecureCapture,
    Spa,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantQuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantQuestion {
    pub question: String,
    pub header: String,
    pub options: Vec<AssistantQuestionOption>,
    #[serde(rename = "multiSelect")]
    pub multi_select: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantCardAction {
    pub label: String,
    #[serde(rename = "type")]
    pub action_type: AssistantActionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub questions: Option<Vec<AssistantQuestion>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantCardUi {
    pub kind: String,
    pub situation: String,
    pub plan: String,
    pub action: AssistantCardAction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantInfoUi {
    pub kind: String,
    pub summary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum AssistantUiPayload {
    Card(AssistantCardUi),
    Done(AssistantInfoUi),
    Info(AssistantInfoUi),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SendMessageV2Result {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_ui: Option<AssistantUiPayload>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserEventType {
    UserConfirm,
    UserSkipOptional,
    UserSubmitSecureForm,
    UserAnswerQuestion,
}

#[derive(Debug, Deserialize)]
struct SecureFormPayload {
    credential_ref: String,
    provider: String,
    chat_channel: Option<String>,
    openclaw_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnswerPayload {
    answer: Option<String>,
    answers: Option<serde_json::Value>,
}

fn infer_action_type(context: &str, label: &str, has_questions: bool) -> AssistantActionType {
    if has_questions {
        return AssistantActionType::Spa;
    }
    let lower = format!("{}\n{}", context, label).to_lowercase();
    if lower.contains("openclaw")
        && (lower.contains("secure credential form")
            || lower.contains("secure form")
            || lower.contains("api key")
            || lower.contains("token"))
    {
        return AssistantActionType::OpenclawSecureCapture;
    }
    AssistantActionType::RunStep
}

fn parse_between<'a>(s: &'a str, start: &str, end: &str) -> Option<String> {
    let i = s.find(start)?;
    let rest = &s[i + start.len()..];
    let j = rest.find(end)?;
    Some(rest[..j].trim().to_string())
}

fn parse_action_label(s: &str) -> Option<String> {
    let marker = "[ACTION:";
    let i = s.find(marker)?;
    let rest = &s[i + marker.len()..];
    let j = rest.find(']')?;
    Some(rest[..j].trim().to_string())
}

fn parse_spa_questions(s: &str) -> Option<Vec<AssistantQuestion>> {
    let marker = "[SPA_QUESTIONS]";
    let i = s.find(marker)?;
    let raw = s[i + marker.len()..].trim();
    if raw.is_empty() {
        return None;
    }
    serde_json::from_str::<Vec<AssistantQuestion>>(raw).ok()
}

pub(crate) fn parse_assistant_ui(text: &str) -> Option<AssistantUiPayload> {
    if text.contains("[DONE]") {
        let summary = text
            .split_once("[DONE]")
            .map(|(_, s)| s.trim().to_string())
            .unwrap_or_default();
        return Some(AssistantUiPayload::Done(AssistantInfoUi {
            kind: "done".to_string(),
            summary,
        }));
    }
    if text.contains("[INFO]") {
        let summary = text
            .split_once("[INFO]")
            .map(|(_, s)| s.trim().to_string())
            .unwrap_or_default();
        return Some(AssistantUiPayload::Info(AssistantInfoUi {
            kind: "info".to_string(),
            summary,
        }));
    }

    let situation = parse_between(text, "[SITUATION]", "[PLAN]")?;
    let plan = parse_between(text, "[PLAN]", "[ACTION:")?;
    let label = parse_action_label(text)?;
    let questions = parse_spa_questions(text);
    let action_type = infer_action_type(text, &label, questions.is_some());

    Some(AssistantUiPayload::Card(AssistantCardUi {
        kind: "card".to_string(),
        situation,
        plan,
        action: AssistantCardAction {
            label,
            action_type,
            questions,
        },
    }))
}

async fn run_agent_turn(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    session_id: String,
    message: String,
    is_confirmation: Option<bool>,
) -> Result<String, String> {
    // Persist the user message for session history replay.
    {
        let conn = state.db.lock().await;
        let confirmation = is_confirmation.unwrap_or(false);
        if confirmation {
            if let Err(e) = journal::save_message_with_flags(&conn, &session_id, "user", &message, false, true) {
                eprintln!("[warn] Failed to persist user confirmation message: {}", e);
            }
            if let Err(e) = journal::mark_last_action_taken(&conn, &session_id) {
                eprintln!("[warn] Failed to mark action taken: {}", e);
            }
        } else if let Err(e) = journal::save_message(&conn, &session_id, "user", &message) {
            eprintln!("[warn] Failed to persist user message: {}", e);
        }
    }

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
                if let Err(e) = journal::update_session_title(&conn, &sid, &title) {
                    eprintln!("[warn] Failed to set session title: {}", e);
                }
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

    {
        let conn = state.db.lock().await;
        if let Err(e) = journal::save_message(&conn, &session_id, "assistant", &result) {
            eprintln!("[warn] Failed to persist assistant message: {}", e);
        }
    }
    if let Some(session) = orchestrator.get_session(&session_id) {
        let count = session.messages.len() as i32;
        let conn = state.db.lock().await;
        if let Err(e) = journal::update_session_message_count(&conn, &session_id, count) {
            eprintln!("[warn] Failed to update message count: {}", e);
        }
    }

    if let Some(handle) = title_handle {
        let _ = handle.await;
    }

    Ok(result)
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    session_id: String,
    message: String,
    is_confirmation: Option<bool>,
) -> Result<String, String> {
    run_agent_turn(state, app_handle, session_id, message, is_confirmation).await
}

#[tauri::command]
pub async fn send_message_v2(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    session_id: String,
    message: String,
    is_confirmation: Option<bool>,
) -> Result<SendMessageV2Result, String> {
    let text = run_agent_turn(state, app_handle, session_id, message, is_confirmation).await?;
    let assistant_ui = parse_assistant_ui(&text);
    Ok(SendMessageV2Result { text, assistant_ui })
}

#[tauri::command]
pub async fn send_user_event(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    session_id: String,
    event_type: UserEventType,
    payload: Option<String>,
) -> Result<SendMessageV2Result, String> {
    let (message, is_confirmation) = match event_type {
        UserEventType::UserConfirm => ("Go ahead".to_string(), Some(true)),
        UserEventType::UserSkipOptional => (
            "Skip this optional step and continue with basic setup.".to_string(),
            Some(true),
        ),
        UserEventType::UserSubmitSecureForm => {
            let raw = payload.ok_or_else(|| "payload required".to_string())?;
            let parsed: SecureFormPayload =
                serde_json::from_str(&raw).map_err(|e| format!("invalid payload: {}", e))?;
            let channel = parsed.chat_channel.unwrap_or_else(|| "none".to_string());
            let version_text = parsed
                .openclaw_version
                .unwrap_or_else(|| "installed".to_string());
            (
                format!(
                    "OpenClaw credentials were submitted via Noah secure form. Credential reference: {}. Provider: {}. Chat channel: {}. Please continue with validation and next setup checkpoint. OpenClaw version: {}.",
                    parsed.credential_ref, parsed.provider, channel, version_text
                ),
                Some(true),
            )
        }
        UserEventType::UserAnswerQuestion => {
            let raw = payload.ok_or_else(|| "payload required".to_string())?;
            let parsed: AnswerPayload =
                serde_json::from_str(&raw).map_err(|e| format!("invalid payload: {}", e))?;
            let answer = parsed.answer.unwrap_or_else(|| {
                parsed
                    .answers
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "provided".to_string())
            });
            (format!("User answer: {}", answer), Some(true))
        }
    };

    let text = run_agent_turn(state, app_handle, session_id, message, is_confirmation).await?;
    let assistant_ui = parse_assistant_ui(&text);
    Ok(SendMessageV2Result { text, assistant_ui })
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

#[tauri::command]
pub async fn cancel_processing(state: State<'_, AppState>) -> Result<(), String> {
    // Set the cancellation flag — doesn't require the orchestrator lock.
    state.cancelled.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn record_action_confirmation(
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    journal::save_message_with_flags(&conn, &session_id, "user", &message, false, true)
        .map_err(|e| format!("Failed to save confirmation message: {}", e))?;
    journal::mark_last_action_taken(&conn, &session_id)
        .map_err(|e| format!("Failed to mark action taken: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_card_ui() {
        let text = "[SITUATION]\nA\n[PLAN]\nB\n[ACTION:Do it]";
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Card(card)) => {
                assert_eq!(card.situation, "A");
                assert_eq!(card.plan, "B");
                assert_eq!(card.action.label, "Do it");
            }
            _ => panic!("expected card ui"),
        }
    }

    #[test]
    fn parses_done_ui() {
        let text = "[DONE]\nAll set";
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Done(done)) => assert_eq!(done.summary, "All set"),
            _ => panic!("expected done ui"),
        }
    }
}
