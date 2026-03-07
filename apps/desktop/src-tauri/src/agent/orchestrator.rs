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
use crate::playbook_runtime;
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
    /// Path to the knowledge directory for building the live TOC (includes `playbooks/`).
    knowledge_dir: std::path::PathBuf,
    /// Set to true to cancel the current agentic loop.
    cancelled: Arc<AtomicBool>,
}

pub type PendingApprovals = Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>;

fn active_playbook_name(messages: &[Message]) -> Option<String> {
    let mut active: Option<String> = None;
    for message in messages {
        let MessageContent::Blocks(blocks) = &message.content else {
            continue;
        };
        for block in blocks {
            if let ContentBlock::ToolUse { name, input, .. } = block {
                if name == "activate_playbook" {
                    active = input
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
        }
    }
    active
}

fn playbook_mode_overlay(active_playbook: Option<&str>) -> String {
    match active_playbook {
        Some("openclaw-install-config") => {
            "\n\n## Playbook Governance Mode\n\
Active playbook: `openclaw-install-config`.\n\
Treat this as a constrained sub-agent protocol for this session.\n\
- Use `[SITUATION]` + `[PLAN]` + `[ACTION:...]` for guided setup turns (including provider/channel selection checkpoints).\n\
- When collecting credentials, direct the user to Noah's secure credential form (privacy-preserving local capture), not plain chat token entry.\n\
- Do not claim a command/wizard ran unless a tool result explicitly confirms it.\n\
- If `shell_run` says a command was blocked or not executed, explicitly state that and switch to a supported path.\n\
- Do not hand off setup as \"configure via app UI\" and stop.\n\
- Stay in guided setup mode until completion criteria in the playbook are met.\n\
- For OpenClaw config, never run interactive wizard commands (`openclaw config` / `openclaw configure`) through `shell_run`."
                .to_string()
        }
        Some(name) => format!(
            "\n\n## Playbook Governance Mode\nActive playbook: `{}`.\nTreat this playbook as binding protocol until its completion criteria are met.",
            name
        ),
        None => String::new(),
    }
}

fn final_user_visible_segment(text: &str) -> String {
    let markers = ["[SITUATION]", "[DONE]", "[INFO]", "[CREDENTIALS_COLLECTED]"];
    let mut best_idx: Option<usize> = None;
    for marker in markers {
        if let Some(idx) = text.rfind(marker) {
            best_idx = Some(best_idx.map_or(idx, |cur| cur.max(idx)));
        }
    }
    match best_idx {
        Some(idx) => text[idx..].trim().to_string(),
        None => text.trim().to_string(),
    }
}


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
    pub async fn send_message<R: tauri::Runtime>(
        &mut self,
        session_id: &str,
        user_message: &str,
        app_handle: &tauri::AppHandle<R>,
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
        let base_system = prompts::system_prompt(&self.os_context, &knowledge_ctx);
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
                return Ok(final_user_visible_segment(&all_text_parts.join("\n")));
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

            let active_playbook = active_playbook_name(&messages);
            let openclaw_ctx = if active_playbook.as_deref() == Some("openclaw-install-config") {
                Some(playbook_runtime::parse_openclaw_context(&messages))
            } else {
                None
            };
            let system = format!(
                "{}{}{}",
                base_system,
                playbook_mode_overlay(active_playbook.as_deref()),
                openclaw_ctx
                    .as_ref()
                    .map(playbook_runtime::openclaw_stage_overlay)
                    .unwrap_or_default()
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
            let appended_text_count = text_parts.len();
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
                let candidate_text = all_text_parts.join("\n");
                let visible_text = final_user_visible_segment(&candidate_text);
                if active_playbook.as_deref() == Some("openclaw-install-config")
                    && playbook_runtime::has_disallowed_openclaw_text(&visible_text)
                {
                    for _ in 0..appended_text_count {
                        let _ = all_text_parts.pop();
                    }
                    let guard_feedback = "Policy guard: do not instruct `openclaw configure` or interactive OpenClaw wizard commands in this playbook mode. Provide a compliant guided setup response.".to_string();
                    {
                        let session = self.sessions.get_mut(session_id).unwrap();
                        session.messages.push(Message {
                            role: "user".to_string(),
                            content: MessageContent::Text(guard_feedback.clone()),
                        });
                    }
                    emit_debug(
                        app_handle,
                        "playbook_guard",
                        "Rejected non-compliant OpenClaw response and requested retry",
                        json!({"reason": "disallowed_openclaw_wizard_instruction"}),
                    );
                    continue;
                }
                if active_playbook.as_deref() == Some("openclaw-install-config")
                    && playbook_runtime::missing_openclaw_action_format(&visible_text)
                {
                    for _ in 0..appended_text_count {
                        let _ = all_text_parts.pop();
                    }
                    let guard_feedback = "Policy guard: OpenClaw setup responses must use [SITUATION], [PLAN], and [ACTION:...] until completion. Rewrite this response in the structured setup format.".to_string();
                    {
                        let session = self.sessions.get_mut(session_id).unwrap();
                        session.messages.push(Message {
                            role: "user".to_string(),
                            content: MessageContent::Text(guard_feedback),
                        });
                    }
                    continue;
                }
                if active_playbook.as_deref() == Some("openclaw-install-config")
                    && playbook_runtime::has_awkward_provider_shorthand(&visible_text)
                {
                    for _ in 0..appended_text_count {
                        let _ = all_text_parts.pop();
                    }
                    let guard_feedback = "Policy guard: provider guidance must use human-readable names (OpenAI, Anthropic, OpenRouter) and plain language, not code-like shorthand list formatting.".to_string();
                    {
                        let session = self.sessions.get_mut(session_id).unwrap();
                        session.messages.push(Message {
                            role: "user".to_string(),
                            content: MessageContent::Text(guard_feedback),
                        });
                    }
                    continue;
                }
                if active_playbook.as_deref() == Some("openclaw-install-config")
                    && openclaw_ctx.as_ref().is_some_and(|ctx| {
                        playbook_runtime::missing_provider_source_guidance(
                            user_message,
                            &visible_text,
                            ctx.provider.as_deref(),
                        )
                    })
                {
                    for _ in 0..appended_text_count {
                        let _ = all_text_parts.pop();
                    }
                    let guard_feedback = "Policy guard: user asked where to get API credentials. Provide concrete source guidance (provider console URL and plain steps) before proceeding.".to_string();
                    {
                        let session = self.sessions.get_mut(session_id).unwrap();
                        session.messages.push(Message {
                            role: "user".to_string(),
                            content: MessageContent::Text(guard_feedback),
                        });
                    }
                    continue;
                }
                if active_playbook.as_deref() == Some("openclaw-install-config")
                    && openclaw_ctx.as_ref().is_some_and(|ctx| {
                        ctx.stage == playbook_runtime::OpenclawStage::PrimaryProviderVerify
                            && ctx.credential_ref.is_some()
                            && playbook_runtime::has_vague_apply_credentials_loop(&visible_text)
                    })
                {
                    for _ in 0..appended_text_count {
                        let _ = all_text_parts.pop();
                    }
                    let guard_feedback = "Policy guard: avoid vague 'apply credentials' loops. Either verify directly now, or ask user to re-save a real key in Noah secure form with a concrete reason.".to_string();
                    {
                        let session = self.sessions.get_mut(session_id).unwrap();
                        session.messages.push(Message {
                            role: "user".to_string(),
                            content: MessageContent::Text(guard_feedback),
                        });
                    }
                    continue;
                }
                return Ok(visible_text);
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
    async fn execute_tool<R: tauri::Runtime>(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: &Value,
        app_handle: &tauri::AppHandle<R>,
        db: &tokio::sync::Mutex<rusqlite::Connection>,
    ) -> Result<String> {
        if tool_name == "shell_run" {
            if let Some(session) = self.sessions.get(session_id) {
                if active_playbook_name(&session.messages).as_deref() == Some("openclaw-install-config") {
                    let ctx = playbook_runtime::parse_openclaw_context(&session.messages);
                    let command = tool_input.get("command").and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(reason) = playbook_runtime::blocked_openclaw_shell_command(ctx.stage, command) {
                        return Ok(format!(
                            "COMMAND NOT EXECUTED: blocked by OpenClaw stage policy (stage={}, reason={}). Use stage-appropriate verification/capture steps instead.",
                            ctx.stage.as_str(),
                            reason
                        ));
                    }
                }
            }
        }

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
    fn test_active_playbook_name_detects_latest_activation() {
        let messages = vec![Message {
            role: "assistant".to_string(),
            content: MessageContent::Blocks(vec![
                ContentBlock::ToolUse {
                    id: "1".to_string(),
                    name: "activate_playbook".to_string(),
                    input: json!({"name":"network-diagnostics"}),
                },
                ContentBlock::ToolUse {
                    id: "2".to_string(),
                    name: "activate_playbook".to_string(),
                    input: json!({"name":"openclaw-install-config"}),
                },
            ]),
        }];
        assert_eq!(
            active_playbook_name(&messages).as_deref(),
            Some("openclaw-install-config")
        );
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
