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

#[derive(Debug, Clone)]
struct DebugOpenclawCredentialCapture {
    credential_ref: String,
    provider: String,
    chat_channel: Option<String>,
}

fn openclaw_config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".openclaw/openclaw.json"))
}

fn first_non_empty_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
}

async fn maybe_simulate_openclaw_secure_capture(
    output: &str,
    db: &Arc<Mutex<rusqlite::Connection>>,
) -> Option<DebugOpenclawCredentialCapture> {
    let lower = output.to_lowercase();
    let looks_like_openclaw_action = lower.contains("[action:")
        && lower.contains("openclaw")
        && (lower.contains("secure")
            || lower.contains("credential")
            || lower.contains("api key")
            || lower.contains("token"));
    if !looks_like_openclaw_action {
        return None;
    }

    let provider = if lower.contains("anthropic") {
        "Anthropic".to_string()
    } else if lower.contains("openrouter") {
        "OpenRouter".to_string()
    } else if lower.contains("gemini") || lower.contains("google") {
        "Google Gemini".to_string()
    } else {
        "OpenAI".to_string()
    };

    let chat_channel = if lower.contains("telegram") {
        Some("Telegram".to_string())
    } else if lower.contains("discord") {
        Some("Discord".to_string())
    } else {
        None
    };

    let provider_token = first_non_empty_env(&[
        "NOAH_DEBUG_OPENCLAW_PROVIDER_TOKEN",
        "NOAH_DEBUG_OPENCLAW_ANTHROPIC_TOKEN",
        "NOAH_DEBUG_OPENCLAW_OPENAI_TOKEN",
    ])
    .unwrap_or_else(|| format!("debug-{}", uuid::Uuid::new_v4()));
    let chat_token = chat_channel.as_ref().map(|ch| {
        let env_token = match ch.to_lowercase().as_str() {
            "telegram" => first_non_empty_env(&[
                "NOAH_DEBUG_OPENCLAW_CHAT_TOKEN",
                "NOAH_DEBUG_OPENCLAW_TELEGRAM_TOKEN",
            ]),
            "discord" => first_non_empty_env(&[
                "NOAH_DEBUG_OPENCLAW_CHAT_TOKEN",
                "NOAH_DEBUG_OPENCLAW_DISCORD_TOKEN",
            ]),
            _ => first_non_empty_env(&["NOAH_DEBUG_OPENCLAW_CHAT_TOKEN"]),
        };
        env_token.unwrap_or_else(|| format!("debug-{}-{}", ch.to_lowercase(), uuid::Uuid::new_v4()))
    });

    let path = match openclaw_config_path() {
        Ok(p) => p,
        Err(_) => return None,
    };
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return None;
        }
    }

    let mut root = if path.exists() {
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        {
            Some(v) if v.is_object() => v,
            _ => serde_json::json!({}),
        }
    } else {
        serde_json::json!({})
    };

    let Some(obj) = root.as_object_mut() else {
        return None;
    };
    obj.insert(
        "model_provider".to_string(),
        serde_json::json!({
            "name": provider,
            "token": provider_token,
        }),
    );
    if let (Some(ch), Some(tok)) = (chat_channel.clone(), chat_token) {
        obj.insert(
            "chat_integration".to_string(),
            serde_json::json!({
                "channel": ch,
                "token": tok,
            }),
        );
    }
    let rendered = match serde_json::to_string_pretty(&root) {
        Ok(v) => v,
        Err(_) => return None,
    };
    if std::fs::write(&path, rendered).is_err() {
        return None;
    }

    let credential_ref = format!("openclaw-{}", uuid::Uuid::new_v4());
    let saved_at = chrono::Utc::now().to_rfc3339();
    let profile = serde_json::json!({
        "credential_ref": credential_ref,
        "provider": provider,
        "chat_channel": chat_channel,
        "saved_at": saved_at,
    });
    let profile_str = match serde_json::to_string(&profile) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let conn = db.lock().await;
    let write_ok = journal::set_setting(&conn, "openclaw_last_profile", &profile_str).is_ok();
    if !write_ok {
        return None;
    }

    Some(DebugOpenclawCredentialCapture {
        credential_ref,
        provider,
        chat_channel,
    })
}

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
            if let Some(saved) = maybe_simulate_openclaw_secure_capture(&output, &db_arc).await {
                let channel = saved.chat_channel.unwrap_or_else(|| "none".to_string());
                input = format!(
                    "OpenClaw credentials were submitted via Noah secure form. Credential reference: {}. Provider: {}. Chat channel: {}. Please continue with validation and next setup checkpoint.",
                    saved.credential_ref, saved.provider, channel
                );
            } else {
                input = "Go ahead".to_string();
            }
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
