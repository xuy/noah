use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use tokio::sync::Mutex;

use crate::agent::llm_client::LlmClient;
use crate::agent::orchestrator::{Orchestrator, PendingApprovals};
use crate::agent::tool_router::ToolRouter;
use crate::knowledge;
use crate::machine_context::MachineContext;
use crate::platform;
use crate::playbooks;
use crate::safety::journal;

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
        Ok(PathBuf::from(home).join("Library/Application Support/com.itman.app"))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(PathBuf::from(home).join(".local/share/com.itman.app"))
    }
    #[cfg(target_os = "windows")]
    {
        Ok(PathBuf::from(home).join("AppData/Roaming/com.itman.app"))
    }
}

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

pub async fn run_prompt_flow(prompt: &str, max_turns: usize) -> Result<PromptRunResult> {
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
    platform::register_platform_tools(&mut router, Some(&db_path));
    router.register(Box::new(knowledge::WriteKnowledgeTool::new(knowledge_dir.clone())));
    router.register(Box::new(knowledge::SearchKnowledgeTool::new(knowledge_dir.clone())));
    router.register(Box::new(knowledge::ReadKnowledgeTool::new(knowledge_dir.clone())));
    router.register(Box::new(knowledge::ListKnowledgeTool::new(knowledge_dir.clone())));

    let playbook_registry = playbooks::PlaybookRegistry::init(&knowledge_dir)?;
    router.register(Box::new(playbooks::ActivatePlaybookTool::new(playbook_registry)));

    let auth = super::load_auth(&app_dir);
    let llm = LlmClient::with_auth(auth);
    if !llm.has_auth() {
        return Err(anyhow!(
            "No auth configured. Set proxy/api key in app config or ANTHROPIC_API_KEY."
        ));
    }

    let pending_approvals: PendingApprovals =
        Arc::new(Mutex::new(HashMap::<String, tokio::sync::oneshot::Sender<bool>>::new()));
    let mut orchestrator = Orchestrator::new(
        llm,
        router,
        MachineContext::load_or_gather(&app_dir).to_prompt_string(),
        pending_approvals.clone(),
        db_arc.clone(),
        knowledge_dir,
    );
    let session_id = orchestrator.create_session();
    {
        let conn = db_arc.lock().await;
        journal::create_session_record(&conn, &session_id, &chrono::Utc::now().to_rfc3339())?;
    }

    let app = tauri::test::mock_app();
    let app_handle = app.handle().clone();
    let approver = spawn_auto_approver(pending_approvals).await;

    let mut turns = Vec::new();
    let mut input = prompt.to_string();
    let mut reached_done = false;

    for _ in 0..max_turns {
        {
            let conn = db_arc.lock().await;
            journal::save_message(&conn, &session_id, "user", &input)?;
        }

        let output = orchestrator
            .send_message(&session_id, &input, &app_handle, &db_arc)
            .await
            .context("orchestrator send_message failed")?;
        {
            let conn = db_arc.lock().await;
            journal::save_message(&conn, &session_id, "assistant", &output)?;
            if let Some(session) = orchestrator.get_session(&session_id) {
                let count = session.messages.len() as i32;
                journal::update_session_message_count(&conn, &session_id, count)?;
            }
        }
        turns.push((input.clone(), output.clone()));

        if output.contains("[DONE]") {
            reached_done = true;
            break;
        }

        if output.contains("[ACTION:") {
            input = "Go ahead".to_string();
            continue;
        }

        break;
    }

    approver.abort();

    Ok(PromptRunResult {
        session_id,
        turns,
        reached_done,
    })
}
