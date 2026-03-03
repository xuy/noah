mod agent;
mod artifacts;
mod commands;
mod machine_context;
mod platform;
mod safety;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::{oneshot, Mutex};

use agent::llm_client::LlmClient;
use agent::orchestrator::{Orchestrator, PendingApprovals};
use agent::tool_router::ToolRouter;
use safety::journal;

/// Shared application state managed by Tauri.
pub struct AppState {
    pub orchestrator: Mutex<Orchestrator>,
    pub pending_approvals: PendingApprovals,
    pub db: Arc<Mutex<rusqlite::Connection>>,
    /// Path to the app data directory (for saving config).
    pub app_dir: PathBuf,
    /// Cancellation flag — can be set without holding the orchestrator lock.
    pub cancelled: Arc<AtomicBool>,
}

/// Load the API key: config file first, then env var.
fn load_api_key(app_dir: &std::path::Path) -> String {
    // Try config file first
    let key_path = app_dir.join("api_key.txt");
    if let Ok(contents) = std::fs::read_to_string(&key_path) {
        let key = contents.trim().to_string();
        if !key.is_empty() {
            return key;
        }
    }
    // Fall back to environment variable
    std::env::var("ANTHROPIC_API_KEY").unwrap_or_default()
}

/// Save API key to config file.
pub fn save_api_key(app_dir: &std::path::Path, key: &str) -> Result<(), String> {
    let key_path = app_dir.join("api_key.txt");
    std::fs::write(&key_path, key).map_err(|e| format!("Failed to save API key: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Initialise the journal database.
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data directory");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");

            let db_path = app_dir.join("journal.db");
            let db_path_str = db_path.to_str().expect("Invalid database path");

            // Try to open the database; if corrupted, back it up and start fresh.
            let db = match journal::init_db(db_path_str) {
                Ok(db) => db,
                Err(_) => {
                    // Rename corrupted DB to .bak
                    let bak_path = app_dir.join("journal.db.bak");
                    let _ = std::fs::rename(&db_path, &bak_path);
                    eprintln!(
                        "Warning: journal.db was corrupted. Backed up to journal.db.bak and starting fresh."
                    );
                    journal::init_db(db_path_str)
                        .expect("Failed to create fresh journal database")
                }
            };

            // Wrap DB in Arc<Mutex<>> early so tools can share it.
            let db_arc = Arc::new(Mutex::new(db));

            // Build the tool router and register platform tools.
            let mut router = ToolRouter::new();
            platform::register_platform_tools(&mut router);

            // Register knowledge artifact tools.
            router.register(Box::new(artifacts::SaveArtifactTool::new(db_arc.clone())));
            router.register(Box::new(artifacts::QueryArtifactsTool::new(db_arc.clone())));

            // Load API key: config file first, then env var.
            let api_key = load_api_key(&app_dir);
            let llm = LlmClient::new(api_key);

            let pending_approvals: PendingApprovals =
                Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<bool>>::new()));

            // Gather OS context for the system prompt.
            let os_context = machine_context::MachineContext::load_or_gather(&app_dir)
                .to_prompt_string();

            // Build the orchestrator.
            let orchestrator =
                Orchestrator::new(llm, router, os_context, pending_approvals.clone(), db_arc.clone());
            let cancelled = orchestrator.cancelled_flag();

            // Manage shared state.
            app.manage(AppState {
                orchestrator: Mutex::new(orchestrator),
                pending_approvals,
                db: db_arc,
                app_dir,
                cancelled,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::session::create_session,
            commands::session::get_session,
            commands::session::end_session,
            commands::session::delete_session,
            commands::session::list_sessions,
            commands::session::get_session_messages,
            commands::session::get_session_summary,
            commands::session::export_session,
            commands::session::mark_resolved,
            commands::agent::send_message,
            commands::agent::approve_action,
            commands::agent::deny_action,
            commands::agent::cancel_processing,
            commands::safety::get_changes,
            commands::safety::undo_change,
            commands::settings::has_api_key,
            commands::settings::set_api_key,
            commands::settings::get_app_version,
            commands::settings::get_telemetry_consent,
            commands::settings::set_telemetry_consent,
            commands::settings::track_event,
            commands::settings::get_feedback_context,
            commands::artifacts::list_artifacts,
            commands::artifacts::delete_artifact,
            commands::artifacts::get_contextual_suggestions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
