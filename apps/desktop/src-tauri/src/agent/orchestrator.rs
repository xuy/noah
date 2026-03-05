use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use itman_tools::SafetyTier;

use crate::agent::llm_client::{
    ContentBlock, LlmClient, Message, MessageContent, ResponseBlock,
};
use crate::agent::prompts;
use crate::agent::tool_router::ToolRouter;
use crate::knowledge;
use crate::safety::journal;

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

fn emit_debug(app_handle: &tauri::AppHandle, event_type: &str, summary: &str, detail: Value) {
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
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct Orchestrator {
    llm: LlmClient,
    router: ToolRouter,
    sessions: HashMap<String, Session>,
    /// Pending approval channels: approval_id -> oneshot sender (true = approved).
    pending_approvals: PendingApprovals,
    os_context: String,
    db: Arc<Mutex<rusqlite::Connection>>,
    /// Path to the knowledge directory for building TOC.
    knowledge_dir: std::path::PathBuf,
    /// Compact playbook listing for the system prompt.
    playbooks_section: String,
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
        playbooks_section: String,
    ) -> Self {
        Self {
            llm,
            router,
            sessions: HashMap::new(),
            pending_approvals,
            os_context,
            db,
            knowledge_dir,
            playbooks_section,
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
        };
        self.sessions.insert(id.clone(), session);
        id
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
    /// is produced. The `app_handle` is used to emit approval-request events
    /// and the `db` connection is used to record changes in the journal.
    pub async fn send_message(
        &mut self,
        session_id: &str,
        user_message: &str,
        app_handle: &tauri::AppHandle,
        db: &tokio::sync::Mutex<rusqlite::Connection>,
    ) -> Result<String> {
        // Verify session exists.
        if !self.sessions.contains_key(session_id) {
            anyhow::bail!("Session not found: {}", session_id);
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
        let system = prompts::system_prompt(&self.os_context, &knowledge_ctx, &self.playbooks_section);
        let tool_defs = self.router.tool_definitions();

        // Reset cancellation flag at the start of each user message.
        self.cancelled.store(false, Ordering::SeqCst);

        // Accumulate text across all loop iterations so we don't lose text
        // from turns where the LLM returns both text AND tool calls.
        let mut all_text_parts: Vec<String> = Vec::new();

        // Agentic loop: keep calling the LLM until we get a text-only response.
        loop {
            // Check for cancellation before each iteration.
            if self.cancelled.load(Ordering::SeqCst) {
                all_text_parts.push("[INFO] Stopped by user.".to_string());
                return Ok(all_text_parts.join("\n"));
            }

            // Clone messages for the LLM call to avoid borrow issues.
            let messages = self.sessions[session_id].messages.clone();

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

            let response = self
                .llm
                .send_message(messages.clone(), tool_defs.clone(), &system)
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

    /// Execute a single tool call, handling safety tier checks and approvals.
    async fn execute_tool(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: &Value,
        app_handle: &tauri::AppHandle,
        db: &tokio::sync::Mutex<rusqlite::Connection>,
    ) -> Result<String> {
        let tool = self
            .router
            .find_tool(tool_name)
            .context(format!("Unknown tool: {}", tool_name))?;

        let tier = tool.safety_tier_for_input(tool_input);

        // For NeedsApproval tools, request approval from the frontend.
        if tier == SafetyTier::NeedsApproval {
            let reason = tool_input
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let approved = self
                .request_approval(app_handle, tool_name, tool.description(), tool_input, &reason)
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
    async fn request_approval(
        &self,
        app_handle: &tauri::AppHandle,
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
                    let _ = app_handle.emit("approval-timeout", json!({
                        "approval_id": approval_id,
                    }));
                }

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
            String::new(),
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
