//! Headless debug runner for end-to-end backend testing.
//!
//! Instantiates a real Orchestrator with a StderrEventEmitter (no Tauri needed),
//! sends prompts through the full agentic loop, and returns structured results.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use tokio::sync::Mutex;

use crate::agent::llm_client::LlmClient;
use crate::agent::orchestrator::{Orchestrator, PendingApprovals};
use crate::agent::tool_router::ToolRouter;
use crate::config;
use crate::events::StderrEventEmitter;
use crate::knowledge;
use crate::machine_context::MachineContext;
use crate::playbooks;
use crate::safety::journal;
use crate::ui_parsing::{
    parse_assistant_ui, AssistantActionType, AssistantUiPayload,
};
use crate::ui_tools;

#[derive(Debug, Clone)]
pub struct PromptRunResult {
    pub session_id: String,
    pub turns: Vec<(String, String)>,
    pub reached_done: bool,
}

fn default_app_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("NOAH_APP_DIR") {
        return Ok(PathBuf::from(dir));
    }

    let home = std::env::var("HOME").context("HOME is not set")?;
    #[cfg(target_os = "macos")]
    {
        Ok(PathBuf::from(home).join("Library/Application Support/app.onnoah.desktop"))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(PathBuf::from(home).join(".local/share/app.onnoah.desktop"))
    }
    #[cfg(target_os = "windows")]
    {
        Ok(PathBuf::from(home).join("AppData/Roaming/app.onnoah.desktop"))
    }
}

/// Load pre-configured secrets from NOAH_SECRETS env var.
/// Format: JSON object mapping secret_name → value.
/// Example: NOAH_SECRETS='{"telegram_bot_token":"123:ABC","api_key":"sk-..."}'
fn load_preset_secrets() -> HashMap<String, String> {
    match std::env::var("NOAH_SECRETS") {
        Ok(json_str) => {
            serde_json::from_str::<HashMap<String, String>>(&json_str).unwrap_or_else(|e| {
                eprintln!("[debug_runner] Warning: failed to parse NOAH_SECRETS: {}", e);
                HashMap::new()
            })
        }
        Err(_) => HashMap::new(),
    }
}

/// Load pre-configured text answers from NOAH_ANSWERS env var.
/// Format: JSON object mapping lowercased keyword → answer.
/// Example: NOAH_ANSWERS='{"telegram":"Telegram","model":"GLM-4-Flash"}'
fn load_preset_answers() -> HashMap<String, String> {
    match std::env::var("NOAH_ANSWERS") {
        Ok(json_str) => {
            serde_json::from_str::<HashMap<String, String>>(&json_str).unwrap_or_else(|e| {
                eprintln!("[debug_runner] Warning: failed to parse NOAH_ANSWERS: {}", e);
                HashMap::new()
            })
        }
        Err(_) => HashMap::new(),
    }
}

/// Auto-approves all pending approval requests (for headless testing).
async fn spawn_auto_approver(pending: PendingApprovals) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let senders = {
                let mut map = pending.lock().await;
                if map.is_empty() {
                    Vec::new()
                } else {
                    map.drain().map(|(_, sender)| sender).collect::<Vec<_>>()
                }
            };

            for sender in senders {
                let _ = sender.send(true);
            }

            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
    })
}

/// Run a prompt through the full orchestrator pipeline and return results.
///
/// This creates a real Orchestrator with all tools registered, sends the prompt,
/// and auto-confirms any actions. Useful for end-to-end testing of the UI tool flow.
///
/// `register_platform_tools` is a callback that registers OS-specific tools.
/// Pass `None` to skip platform tools (core-only testing).
pub async fn run_prompt_flow(
    prompt: &str,
    max_turns: usize,
    register_platform_tools: Option<&dyn Fn(&mut ToolRouter, Option<&std::path::Path>)>,
    bundled_playbooks_dir: Option<&std::path::Path>,
) -> Result<PromptRunResult> {
    let app_dir = default_app_dir()?;
    std::fs::create_dir_all(&app_dir).context("Failed to create app dir")?;

    let db_path = app_dir.join("journal.db");
    let db = journal::init_db(
        db_path
            .to_str()
            .ok_or_else(|| anyhow!("Invalid DB path"))?,
    )?;
    let db_arc = Arc::new(Mutex::new(db));

    let knowledge_dir = knowledge::init_knowledge_dir(&app_dir)?;
    {
        let conn = db_arc.lock().await;
        journal::run_file_migrations(&conn, &knowledge_dir)?;
    }

    let mut router = ToolRouter::new();
    if let Some(register_fn) = register_platform_tools {
        register_fn(&mut router, Some(&db_path));
    }
    ui_tools::register_ui_tools(&mut router);
    router.register(Box::new(knowledge::WriteKnowledgeTool::new(
        knowledge_dir.clone(),
    )));
    router.register(Box::new(knowledge::KnowledgeSearchTool::new(
        knowledge_dir.clone(),
    )));
    router.register(Box::new(knowledge::KnowledgeReadTool::new(
        knowledge_dir.clone(),
    )));

    router.register(Box::new(crate::web_fetch::WebFetchTool));

    // Playbooks: use provided dir or skip.
    if let Some(pb_dir) = bundled_playbooks_dir {
        let playbook_registry = playbooks::PlaybookRegistry::init(&knowledge_dir, pb_dir)?;
        router.register(Box::new(playbooks::ActivatePlaybookTool::new(
            playbook_registry,
        )));
    }

    let auth = config::load_auth(&app_dir);
    let llm = LlmClient::with_auth(auth);
    if !llm.has_auth() {
        return Err(anyhow!(
            "No auth configured. Set proxy/api key in app config or ANTHROPIC_API_KEY."
        ));
    }

    let pending_approvals: PendingApprovals =
        Arc::new(Mutex::new(HashMap::<String, tokio::sync::oneshot::Sender<bool>>::new()));
    // Allow overriding the OS context for testing (e.g. emulate Linux on macOS).
    let os_context = match std::env::var("NOAH_PLATFORM") {
        Ok(plat) => format!("Platform: {}\nHostname: test-machine", plat),
        Err(_) => MachineContext::load_or_gather(&app_dir).to_prompt_string(),
    };
    let mut orchestrator = Orchestrator::new(
        llm,
        router,
        os_context,
        pending_approvals.clone(),
        db_arc.clone(),
        knowledge_dir,
    );
    let session_id = orchestrator.create_session();

    // Support NOAH_MODE=learn to enable knowledge-creation system prompt.
    if let Ok(mode) = std::env::var("NOAH_MODE") {
        if mode == "learn" {
            orchestrator.set_mode(&session_id, "learn");
        }
    }
    {
        let conn = db_arc.lock().await;
        journal::create_session_record(&conn, &session_id, &chrono::Utc::now().to_rfc3339())?;
    }

    let emitter = StderrEventEmitter;
    let approver = spawn_auto_approver(pending_approvals).await;

    let preset_secrets = load_preset_secrets();
    let preset_answers = load_preset_answers();

    let mut turns = Vec::new();
    let mut input = prompt.to_string();
    let mut reached_done = false;

    for _ in 0..max_turns {
        {
            let conn = db_arc.lock().await;
            journal::save_message(&conn, &session_id, "user", &input)?;
        }

        let output = match tokio::time::timeout(
            std::time::Duration::from_secs(600),
            orchestrator.send_message(&session_id, &input, &emitter, &db_arc),
        )
        .await
        {
            Ok(res) => res.context("orchestrator send_message failed")?,
            Err(_) => r#"{"kind":"info","summary":"Runner timeout waiting for assistant response."}"#.to_string(),
        };
        {
            let conn = db_arc.lock().await;
            journal::save_message(&conn, &session_id, "assistant", &output)?;
            if let Some(session) = orchestrator.get_session(&session_id) {
                let count = session.messages.len() as i32;
                journal::update_session_message_count(&conn, &session_id, count)?;
            }
        }
        turns.push((input.clone(), output.clone()));

        // Check for done
        match parse_assistant_ui(&output) {
            Some(AssistantUiPayload::Done(_)) => {
                reached_done = true;
                break;
            }
            Some(AssistantUiPayload::Spa(ref spa)) => {
                // WAIT_FOR_USER: simulate user completing external action.
                if spa.action.action_type == AssistantActionType::WaitForUser {
                    input = "I've done this, let's continue.".to_string();
                } else {
                    input = "Go ahead".to_string();
                }
                continue;
            }
            Some(AssistantUiPayload::UserQuestion(ref uq)) => {
                // Generate a reasonable auto-answer based on question type.
                let q = &uq.questions[0];
                if q.secure_input.is_some() {
                    // For secure inputs, use preset value if available, else dummy.
                    let secret_name = q.secure_input.as_ref().unwrap().secret_name.clone();
                    let value = preset_secrets
                        .get(&secret_name)
                        .cloned()
                        .unwrap_or_else(|| "test-secret-value-12345".to_string());
                    let is_preset = preset_secrets.contains_key(&secret_name);
                    orchestrator.store_secret(&session_id, &secret_name, &value);
                    input = format!(
                        "[SECRET:{}] stored securely{}",
                        secret_name,
                        if is_preset { " (preset)" } else { " (dummy)" }
                    );
                } else if q.text_input.is_some() {
                    // Check preset answers first: match any keyword in header/question.
                    let header = q.header.to_lowercase();
                    let question = q.question.to_lowercase();
                    let combined = format!("{} {}", header, question);

                    let answer = preset_answers
                        .iter()
                        .find(|(keyword, _)| combined.contains(keyword.as_str()))
                        .map(|(_, answer)| answer.clone())
                        .unwrap_or_else(|| {
                            // Fall back to heuristic auto-answers.
                            if header.contains("email") || question.contains("email") {
                                "testuser@example.com".to_string()
                            } else if header.contains("ssid") || question.contains("ssid") || question.contains("wi-fi") || question.contains("wifi") || question.contains("network name") {
                                "TestNetwork".to_string()
                            } else if header.contains("server") || question.contains("server") {
                                "mail.example.com".to_string()
                            } else if header.contains("drive") || question.contains("drive") || question.contains("connect") {
                                "My Backup Drive, 1TB, appears in Finder".to_string()
                            } else if header.contains("username") || question.contains("username") {
                                "testuser".to_string()
                            } else if header.contains("path") || question.contains("path") || question.contains("folder") {
                                "/Users/test/Documents".to_string()
                            } else {
                                "test-input-value".to_string()
                            }
                        });
                    input = answer;
                } else if let Some(ref opts) = q.options {
                    // Pick the first option.
                    if let Some(first) = opts.first() {
                        input = first.label.clone();
                    } else {
                        input = "Yes".to_string();
                    }
                } else {
                    input = "Yes".to_string();
                }
                continue;
            }
            Some(AssistantUiPayload::Info(_)) => {
                // Info is terminal-ish, break
                break;
            }
            None => {
                // No UI payload — plain text response. Continue with a generic followup.
                input = "Continue. Please use the structured UI tools (ui_spa, ui_user_question, ui_info, or ui_done) to present your response.".to_string();
                continue;
            }
        }
    }

    approver.abort();

    Ok(PromptRunResult {
        session_id,
        turns,
        reached_done,
    })
}
