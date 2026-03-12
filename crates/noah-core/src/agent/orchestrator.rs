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
    ContentBlock, LlmClient, Message, MessageContent, ResponseBlock,
};
use crate::agent::prompts;
use crate::agent::tool_router::ToolRouter;
use crate::events::EventEmitter;
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

/// Session state kept in memory.
pub struct Session {
    pub id: String,
    pub messages: Vec<Message>,
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

    // ── Agentic loop ───────────────────────────────────────────────────

    /// Send a user message and run the agentic loop until a text response
    /// is produced. The `emitter` is used to emit debug/approval events
    /// and the `db` connection is used to record changes in the journal.
    pub async fn send_message(
        &mut self,
        session_id: &str,
        user_message: &str,
        emitter: &dyn EventEmitter,
        db: &tokio::sync::Mutex<rusqlite::Connection>,
    ) -> Result<String> {
        // Verify session exists, or restore it from the database.
        if !self.sessions.contains_key(session_id) {
            // Try to restore the session from the database.
            let conn = db.lock().await;

            // Get session metadata
            let session_record = journal::get_session(&conn, session_id)
                .context("Failed to load session metadata from database")?;

            if session_record.is_none() {
                drop(conn);
                anyhow::bail!("Session not found: {}", session_id);
            }

            let session_record = session_record.unwrap();

            // Load messages
            let messages = journal::get_messages(&conn, session_id)
                .context("Failed to load session messages from database")?;
            drop(conn);

            // Restore the session to memory.
            let restored_messages: Vec<Message> = messages
                .iter()
                .map(|record| Message {
                    role: record.role.clone(),
                    content: MessageContent::Text(record.content.clone()),
                })
                .collect();

            // Parse created_at timestamp
            let created_at = chrono::DateTime::parse_from_rfc3339(&session_record.created_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            let session = Session {
                id: session_id.to_string(),
                messages: restored_messages,
                created_at,
                playbook: None,
                secrets: HashMap::new(),
                locale: None,
                mode: "default".to_string(),
            };
            self.sessions.insert(session_id.to_string(), session);
        }

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
        let system = prompts::system_prompt_blocks(&self.os_context, &knowledge_ctx, locale.as_deref(), &mode);
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

            // Clone messages for the LLM call to avoid borrow issues.
            let messages = self.sessions[session_id].messages.clone();

            emitter.emit_debug(
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

            let response = self
                .llm
                .send_message(messages.clone(), tool_defs.clone(), system.clone())
                .await?;

            // Save LLM trace for debugging.
            {
                let request_json = serde_json::to_string(&json!({
                    "message_count": messages.len(),
                    "tool_count": tool_defs.len(),
                }))
                .unwrap_or_default();
                let response_json =
                    serde_json::to_string(&response).unwrap_or_default();
                let conn = self.db.lock().await;
                if let Err(e) = journal::save_llm_trace(&conn, session_id, &request_json, &response_json) {
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

            emitter.emit_debug(
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
                    .filter(|(_, name, _)| matches!(name.as_str(), "ui_spa" | "ui_user_question" | "ui_info" | "ui_done"))
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
                        if let Some(fallback) = handle_ui_protocol_error("mixed/multiple ui_* calls".to_string()) {
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

                emitter.emit_debug(
                    "tool_call",
                    &format!("Calling {} [{}]", tool_name, tier_label),
                    json!({
                        "name": tool_name,
                        "input": tool_input,
                        "safety_tier": tier_label,
                    }),
                );

                let result = self
                    .execute_tool(session_id, &tool_name, &tool_input, emitter, db)
                    .await;

                match result {
                    Ok(ref output) => {
                        // Detect activate_playbook and set up runtime state.
                        if tool_name == "activate_playbook" {
                            if let Some(pb_name) = tool_input.get("name").and_then(|v| v.as_str()) {
                                let state = PlaybookState::from_content(pb_name, output);
                                if !state.steps.is_empty() {
                                    emitter.emit_debug(
                                        "playbook_activated",
                                        &format!("Playbook '{}' activated with {} steps", pb_name, state.total_steps),
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

                        emitter.emit_debug(
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
                        emitter.emit_debug(
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

    /// Execute a single tool call, handling safety tier checks and approvals.
    async fn execute_tool(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: &Value,
        emitter: &dyn EventEmitter,
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
                .request_approval(emitter, tool_name, tool.description(), tool_input, &reason)
                .await?;

            if !approved {
                emitter.emit_debug(
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

    /// Emit an approval-request event and wait for the response.
    async fn request_approval(
        &self,
        emitter: &dyn EventEmitter,
        tool_name: &str,
        description: &str,
        parameters: &Value,
        reason: &str,
    ) -> Result<bool> {
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

        emitter
            .emit_approval_request(&request)
            .context("Failed to emit approval-request event")?;

        // Wait for the frontend to respond, with a 5-minute timeout.
        let approved = match tokio::time::timeout(
            std::time::Duration::from_secs(300),
            rx,
        )
        .await
        {
            Ok(result) => result.unwrap_or(false),
            Err(_) => {
                // Timeout expired — clean up the pending approval and auto-deny.
                {
                    let mut pending = self.pending_approvals.lock().await;
                    pending.remove(&approval_id);
                }

                emitter.emit_debug(
                    "approval_timeout",
                    &format!("Approval for {} timed out after 5 minutes", tool_name),
                    json!({
                        "approval_id": approval_id,
                        "tool_name": tool_name,
                    }),
                );

                emitter.emit_approval_timeout(&approval_id);

                false
            }
        };

        Ok(approved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::llm_client::LlmClient;
    use crate::agent::tool_router::ToolRouter;

    fn test_orchestrator() -> Orchestrator {
        let llm = LlmClient::new(String::new());
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
        for key in ["approval_id", "tool_name", "description", "parameters", "reason"] {
            assert!(obj.contains_key(key), "Missing expected key: {}", key);
        }
        assert_eq!(obj.len(), 5, "Unexpected extra fields in ApprovalRequest");
        // Must NOT have camelCase
        assert!(!obj.contains_key("approvalId"));
        assert!(!obj.contains_key("toolName"));
    }
}
