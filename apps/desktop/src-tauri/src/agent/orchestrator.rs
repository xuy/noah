use std::collections::HashMap;
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
}

pub type PendingApprovals = Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>;

impl Orchestrator {
    pub fn new(
        llm: LlmClient,
        router: ToolRouter,
        os_context: String,
        pending_approvals: PendingApprovals,
    ) -> Self {
        Self {
            llm,
            router,
            sessions: HashMap::new(),
            pending_approvals,
            os_context,
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

    pub fn has_api_key(&self) -> bool {
        self.llm.has_api_key()
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

        let system = prompts::system_prompt(&self.os_context);
        let tool_defs = self.router.tool_definitions();

        // Agentic loop: keep calling the LLM until we get a text-only response.
        loop {
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
                .send_message(messages, tool_defs.clone(), &system)
                .await?;

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
                    "Response: {} text blocks, {} tool calls",
                    text_parts.len(),
                    tool_uses.len()
                ),
                json!({
                    "stop_reason": response.stop_reason,
                    "text_blocks": text_parts.len(),
                    "tool_calls": tool_uses.len(),
                    "usage": {
                        "input_tokens": response.usage.input_tokens,
                        "output_tokens": response.usage.output_tokens,
                    },
                }),
            );

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

            // If no tool calls, we're done — return the text.
            if tool_uses.is_empty() {
                return Ok(text_parts.join("\n"));
            }

            // Execute each tool call.
            let mut tool_result_blocks: Vec<ContentBlock> = Vec::new();

            for (tool_use_id, tool_name, tool_input) in tool_uses {
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
                let _ = journal::record_change(&conn, session_id, tool_name, change);
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

        // Wait for the frontend to respond.
        let approved = rx.await.unwrap_or(false);

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
        Orchestrator::new(
            llm,
            router,
            "test context".to_string(),
            Arc::new(Mutex::new(HashMap::new())),
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
