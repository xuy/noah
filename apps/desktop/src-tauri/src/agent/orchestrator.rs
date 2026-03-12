use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use noah_tools::SafetyTier;

use crate::agent::llm_client::{
    is_context_limit_error, ContentBlock, LlmClient, Message, MessageContent, ResponseBlock,
};
use crate::agent::prompts;
use crate::agent::tool_router::ToolRouter;
use crate::knowledge;
use crate::playbooks::PlaybookState;
use crate::safety::journal;
use crate::ui_tools;

/// A pending approval that the frontend must accept or deny.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub approval_id: String,
    pub tool_name: String,
    pub description: String,
    pub parameters: Value,
    /// Plain-language reason from the LLM explaining why this action is needed.
    pub reason: String,
}

/// A debug event emitted to the frontend for observability.
#[derive(Debug, Clone, Serialize)]
struct DebugEvent {
    timestamp: String,
    event_type: String,
    summary: String,
    detail: Value,
}

fn emit_debug<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    event_type: &str,
    summary: &str,
    detail: Value,
) {
    use tauri::Emitter;

    let event = DebugEvent {
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        event_type: event_type.to_string(),
        summary: summary.to_string(),
        detail,
    };
    let _ = app_handle.emit("debug-log", &event);
}

/// Session state kept in memory.
pub struct Session {
    pub id: String,
    pub messages: Vec<Message>,
    pub compressed_summary: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Active playbook state for progress tracking. Set when `activate_playbook` is called.
    pub playbook: Option<PlaybookState>,
    /// Ephemeral secret store for secure_input values. Keyed by `secret_name`.
    /// Values never enter LLM context. Cleared when session ends.
    pub secrets: HashMap<String, String>,
    /// User's preferred locale (e.g. "en", "zh"). Used to hint the LLM response language.
    pub locale: Option<String>,
    /// Session mode: "default" for normal chat, "learn" for knowledge-creation flow.
    pub mode: String,
}

pub struct Orchestrator {
    llm: LlmClient,
    router: ToolRouter,
    sessions: HashMap<String, Session>,
    /// Pending approval channels: approval_id -> oneshot sender (true = approved).
    pending_approvals: PendingApprovals,
    os_context: String,
    db: Arc<Mutex<rusqlite::Connection>>,
    /// Path to the knowledge directory for building the live TOC (includes `playbooks/`).
    knowledge_dir: std::path::PathBuf,
    /// Set to true to cancel the current agentic loop.
    cancelled: Arc<AtomicBool>,
}

pub type PendingApprovals = Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>;

const ESTIMATED_MAX_CONTEXT_TOKENS: usize = 160_000;
const CONTEXT_SOFT_THRESHOLD_TOKENS: usize = ESTIMATED_MAX_CONTEXT_TOKENS * 4 / 5;
const CONTEXT_HARD_THRESHOLD_TOKENS: usize = ESTIMATED_MAX_CONTEXT_TOKENS * 9 / 10;
const RECENT_MESSAGES_TO_KEEP: usize = 6;
const MAX_SUMMARY_INPUT_CHARS: usize = 48_000;
const FALLBACK_SUMMARY_EXCERPT_CHARS: usize = 1_600;

impl Orchestrator {
    pub fn new(
        llm: LlmClient,
        router: ToolRouter,
        os_context: String,
        pending_approvals: PendingApprovals,
        db: Arc<Mutex<rusqlite::Connection>>,
        knowledge_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            llm,
            router,
            sessions: HashMap::new(),
            pending_approvals,
            os_context,
            db,
            knowledge_dir,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    // ── Session management ─────────────────────────────────────────────

    pub fn create_session(&mut self) -> String {
        let id = Uuid::new_v4().to_string();
        let session = Session {
            id: id.clone(),
            messages: Vec::new(),
            compressed_summary: None,
            created_at: chrono::Utc::now(),
            playbook: None,
            secrets: HashMap::new(),
            locale: None,
            mode: "default".to_string(),
        };
        self.sessions.insert(id.clone(), session);
        id
    }

    /// Store a secret value from a secure_input response. Never enters LLM context.
    pub fn store_secret(&mut self, session_id: &str, name: &str, value: &str) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.secrets.insert(name.to_string(), value.to_string());
        }
    }

    /// Retrieve a stored secret by name (for write_secret tool).
    pub fn get_secret(&self, session_id: &str, name: &str) -> Option<String> {
        self.sessions
            .get(session_id)
            .and_then(|s| s.secrets.get(name).cloned())
    }

    pub fn set_locale(&mut self, session_id: &str, locale: &str) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.locale = Some(locale.to_string());
        }
    }

    pub fn get_locale(&self, session_id: &str) -> Option<String> {
        self.sessions.get(session_id).and_then(|s| s.locale.clone())
    }

    pub fn set_mode(&mut self, session_id: &str, mode: &str) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.mode = mode.to_string();
        }
    }

    pub fn get_session(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id)
    }

    pub fn end_session(&mut self, session_id: &str) -> bool {
        self.sessions.remove(session_id).is_some()
    }

    pub fn set_api_key(&mut self, key: String) {
        self.llm.set_api_key(key);
    }

    pub fn set_auth(&mut self, auth: crate::agent::llm_client::AuthMode) {
        self.llm.set_auth(auth);
    }

    pub fn has_api_key(&self) -> bool {
        self.llm.has_auth()
    }

    pub fn auth_mode_name(&self) -> &str {
        self.llm.auth_mode_name()
    }

    /// Get a clone of the LLM client for background tasks (e.g. title generation).
    pub fn llm_clone(&self) -> LlmClient {
        self.llm.clone()
    }

    /// Generate a session summary using Haiku.
    pub async fn generate_session_summary(&self, transcript: &str) -> anyhow::Result<String> {
        self.llm.generate_session_summary(transcript).await
    }

    // ── Approval flow ──────────────────────────────────────────────────

    /// Resolve a pending approval. Returns false if the approval_id was not found.
    pub async fn resolve_approval(&self, approval_id: &str, approved: bool) -> bool {
        let mut pending = self.pending_approvals.lock().await;
        if let Some(sender) = pending.remove(approval_id) {
            let _ = sender.send(approved);
            true
        } else {
            false
        }
    }

    /// Signal cancellation of the current agentic loop.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Get a clone of the cancellation flag for external cancellation.
    pub fn cancelled_flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    async fn restore_session_if_needed(&mut self, session_id: &str) -> Result<()> {
        if self.sessions.contains_key(session_id) {
            return Ok(());
        }

        let conn = self.db.lock().await;
        let session_record = journal::get_session(&conn, session_id)
            .context("Failed to load session metadata from database")?;

        let Some(session_record) = session_record else {
            anyhow::bail!("Session not found: {}", session_id);
        };

        let messages = if session_record
            .compressed_summary
            .as_deref()
            .is_some_and(|summary| !summary.trim().is_empty())
        {
            journal::get_recent_messages(&conn, session_id, RECENT_MESSAGES_TO_KEEP)
                .context("Failed to load recent session messages from database")?
        } else {
            journal::get_messages(&conn, session_id)
                .context("Failed to load session messages from database")?
        };
        drop(conn);

        let restored_messages: Vec<Message> = messages
            .iter()
            .map(|record| Message {
                role: record.role.clone(),
                content: MessageContent::Text(record.content.clone()),
            })
            .collect();

        let created_at = chrono::DateTime::parse_from_rfc3339(&session_record.created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        let session = Session {
            id: session_id.to_string(),
            messages: restored_messages,
            compressed_summary: session_record.compressed_summary,
            created_at,
            playbook: None,
            secrets: HashMap::new(),
            locale: None,
            mode: "default".to_string(),
        };
        self.sessions.insert(session_id.to_string(), session);
        Ok(())
    }

    // ── Agentic loop ───────────────────────────────────────────────────

    /// Send a user message and run the agentic loop until a text response
    /// is produced. The `app_handle` is used to emit approval-request events
    /// and the `db` connection is used to record changes in the journal.
    pub async fn send_message<R: tauri::Runtime>(
        &mut self,
        session_id: &str,
        user_message: &str,
        app_handle: &tauri::AppHandle<R>,
        db: &tokio::sync::Mutex<rusqlite::Connection>,
    ) -> Result<String> {
        self.restore_session_if_needed(session_id).await?;

        // Add the user message to history.
        {
            let session = self.sessions.get_mut(session_id).unwrap();
            session.messages.push(Message {
                role: "user".to_string(),
                content: MessageContent::Text(user_message.to_string()),
            });
        }

        let knowledge_ctx = knowledge::knowledge_toc(&self.knowledge_dir).unwrap_or_default();
        let locale = self.sessions[session_id].locale.clone();
        let mode = self.sessions[session_id].mode.clone();
        let system = prompts::system_prompt_blocks(
            &self.os_context,
            &knowledge_ctx,
            locale.as_deref(),
            &mode,
        );
        let tool_defs = self.router.tool_definitions();

        // Reset cancellation flag at the start of each user message.
        self.cancelled.store(false, Ordering::SeqCst);

        // Accumulate text across all loop iterations so we don't lose text
        // from turns where the LLM returns both text AND tool calls.
        let mut all_text_parts: Vec<String> = Vec::new();
        let mut ui_protocol_retries = 0usize;

        let mut handle_ui_protocol_error = |reason: String| -> Option<String> {
            ui_protocol_retries += 1;
            if ui_protocol_retries >= 3 {
                return Some(
                    json!({
                        "kind": "info",
                        "summary": format!(
                            "I hit an internal response-format issue ({}). Please try again.",
                            reason
                        )
                    })
                    .to_string(),
                );
            }
            None
        };

        // Agentic loop: keep calling the LLM until we get a text-only response.
        loop {
            // Check for cancellation before each iteration.
            if self.cancelled.load(Ordering::SeqCst) {
                all_text_parts.push("[INFO] Stopped by user.".to_string());
                return Ok(all_text_parts.join("\n"));
            }

            self.compress_session_context_if_needed(session_id, false)
                .await?;
            let messages = self.messages_for_llm(session_id);

            emit_debug(
                app_handle,
                "llm_request",
                &format!(
                    "Calling Claude with {} messages, {} tools",
                    messages.len(),
                    tool_defs.len()
                ),
                json!({
                    "message_count": messages.len(),
                    "tool_count": tool_defs.len(),
                    "last_user_message_preview": user_message.chars().take(200).collect::<String>(),
                }),
            );

            let response = match self
                .llm
                .send_message(messages.clone(), tool_defs.clone(), system.clone())
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    let err_text = err.to_string();
                    if err_text.starts_with("Context limit exceeded:")
                        || is_context_limit_error(reqwest::StatusCode::BAD_REQUEST, &err_text)
                    {
                        emit_debug(
                            app_handle,
                            "context_compression_retry",
                            "Context limit hit; compressing history and retrying",
                            json!({
                                "session_id": session_id,
                                "message_count": messages.len(),
                            }),
                        );
                        self.compress_session_context_if_needed(session_id, true)
                            .await?;
                        let retry_messages = self.messages_for_llm(session_id);
                        self.llm
                            .send_message(retry_messages, tool_defs.clone(), system.clone())
                            .await?
                    } else {
                        return Err(err);
                    }
                }
            };

            // Save LLM trace for debugging.
            {
                let request_json = serde_json::to_string(&json!({
                    "message_count": messages.len(),
                    "tool_count": tool_defs.len(),
                }))
                .unwrap_or_default();
                let response_json = serde_json::to_string(&response).unwrap_or_default();
                let conn = self.db.lock().await;
                if let Err(e) =
                    journal::save_llm_trace(&conn, session_id, &request_json, &response_json)
                {
                    eprintln!("[warn] Failed to save LLM trace: {}", e);
                }
            }

            // Collect tool_use blocks and text blocks.
            let mut tool_uses: Vec<(String, String, Value)> = Vec::new();
            let mut text_parts: Vec<String> = Vec::new();

            for block in &response.content {
                match block {
                    ResponseBlock::Text { text } => {
                        text_parts.push(text.clone());
                    }
                    ResponseBlock::ToolUse { id, name, input } => {
                        tool_uses.push((id.clone(), name.clone(), input.clone()));
                    }
                }
            }

            emit_debug(
                app_handle,
                "llm_response",
                &format!(
                    "Response: {} text blocks, {} tool calls, stop={}",
                    text_parts.len(),
                    tool_uses.len(),
                    response.stop_reason.as_deref().unwrap_or("none"),
                ),
                json!({
                    "stop_reason": response.stop_reason,
                    "text_blocks": text_parts.len(),
                    "tool_calls": tool_uses.len(),
                    "text_preview": text_parts.join("\n").chars().take(500).collect::<String>(),
                    "usage": {
                        "input_tokens": response.usage.input_tokens,
                        "output_tokens": response.usage.output_tokens,
                    },
                }),
            );

            // Accumulate any text from this turn.
            all_text_parts.extend(text_parts);

            // Add the assistant message to history (as blocks).
            let assistant_blocks: Vec<ContentBlock> = response
                .content
                .iter()
                .map(|b| match b {
                    ResponseBlock::Text { text } => ContentBlock::Text { text: text.clone() },
                    ResponseBlock::ToolUse { id, name, input } => ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    },
                })
                .collect();

            {
                let session = self.sessions.get_mut(session_id).unwrap();
                session.messages.push(Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Blocks(assistant_blocks),
                });
            }

            // Intercept UI tool calls — exactly 1 ui_* call, no mixing with other tools.
            if !tool_uses.is_empty() {
                let ui_calls: Vec<&(String, String, Value)> = tool_uses
                    .iter()
                    .filter(|(_, name, _)| {
                        matches!(
                            name.as_str(),
                            "ui_spa" | "ui_user_question" | "ui_info" | "ui_done"
                        )
                    })
                    .collect();
                if !ui_calls.is_empty() {
                    if ui_calls.len() != tool_uses.len() || ui_calls.len() != 1 {
                        // Policy violation: mixed or multiple ui_* calls
                        let guard_msg = "Policy guard: do not mix ui_* tools with other tools or multiple ui_* tools in one turn. Use exactly one ui_* tool call as the final response step.";
                        let mut blocks = Vec::new();
                        for (id, _, _) in &tool_uses {
                            blocks.push(ContentBlock::ToolResult {
                                tool_use_id: id.clone(),
                                content: guard_msg.to_string(),
                                is_error: Some(true),
                            });
                        }
                        let session = self.sessions.get_mut(session_id).unwrap();
                        session.messages.push(Message {
                            role: "user".to_string(),
                            content: MessageContent::Blocks(blocks),
                        });
                        if let Some(fallback) =
                            handle_ui_protocol_error("mixed/multiple ui_* calls".to_string())
                        {
                            return Ok(fallback);
                        }
                        continue;
                    }
                    // Exactly one ui_* call — validate and return the payload
                    let (tool_use_id, name, input) = ui_calls[0];
                    match ui_tools::ui_payload_from_tool_call(name, input) {
                        Ok(payload) => {
                            // Inject playbook progress if active.
                            let payload = {
                                let session = self.sessions.get_mut(session_id).unwrap();
                                let payload = if let Some(ref mut pb) = session.playbook {
                                    if let Some(progress) = pb.progress_json() {
                                        // Parse, inject progress, re-serialize.
                                        if let Ok(mut v) = serde_json::from_str::<Value>(&payload) {
                                            v["progress"] = progress;
                                            pb.advance();
                                            v.to_string()
                                        } else {
                                            pb.advance();
                                            payload
                                        }
                                    } else {
                                        payload
                                    }
                                } else {
                                    payload
                                };
                                payload
                            };

                            let session = self.sessions.get_mut(session_id).unwrap();
                            session.messages.push(Message {
                                role: "user".to_string(),
                                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                                    tool_use_id: tool_use_id.clone(),
                                    content: payload.clone(),
                                    is_error: None,
                                }]),
                            });
                            return Ok(payload);
                        }
                        Err(err) => {
                            let session = self.sessions.get_mut(session_id).unwrap();
                            session.messages.push(Message {
                                role: "user".to_string(),
                                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                                    tool_use_id: tool_use_id.clone(),
                                    content: format!(
                                        "Policy guard: invalid ui_* payload: {}. Re-emit one valid ui_* tool call matching schema.",
                                        err
                                    ),
                                    is_error: Some(true),
                                }]),
                            });
                            if let Some(fallback) =
                                handle_ui_protocol_error(format!("invalid ui_* payload: {}", err))
                            {
                                return Ok(fallback);
                            }
                            continue;
                        }
                    }
                }
            }

            // If no tool calls, we're done — return all accumulated text.
            if tool_uses.is_empty() {
                return Ok(all_text_parts.join("\n"));
            }

            // Execute each tool call.
            let mut tool_result_blocks: Vec<ContentBlock> = Vec::new();

            for (tool_use_id, tool_name, tool_input) in tool_uses {
                // Check for cancellation between tool calls.
                if self.cancelled.load(Ordering::SeqCst) {
                    tool_result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id,
                        content: "Cancelled by user.".to_string(),
                        is_error: Some(true),
                    });
                    continue;
                }

                let tier_label = self
                    .router
                    .find_tool(&tool_name)
                    .map(|t| format!("{:?}", t.safety_tier_for_input(&tool_input)))
                    .unwrap_or_else(|| "unknown".to_string());

                emit_debug(
                    app_handle,
                    "tool_call",
                    &format!("Calling {} [{}]", tool_name, tier_label),
                    json!({
                        "name": tool_name,
                        "input": tool_input,
                        "safety_tier": tier_label,
                    }),
                );

                let result = self
                    .execute_tool(session_id, &tool_name, &tool_input, app_handle, db)
                    .await;

                match result {
                    Ok(ref output) => {
                        // Detect activate_playbook and set up runtime state.
                        if tool_name == "activate_playbook" {
                            if let Some(pb_name) = tool_input.get("name").and_then(|v| v.as_str()) {
                                let state = PlaybookState::from_content(pb_name, output);
                                if !state.steps.is_empty() {
                                    emit_debug(
                                        app_handle,
                                        "playbook_activated",
                                        &format!(
                                            "Playbook '{}' activated with {} steps",
                                            pb_name, state.total_steps
                                        ),
                                        json!({
                                            "playbook": pb_name,
                                            "total_steps": state.total_steps,
                                            "steps": state.steps.iter().map(|s| json!({
                                                "number": s.number,
                                                "label": s.label,
                                            })).collect::<Vec<_>>(),
                                        }),
                                    );
                                }
                                let session = self.sessions.get_mut(session_id).unwrap();
                                session.playbook = Some(state);
                            }
                        }

                        emit_debug(
                            app_handle,
                            "tool_result",
                            &format!("{} completed", tool_name),
                            json!({
                                "name": tool_name,
                                "output_preview": output.chars().take(500).collect::<String>(),
                            }),
                        );
                        tool_result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id,
                            content: output.clone(),
                            is_error: None,
                        });
                    }
                    Err(ref e) => {
                        emit_debug(
                            app_handle,
                            "error",
                            &format!("{} failed: {}", tool_name, e),
                            json!({
                                "name": tool_name,
                                "error": format!("{}", e),
                            }),
                        );
                        tool_result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id,
                            content: format!("Error: {}", e),
                            is_error: Some(true),
                        });
                    }
                }
            }

            // Add tool results as a user message.
            {
                let session = self.sessions.get_mut(session_id).unwrap();
                session.messages.push(Message {
                    role: "user".to_string(),
                    content: MessageContent::Blocks(tool_result_blocks),
                });
            }
        }
    }

    fn messages_for_llm(&self, session_id: &str) -> Vec<Message> {
        let session = &self.sessions[session_id];
        let mut messages = Vec::with_capacity(session.messages.len() + 1);

        if let Some(summary) = session
            .compressed_summary
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            messages.push(Message {
                role: "assistant".to_string(),
                content: MessageContent::Text(format!(
                    "[Compressed session context]\n{}\n\nKeep this summary in mind, but rely on the most recent verbatim turns when they conflict.",
                    summary
                )),
            });
        }

        messages.extend(session.messages.iter().cloned());
        messages
    }

    async fn compress_session_context_if_needed(
        &mut self,
        session_id: &str,
        force: bool,
    ) -> Result<()> {
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(());
        };

        if session.messages.len() <= RECENT_MESSAGES_TO_KEEP {
            return Ok(());
        }

        let estimated_tokens = estimate_messages_tokens(&session.messages)
            + session
                .compressed_summary
                .as_deref()
                .map(estimate_tokens)
                .unwrap_or_default();
        let threshold = if force {
            CONTEXT_HARD_THRESHOLD_TOKENS / 2
        } else {
            CONTEXT_SOFT_THRESHOLD_TOKENS
        };

        if estimated_tokens < threshold {
            return Ok(());
        }

        let split_at = if force {
            session.messages.len().saturating_sub(4).max(1)
        } else {
            session
                .messages
                .len()
                .saturating_sub(RECENT_MESSAGES_TO_KEEP)
        };
        let older_messages = session.messages[..split_at].to_vec();
        let existing_summary = session.compressed_summary.clone();

        let summary_input = summarized_transcript(&older_messages, MAX_SUMMARY_INPUT_CHARS);
        if summary_input.trim().is_empty() {
            return Ok(());
        }

        let updated_summary = match self
            .llm
            .generate_context_summary(existing_summary.as_deref(), &summary_input)
            .await
        {
            Ok(summary) => summary,
            Err(err) => {
                eprintln!("[warn] Context summary generation failed: {}", err);
                fallback_context_summary(existing_summary.as_deref(), &summary_input)
            }
        };

        let session = self.sessions.get_mut(session_id).unwrap();
        session.compressed_summary = Some(updated_summary);
        session.messages = session.messages.split_off(split_at);
        let persisted_summary = session.compressed_summary.clone();

        let conn = self.db.lock().await;
        journal::update_session_compressed_summary(
            &conn,
            session_id,
            persisted_summary.as_deref(),
        )?;

        Ok(())
    }

    /// Execute a single tool call, handling safety tier checks and approvals.
    async fn execute_tool<R: tauri::Runtime>(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: &Value,
        app_handle: &tauri::AppHandle<R>,
        db: &tokio::sync::Mutex<rusqlite::Connection>,
    ) -> Result<String> {
        let tool = self
            .router
            .find_tool(tool_name)
            .context(format!("Unknown tool: {}", tool_name))?;

        // For write_secret, inject the actual secret value from the session's secret store.
        let tool_input = if tool_name == "write_secret" {
            let mut input = tool_input.clone();
            if let Some(secret_name) = input.get("secret_name").and_then(|v| v.as_str()) {
                if let Some(secret_value) = self.get_secret(session_id, secret_name) {
                    input["__secret_value__"] = json!(secret_value);
                }
            }
            input
        } else {
            tool_input.clone()
        };
        let tool_input = &tool_input;

        let tier = tool.safety_tier_for_input(tool_input);

        // For NeedsApproval tools, request approval from the frontend.
        if tier == SafetyTier::NeedsApproval {
            let reason = tool_input
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let approved = self
                .request_approval(
                    app_handle,
                    tool_name,
                    tool.description(),
                    tool_input,
                    &reason,
                )
                .await?;

            if !approved {
                emit_debug(
                    app_handle,
                    "tool_denied",
                    &format!("User denied {}", tool_name),
                    json!({
                        "name": tool_name,
                        "input": tool_input,
                    }),
                );
                return Ok("Action denied by user.".to_string());
            }
        }

        // Execute the tool.
        let tool_result = tool.execute(tool_input).await?;

        // Record any changes in the journal.
        if !tool_result.changes.is_empty() {
            let conn = db.lock().await;
            for change in &tool_result.changes {
                if let Err(e) = journal::record_change(&conn, session_id, tool_name, change) {
                    eprintln!("[warn] Failed to record change in journal: {}", e);
                }
            }
        }

        Ok(tool_result.output)
    }

    /// Emit an approval-request event to the frontend and wait for the response.
    async fn request_approval<R: tauri::Runtime>(
        &self,
        app_handle: &tauri::AppHandle<R>,
        tool_name: &str,
        description: &str,
        parameters: &Value,
        reason: &str,
    ) -> Result<bool> {
        use tauri::Emitter;

        let approval_id = Uuid::new_v4().to_string();

        let (tx, rx) = oneshot::channel::<bool>();

        // Store the sender so approve_action / deny_action can resolve it.
        {
            let mut pending = self.pending_approvals.lock().await;
            pending.insert(approval_id.clone(), tx);
        }

        let request = ApprovalRequest {
            approval_id: approval_id.clone(),
            tool_name: tool_name.to_string(),
            description: description.to_string(),
            parameters: parameters.clone(),
            reason: reason.to_string(),
        };

        app_handle
            .emit("approval-request", &request)
            .context("Failed to emit approval-request event")?;

        // Wait for the frontend to respond, with a 5-minute timeout.
        let approved = match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
            Ok(result) => result.unwrap_or(false),
            Err(_) => {
                // Timeout expired — clean up the pending approval and auto-deny.
                {
                    let mut pending = self.pending_approvals.lock().await;
                    pending.remove(&approval_id);
                }

                emit_debug(
                    app_handle,
                    "approval_timeout",
                    &format!("Approval for {} timed out after 5 minutes", tool_name),
                    json!({
                        "approval_id": approval_id,
                        "tool_name": tool_name,
                    }),
                );

                // Emit a timeout event so the frontend can dismiss the approval modal.
                {
                    use tauri::Emitter;
                    let _ = app_handle.emit(
                        "approval-timeout",
                        json!({
                            "approval_id": approval_id,
                        }),
                    );
                }

                false
            }
        };

        Ok(approved)
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

fn estimate_message_tokens(message: &Message) -> usize {
    let role_overhead = 8;
    role_overhead + estimate_tokens(&render_message_for_summary(message))
}

fn estimate_messages_tokens(messages: &[Message]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

fn render_message_for_summary(message: &Message) -> String {
    let role = match message.role.as_str() {
        "user" => "User",
        "assistant" => "Assistant",
        other => other,
    };

    match &message.content {
        MessageContent::Text(text) => format!("{}: {}", role, text),
        MessageContent::Blocks(blocks) => {
            let rendered = blocks
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => text.clone(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        format!("Tool call `{}` with input {}", name, input)
                    }
                    ContentBlock::ToolResult {
                        content, is_error, ..
                    } => {
                        if is_error.unwrap_or(false) {
                            format!("Tool error: {}", content)
                        } else {
                            format!("Tool result: {}", content)
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{}: {}", role, rendered)
        }
    }
}

fn summarized_transcript(messages: &[Message], max_chars: usize) -> String {
    let mut sections = Vec::new();
    let mut used = 0usize;

    for message in messages.iter().rev() {
        let mut rendered = render_message_for_summary(message);
        if rendered.chars().count() > 1_500 {
            rendered = format!("{}...", rendered.chars().take(1_500).collect::<String>());
        }

        let rendered_len = rendered.chars().count();
        if used + rendered_len > max_chars {
            break;
        }

        used += rendered_len;
        sections.push(rendered);
    }

    sections.reverse();
    sections.join("\n\n")
}

fn fallback_context_summary(existing_summary: Option<&str>, messages_text: &str) -> String {
    let mut sections = Vec::new();

    if let Some(existing) = existing_summary
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
    {
        sections.push(existing.to_string());
    }

    sections.push(
        "## Pending / watch-outs\n- Older session history was trimmed after automatic compression failed. Use the most recent verbatim turns for exact details."
            .to_string(),
    );

    let excerpt_chars: Vec<char> = messages_text.chars().collect();
    let start = excerpt_chars
        .len()
        .saturating_sub(FALLBACK_SUMMARY_EXCERPT_CHARS);
    let excerpt = excerpt_chars[start..].iter().collect::<String>();
    if !excerpt.trim().is_empty() {
        sections.push(format!(
            "## Recent older context excerpt\n{}",
            excerpt.trim()
        ));
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::llm_client::{AuthMode, LlmClient};
    use crate::agent::tool_router::ToolRouter;

    fn test_orchestrator() -> Orchestrator {
        test_orchestrator_with_llm(LlmClient::new(String::new()))
    }

    fn test_orchestrator_with_llm(llm: LlmClient) -> Orchestrator {
        let router = ToolRouter::new();
        let conn = crate::safety::journal::init_db(":memory:").expect("Failed to init test DB");
        Orchestrator::new(
            llm,
            router,
            "test context".to_string(),
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(Mutex::new(conn)),
            std::path::PathBuf::from("/tmp/test-knowledge"),
        )
    }

    #[test]
    fn test_create_session_returns_unique_ids() {
        let mut orch = test_orchestrator();
        let id1 = orch.create_session();
        let id2 = orch.create_session();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_get_session_works() {
        let mut orch = test_orchestrator();
        let id = orch.create_session();
        assert!(orch.get_session(&id).is_some());
        assert!(orch.get_session("nonexistent").is_none());
    }

    #[test]
    fn test_end_session_removes_it() {
        let mut orch = test_orchestrator();
        let id = orch.create_session();
        assert!(orch.end_session(&id));
        assert!(orch.get_session(&id).is_none());
        assert!(!orch.end_session(&id)); // already gone
    }

    #[tokio::test]
    async fn test_approval_resolve_approved() {
        let orch = test_orchestrator();
        let (tx, rx) = oneshot::channel::<bool>();
        {
            let mut pending = orch.pending_approvals.lock().await;
            pending.insert("req-1".to_string(), tx);
        }
        let found = orch.resolve_approval("req-1", true).await;
        assert!(found);
        assert!(rx.await.unwrap());
    }

    #[tokio::test]
    async fn test_approval_resolve_missing() {
        let orch = test_orchestrator();
        let found = orch.resolve_approval("nonexistent", true).await;
        assert!(!found);
    }

    // ── Stage 0 tests ──

    #[test]
    fn test_cancel_flag_starts_false() {
        let orch = test_orchestrator();
        assert!(!orch.cancelled.load(Ordering::SeqCst));
    }

    #[test]
    fn test_cancel_sets_flag() {
        let orch = test_orchestrator();
        orch.cancel();
        assert!(orch.cancelled.load(Ordering::SeqCst));
    }

    #[test]
    fn test_cancelled_flag_is_shared() {
        let orch = test_orchestrator();
        let flag = orch.cancelled_flag();
        assert!(!flag.load(Ordering::SeqCst));
        orch.cancel();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_approval_request_json_keys() {
        // Ensures the JSON keys match the TS ApprovalRequest interface.
        let req = ApprovalRequest {
            approval_id: "req-1".to_string(),
            tool_name: "mac_kill_process".to_string(),
            description: "Kill process 1234".to_string(),
            parameters: serde_json::json!({"pid": 1234}),
            reason: "Stop a frozen application that is using too much CPU".to_string(),
        };
        let json = serde_json::to_value(&req).unwrap();
        let obj = json.as_object().unwrap();

        // TS expects: { approval_id, tool_name, description, parameters, reason }
        for key in [
            "approval_id",
            "tool_name",
            "description",
            "parameters",
            "reason",
        ] {
            assert!(obj.contains_key(key), "Missing expected key: {}", key);
        }
        assert_eq!(obj.len(), 5, "Unexpected extra fields in ApprovalRequest");
        // Must NOT have camelCase
        assert!(!obj.contains_key("approvalId"));
        assert!(!obj.contains_key("toolName"));
    }

    #[test]
    fn test_messages_for_llm_prepends_compressed_summary() {
        let mut orch = test_orchestrator();
        let id = orch.create_session();
        let session = orch.sessions.get_mut(&id).unwrap();
        session.compressed_summary = Some("## Current state\nDNS issue isolated".to_string());
        session.messages.push(Message {
            role: "user".to_string(),
            content: MessageContent::Text("Newest message".to_string()),
        });

        let messages = orch.messages_for_llm(&id);
        assert_eq!(messages.len(), 2);
        let summary = match &messages[0].content {
            MessageContent::Text(text) => text,
            _ => panic!("expected summary text"),
        };
        assert!(summary.contains("Compressed session context"));
        assert!(summary.contains("DNS issue isolated"));
    }

    #[test]
    fn test_summarized_transcript_truncates_large_tool_output() {
        let large_output = "x".repeat(2_000);
        let transcript = summarized_transcript(
            &[Message {
                role: "user".to_string(),
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "tool-1".to_string(),
                    content: large_output,
                    is_error: None,
                }]),
            }],
            10_000,
        );

        assert!(transcript.contains("Tool result:"));
        assert!(transcript.len() < 1_700);
        assert!(transcript.ends_with("..."));
    }

    #[test]
    fn test_estimate_messages_tokens_grows_with_content() {
        let short = vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text("short".to_string()),
        }];
        let long = vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text("x".repeat(4_000)),
        }];

        assert!(estimate_messages_tokens(&long) > estimate_messages_tokens(&short));
    }

    #[test]
    fn test_fallback_context_summary_keeps_existing_and_excerpt() {
        let summary = fallback_context_summary(
            Some("## Current state\nDNS issue isolated"),
            "User: collected router logs\nAssistant: asked for modem reboot",
        );

        assert!(summary.contains("DNS issue isolated"));
        assert!(summary.contains("Older session history was trimmed"));
        assert!(summary.contains("collected router logs"));
    }

    #[tokio::test]
    async fn test_force_compression_falls_back_when_summary_request_fails() {
        let llm = LlmClient::with_auth(AuthMode::Proxy {
            base_url: "http://127.0.0.1:9".to_string(),
            token: "test-token".to_string(),
        });
        let mut orch = test_orchestrator_with_llm(llm);
        let id = orch.create_session();
        let session = orch.sessions.get_mut(&id).unwrap();
        for idx in 0..8 {
            session.messages.push(Message {
                role: if idx % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: MessageContent::Text(format!("message-{idx}: {}", "x".repeat(40_000))),
            });
        }

        let result = orch.compress_session_context_if_needed(&id, true).await;
        assert!(result.is_ok());

        let session = orch.sessions.get(&id).unwrap();
        assert!(session.compressed_summary.is_some());
        assert!(session.messages.len() <= 4);
        assert!(session
            .compressed_summary
            .as_deref()
            .unwrap_or_default()
            .contains("Older session history was trimmed"));
    }

    #[tokio::test]
    async fn test_restore_session_reuses_compressed_summary_and_recent_messages() {
        let mut orch = test_orchestrator();
        let session_id = "session-1";
        {
            let conn = orch.db.lock().await;
            crate::safety::journal::create_session_record(
                &conn,
                session_id,
                "2026-03-12T00:00:00Z",
            )
            .unwrap();
            crate::safety::journal::update_session_compressed_summary(
                &conn,
                session_id,
                Some("## Current state\nPreserved diagnosis"),
            )
            .unwrap();
            for idx in 0..10 {
                crate::safety::journal::save_message(
                    &conn,
                    session_id,
                    if idx % 2 == 0 { "user" } else { "assistant" },
                    &format!("message-{idx}"),
                )
                .unwrap();
            }
        }

        orch.restore_session_if_needed(session_id).await.unwrap();

        let session = orch.sessions.get(session_id).unwrap();
        assert_eq!(
            session.compressed_summary.as_deref(),
            Some("## Current state\nPreserved diagnosis")
        );
        assert_eq!(session.messages.len(), RECENT_MESSAGES_TO_KEEP);
        let restored: Vec<String> = session
            .messages
            .iter()
            .map(|message| match &message.content {
                MessageContent::Text(text) => text.clone(),
                _ => panic!("expected restored text messages"),
            })
            .collect();
        assert_eq!(
            restored,
            vec![
                "message-4".to_string(),
                "message-5".to_string(),
                "message-6".to_string(),
                "message-7".to_string(),
                "message-8".to_string(),
                "message-9".to_string()
            ]
        );
    }
}
