mod agent;
mod commands;
mod platform;
mod safety;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
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

/// Gather OS context string to include in the system prompt.
fn gather_os_context() -> String {
    let sw_vers = Command::new("sw_vers")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown OS".to_string());

    let hostname = Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let cpu = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown CPU".to_string());

    let mem = Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            s.parse::<u64>()
                .map(|b| format!("{} GB", b / (1024 * 1024 * 1024)))
                .unwrap_or(s)
        })
        .unwrap_or_else(|_| "Unknown".to_string());

    format!(
        "Hostname: {}\n{}\nCPU: {}\nMemory: {}",
        hostname, sw_vers, cpu, mem
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialise the journal database.
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data directory");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");

            let db_path = app_dir.join("journal.db");
            let db = journal::init_db(
                db_path
                    .to_str()
                    .expect("Invalid database path"),
            )
            .expect("Failed to initialise journal database");

            // Build the tool router and register platform tools.
            let mut router = ToolRouter::new();
            platform::register_platform_tools(&mut router);

            // Load API key: config file first, then env var.
            let api_key = load_api_key(&app_dir);
            let llm = LlmClient::new(api_key);

            let pending_approvals: PendingApprovals =
                Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<bool>>::new()));

            // Gather OS context for the system prompt.
            let os_context = gather_os_context();

            // Build the orchestrator.
            let orchestrator =
                Orchestrator::new(llm, router, os_context, pending_approvals.clone());

            // Manage shared state.
            app.manage(AppState {
                orchestrator: Mutex::new(orchestrator),
                pending_approvals,
                db: Arc::new(Mutex::new(db)),
                app_dir,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::session::create_session,
            commands::session::get_session,
            commands::session::end_session,
            commands::session::list_sessions,
            commands::session::get_session_messages,
            commands::agent::send_message,
            commands::agent::approve_action,
            commands::agent::deny_action,
            commands::safety::get_changes,
            commands::safety::undo_change,
            commands::settings::has_api_key,
            commands::settings::set_api_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
