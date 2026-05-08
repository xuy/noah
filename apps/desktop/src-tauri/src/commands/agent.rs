use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::safety::journal;
use crate::AppState;

// ── Types ──

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssistantActionType {
    RunStep,
    WaitForUser,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantQuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantTextInput {
    pub placeholder: Option<String>,
    pub default: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantSecureInput {
    pub placeholder: Option<String>,
    pub secret_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantQuestion {
    pub question: String,
    pub header: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<AssistantQuestionOption>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_input: Option<AssistantTextInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure_input: Option<AssistantSecureInput>,
    #[serde(rename = "multiSelect", skip_serializing_if = "Option::is_none")]
    pub multi_select: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantCardAction {
    pub label: String,
    #[serde(rename = "type")]
    pub action_type: AssistantActionType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaybookProgress {
    pub step: u32,
    pub total: u32,
    pub label: String,
}

/// Structured diagnostic fact rendered as a label/value tile.
/// Mirror of `AssistantFinding` on the desktop TS side.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantFinding {
    pub label: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
}

/// One ordered remediation step. Mirror of `AssistantStep` on the TS side.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantStep {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantSpaUi {
    pub kind: String,
    pub situation: String,
    /// Structured diagnostic facts. Forwarded as-is to the desktop UI
    /// where they render as a tile grid. Dropping these here is a bug
    /// — without them the card renders as a bare situation only,
    /// regardless of what the model actually emitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub findings: Option<Vec<AssistantFinding>>,
    /// Ordered remediation plan. Wins over `plan` (markdown) when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<AssistantStep>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    pub action: AssistantCardAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<PlaybookProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qr_data: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantUserQuestionUi {
    pub kind: String,
    pub questions: Vec<AssistantQuestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<PlaybookProgress>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantInfoUi {
    pub kind: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<PlaybookProgress>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum AssistantUiPayload {
    Spa(AssistantSpaUi),
    UserQuestion(AssistantUserQuestionUi),
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
    UserAnswerQuestion,
}

#[derive(Debug, Deserialize)]
struct AnswerPayload {
    answer: Option<String>,
    answers: Option<serde_json::Value>,
}

// ── Parsing ──

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

fn parse_progress(v: &serde_json::Value) -> Option<PlaybookProgress> {
    let p = v.get("progress")?;
    Some(PlaybookProgress {
        step: p.get("step")?.as_u64()? as u32,
        total: p.get("total")?.as_u64()? as u32,
        label: p.get("label")?.as_str()?.to_string(),
    })
}

fn parse_assistant_ui_json(text: &str) -> Option<AssistantUiPayload> {
    let candidate = if let Some(start) = text.find("```json") {
        let rest = &text[start + "```json".len()..];
        let end = rest.find("```")?;
        rest[..end].trim().to_string()
    } else if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        text[start..=end].trim().to_string()
    } else {
        text.trim().to_string()
    };
    if !candidate.starts_with('{') {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&candidate).ok()?;
    let kind = v.get("kind")?.as_str()?.to_lowercase();
    let progress = parse_progress(&v);
    match kind.as_str() {
        "done" | "info" => {
            let summary = v.get("summary")?.as_str()?.to_string();
            Some(if kind == "done" {
                AssistantUiPayload::Done(AssistantInfoUi { kind, summary, progress })
            } else {
                AssistantUiPayload::Info(AssistantInfoUi { kind, summary, progress })
            })
        }
        "spa" => {
            let situation = v.get("situation")?.as_str()?.to_string();
            let plan = v.get("plan").and_then(|v| v.as_str()).map(|s| s.to_string());
            // Structured findings + steps — round-trip from the model
            // without re-validating shape (the orchestrator's ui_payload
            // builder already enforced schema before this string was
            // generated). Forwarding via serde_json::from_value lets us
            // accept any superset of fields the TS side knows about.
            let findings = v
                .get("findings")
                .and_then(|f| serde_json::from_value::<Vec<AssistantFinding>>(f.clone()).ok());
            let steps = v
                .get("steps")
                .and_then(|s| serde_json::from_value::<Vec<AssistantStep>>(s.clone()).ok());
            let action_v = v.get("action")?;
            let label = action_v.get("label")?.as_str()?.to_string();
            let action_type = action_v
                .get("type")
                .and_then(|x| x.as_str())
                .map(|s| s.to_uppercase())
                .and_then(|s| match s.as_str() {
                    "RUN_STEP" => Some(AssistantActionType::RunStep),
                    "WAIT_FOR_USER" => Some(AssistantActionType::WaitForUser),
                    _ => None,
                })
                .unwrap_or(AssistantActionType::RunStep);
            let qr_data = v.get("qr_data").and_then(|v| v.as_str()).map(|s| s.to_string());
            Some(AssistantUiPayload::Spa(AssistantSpaUi {
                kind: "spa".to_string(),
                situation,
                findings,
                steps,
                plan,
                action: AssistantCardAction {
                    label,
                    action_type,
                },
                progress,
                qr_data,
            }))
        }
        "user_question" => {
            let questions = v
                .get("questions")
                .and_then(|q| q.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|q| {
                            let question = q.get("question")?.as_str()?.to_string();
                            let header = q.get("header")?.as_str()?.to_string();
                            let multi_select = q
                                .get("multiSelect")
                                .and_then(|v| v.as_bool());

                            let options = q
                                .get("options")
                                .and_then(|o| o.as_array())
                                .map(|opts| {
                                    opts.iter()
                                        .filter_map(|o| {
                                            Some(AssistantQuestionOption {
                                                label: o.get("label")?.as_str()?.to_string(),
                                                description: o.get("description")?.as_str()?.to_string(),
                                            })
                                        })
                                        .collect::<Vec<_>>()
                                });

                            let text_input = q.get("text_input").map(|ti| {
                                AssistantTextInput {
                                    placeholder: ti.get("placeholder").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                    default: ti.get("default").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                }
                            });

                            let secure_input = q.get("secure_input").and_then(|si| {
                                Some(AssistantSecureInput {
                                    placeholder: si.get("placeholder").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                    secret_name: si.get("secret_name")?.as_str()?.to_string(),
                                })
                            });

                            Some(AssistantQuestion {
                                question,
                                header,
                                options,
                                text_input,
                                secure_input,
                                multi_select,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Some(AssistantUiPayload::UserQuestion(AssistantUserQuestionUi {
                kind: "user_question".to_string(),
                questions,
                progress,
            }))
        }
        _ => None,
    }
}

pub fn parse_assistant_ui(text: &str) -> Option<AssistantUiPayload> {
    if let Some(ui) = parse_assistant_ui_json(text) {
        return Some(ui);
    }
    if text.contains("[DONE]") {
        let summary = text
            .split_once("[DONE]")
            .map(|(_, s)| s.trim().to_string())
            .unwrap_or_default();
        return Some(AssistantUiPayload::Done(AssistantInfoUi {
            kind: "done".to_string(),
            summary,
            progress: None,
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
            progress: None,
        }));
    }

    let situation = parse_between(text, "[SITUATION]", "[PLAN]")?;
    let plan = parse_between(text, "[PLAN]", "[ACTION:")?;
    let label = parse_action_label(text)?;

    Some(AssistantUiPayload::Spa(AssistantSpaUi {
        kind: "spa".to_string(),
        situation,
        findings: None,
        steps: None,
        plan: Some(plan),
        action: AssistantCardAction {
            label,
            action_type: AssistantActionType::RunStep,
        },
        progress: None,
        qr_data: None,
    }))
}

// ── Shared agent turn logic ──

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

// ── Commands ──

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
            "Skip this optional step and continue.".to_string(),
            Some(true),
        ),
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
            (answer, Some(true))
        }
    };

    let text = run_agent_turn(state, app_handle, session_id, message, is_confirmation).await?;
    let assistant_ui = parse_assistant_ui(&text);
    Ok(SendMessageV2Result { text, assistant_ui })
}

#[tauri::command]
pub async fn store_secret(
    state: State<'_, AppState>,
    session_id: String,
    secret_name: String,
    secret_value: String,
) -> Result<(), String> {
    let mut orch = state.orchestrator.lock().await;
    orch.store_secret(&session_id, &secret_name, &secret_value);
    Ok(())
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
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(card.situation, "A");
                assert_eq!(card.plan, Some("B".to_string()));
                assert_eq!(card.action.label, "Do it");
                assert_eq!(card.action.action_type, AssistantActionType::RunStep);
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

    #[test]
    fn parses_json_card_ui() {
        let text = r#"{"kind":"spa","situation":"CPU is high","plan":"Stop heavy app","action":{"label":"Stop App","type":"RUN_STEP"}}"#;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(card.action.label, "Stop App");
                assert_eq!(card.situation, "CPU is high");
            }
            _ => panic!("expected json card ui"),
        }
    }

    #[test]
    fn parses_json_card_with_prefixed_text() {
        let text = r#"I ran diagnostics.
{
  "kind":"spa",
  "situation":"CPU is high",
  "plan":"Stop heavy app",
  "action":{"label":"Stop App","type":"RUN_STEP"}
}"#;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => assert_eq!(card.action.label, "Stop App"),
            _ => panic!("expected json card ui with preface"),
        }
    }

    // ── Real-world database message patterns ──

    #[test]
    fn parses_real_json_done_with_markdown() {
        let text = r###"{"kind":"done","summary":"## Wi-Fi Issue Resolved\n\nYour Wi-Fi connection is **rock solid**:\n- **Strong 6GHz**: -52 dBm\n- **High speed**: 1.7 Gbps"}"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Done(done)) => {
                assert!(done.summary.contains("Wi-Fi Issue Resolved"));
                assert!(done.summary.contains("rock solid"));
            }
            _ => panic!("expected done ui, got {:?}", ui),
        }
    }

    // ── Regression: structured findings + steps must round-trip ──
    //
    // History: parse_assistant_ui_json's spa branch only extracted
    // situation/plan/action/progress/qr_data, silently dropping any
    // findings[] and steps[] the model emitted. The desktop card then
    // rendered as a bare situation regardless of how rich the model's
    // tool call was. Symptom: `[noah-spa-shape] spa_bare findings=0
    // steps=0` in devtools console even when the model populated them.
    //
    // These tests pin the contract: any structured ui_spa JSON that
    // contains findings/steps MUST surface them in the parsed struct
    // so the frontend renders FindingsGrid + StepsList.

    #[test]
    fn spa_json_preserves_findings() {
        let text = r###"{
            "kind":"spa",
            "situation":"DNS is slow",
            "findings":[
                {"label":"DNS Resolution","value":"34ms","tone":"warn","sub":"slower than usual"},
                {"label":"Internet Ping","value":"19ms","tone":"good","sub":"avg, 3 packets"}
            ],
            "action":{"label":"Fix DNS","type":"RUN_STEP"}
        }"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                let findings = card.findings.expect("findings should round-trip");
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0].label, "DNS Resolution");
                assert_eq!(findings[0].value, "34ms");
                assert_eq!(findings[0].tone.as_deref(), Some("warn"));
                assert_eq!(findings[0].sub.as_deref(), Some("slower than usual"));
                assert_eq!(findings[1].label, "Internet Ping");
            }
            _ => panic!("expected spa ui, got {:?}", ui),
        }
    }

    #[test]
    fn spa_json_preserves_steps() {
        let text = r###"{
            "kind":"spa",
            "situation":"DNS is slow",
            "steps":[
                {"label":"Flush DNS cache","detail":"Clear cached DNS records"},
                {"label":"Re-test DNS performance","status":"pending","detail":"Verify improvement"}
            ],
            "action":{"label":"Fix DNS","type":"RUN_STEP"}
        }"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                let steps = card.steps.expect("steps should round-trip");
                assert_eq!(steps.len(), 2);
                assert_eq!(steps[0].label, "Flush DNS cache");
                assert_eq!(steps[0].detail.as_deref(), Some("Clear cached DNS records"));
                assert_eq!(steps[1].label, "Re-test DNS performance");
                assert_eq!(steps[1].status.as_deref(), Some("pending"));
            }
            _ => panic!("expected spa ui, got {:?}", ui),
        }
    }

    #[test]
    fn spa_json_with_both_findings_and_steps_renders_both() {
        // The exact shape that produced the original bug — model emitted
        // a rich SPA, the parser dropped the structured fields.
        let text = r###"{
            "kind":"spa",
            "situation":"Internet is slow due to high latency.",
            "findings":[
                {"label":"Internet Ping","value":"175ms","tone":"bad","sub":"avg, highly variable"},
                {"label":"DNS Lookup","value":"219ms","tone":"warn","sub":"slower than normal"},
                {"label":"HTTP to Google","value":"438ms","tone":"warn","sub":"total response time"},
                {"label":"Wi-Fi","value":"Connected","sub":"192.168.86.27"}
            ],
            "steps":[
                {"label":"Power-cycle your router","detail":"Unplug for 10 seconds, then plug back in"},
                {"label":"Restart Wi-Fi on your Mac","detail":"Turn off and back on in System Settings"},
                {"label":"Re-test internet speed","detail":"Verify improvement with speed test"}
            ],
            "action":{"label":"Fix it","type":"RUN_STEP"}
        }"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(
                    card.findings.as_ref().map(|f| f.len()),
                    Some(4),
                    "findings array dropped or wrong length",
                );
                assert_eq!(
                    card.steps.as_ref().map(|s| s.len()),
                    Some(3),
                    "steps array dropped or wrong length",
                );
                assert_eq!(card.action.label, "Fix it");
            }
            _ => panic!("expected spa ui, got {:?}", ui),
        }
    }

    #[test]
    fn spa_json_without_findings_or_steps_is_still_valid() {
        // Bare SPA (some model responses legitimately have no diagnostics
        // to report) — both fields should parse as None, NOT Some(empty).
        let text = r###"{
            "kind":"spa",
            "situation":"Restart needed.",
            "action":{"label":"Restart","type":"RUN_STEP"}
        }"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert!(card.findings.is_none(), "bare SPA should have no findings");
                assert!(card.steps.is_none(), "bare SPA should have no steps");
            }
            _ => panic!("expected spa ui, got {:?}", ui),
        }
    }

    #[test]
    fn parses_real_json_spa_run_step() {
        let text = r###"{"action":{"label":"Fix Wi-Fi stability","type":"RUN_STEP"},"kind":"spa","plan":"I'll flush the DNS cache to clear any connection hiccups.","situation":"Your Wi-Fi is actually **connected and stable**."}"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(card.action.label, "Fix Wi-Fi stability");
                assert_eq!(card.action.action_type, AssistantActionType::RunStep);
                assert!(card.situation.contains("connected and stable"));
            }
            _ => panic!("expected spa ui, got {:?}", ui),
        }
    }

    #[test]
    fn parses_unknown_action_type_as_run_step() {
        // Old sessions may have unknown action types — should gracefully default to RunStep
        let text = r###"{"action":{"label":"Enter Settings","type":"UNKNOWN_TYPE"},"kind":"spa","plan":"Open settings form.","situation":"Needs configuration."}"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(card.action.label, "Enter Settings");
                assert_eq!(card.action.action_type, AssistantActionType::RunStep);
            }
            _ => panic!("expected spa ui, got {:?}", ui),
        }
    }

    #[test]
    fn parses_another_unknown_action_type_as_run_step() {
        let text = r###"{"action":{"label":"Configure Service","type":"CUSTOM_FORM"},"kind":"spa","plan":"I'll guide you through setup.","situation":"Need configuration."}"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(card.action.label, "Configure Service");
                assert_eq!(card.action.action_type, AssistantActionType::RunStep);
            }
            _ => panic!("expected spa ui, got {:?}", ui),
        }
    }

    #[test]
    fn parses_real_json_user_question() {
        let text = r###"{"kind":"user_question","questions":[{"header":"Choose Setup Approach","multiSelect":false,"options":[{"description":"Set up from scratch","label":"Fresh Setup"},{"description":"Import existing config","label":"Import Config"}],"question":"How would you like to proceed?"}]}"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::UserQuestion(q)) => {
                assert_eq!(q.questions.len(), 1);
                assert_eq!(q.questions[0].header, "Choose Setup Approach");
                assert_eq!(q.questions[0].options.as_ref().expect("should have options").len(), 2);
            }
            _ => panic!("expected user_question ui, got {:?}", ui),
        }
    }

    #[test]
    fn parses_real_json_info() {
        let text = r###"{"kind":"info","summary":"Already Set Up!\n\nYour installation is complete and working."}"###;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Info(info)) => {
                assert!(info.summary.contains("Already Set Up"));
            }
            _ => panic!("expected info ui, got {:?}", ui),
        }
    }

    #[test]
    fn parses_legacy_info_marker() {
        let text = "[INFO]\nRunner timeout waiting for assistant response.";
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Info(info)) => {
                assert!(info.summary.contains("Runner timeout"));
            }
            _ => panic!("expected info ui from legacy marker"),
        }
    }

    #[test]
    fn parses_legacy_action_with_bold_situation() {
        let text = "[SITUATION]\nYour Mac has high load averages (3.85) caused by the Codex app using 54% CPU.\n\n[PLAN]\nI'll kill the high CPU Codex processes to reduce system load.\n\n[ACTION:Stop Codex]";
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(card.action.label, "Stop Codex");
                assert!(card.situation.contains("high load averages"));
            }
            _ => panic!("expected spa ui from legacy markers, got {:?}", ui),
        }
    }

    #[test]
    fn parses_legacy_action_marker_with_custom_label() {
        // Legacy marker with unusual label — should parse as Spa with RunStep
        let text = "[SITUATION]\nService needs configuration.\n[PLAN]\nCapture settings.\n[ACTION: CONFIGURE_SERVICE]";
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(card.action.label, "CONFIGURE_SERVICE");
                assert_eq!(card.action.action_type, AssistantActionType::RunStep);
            }
            _ => panic!("expected spa ui from legacy marker, got {:?}", ui),
        }
    }

    #[test]
    fn parses_plain_text_returns_none() {
        let text = "Hello, how can I help you today?";
        let ui = parse_assistant_ui(text);
        assert!(ui.is_none(), "plain text should return None");
    }

    #[test]
    fn parses_json_with_unknown_kind_returns_none() {
        let text = r#"{"kind":"unknown","data":"something"}"#;
        let ui = parse_assistant_ui(text);
        assert!(ui.is_none(), "unknown kind should return None");
    }
}
