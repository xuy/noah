mod agent;
mod commands;
pub mod debug_runner;
mod knowledge;
mod machine_context;
mod platform;
mod playbooks;
mod proactive;
mod safety;
mod scanner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;
use tauri::menu::{MenuBuilder, SubmenuBuilder, PredefinedMenuItem};
use tokio::sync::{oneshot, Mutex};

use agent::llm_client::{AuthMode, LlmClient};
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
    /// Path to the knowledge directory (knowledge/).
    pub knowledge_dir: PathBuf,
    /// Cancellation flag — can be set without holding the orchestrator lock.
    pub cancelled: Arc<AtomicBool>,
    /// Scanner trigger handle — set a scan_type string to request an on-demand scan.
    pub scanner_trigger: Arc<std::sync::Mutex<Option<String>>>,
    /// Scanner pause handle — add scan_type strings to pause specific scanners.
    pub scanner_pause: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

/// Load auth: proxy.json first, then api_key.txt, then env var.
fn load_auth(app_dir: &std::path::Path) -> AuthMode {
    // Check for proxy config first
    let proxy_path = app_dir.join("proxy.json");
    if let Ok(contents) = std::fs::read_to_string(&proxy_path) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&contents) {
            if let (Some(base_url), Some(token)) = (
                parsed.get("base_url").and_then(|v| v.as_str()),
                parsed.get("token").and_then(|v| v.as_str()),
            ) {
                if !token.is_empty() {
                    return AuthMode::Proxy {
                        base_url: base_url.to_string(),
                        token: token.to_string(),
                    };
                }
            }
        }
    }

    // Fall back to API key file
    let key_path = app_dir.join("api_key.txt");
    if let Ok(contents) = std::fs::read_to_string(&key_path) {
        let key = contents.trim().to_string();
        if !key.is_empty() {
            return AuthMode::ApiKey(key);
        }
    }

    // Fall back to environment variable
    AuthMode::ApiKey(std::env::var("ANTHROPIC_API_KEY").unwrap_or_default())
}

/// Save API key to config file (and remove proxy.json if present).
pub fn save_api_key(app_dir: &std::path::Path, key: &str) -> Result<(), String> {
    let key_path = app_dir.join("api_key.txt");
    std::fs::write(&key_path, key).map_err(|e| format!("Failed to save API key: {}", e))?;
    // Remove proxy config if switching to API key mode
    let proxy_path = app_dir.join("proxy.json");
    let _ = std::fs::remove_file(&proxy_path);
    Ok(())
}

/// Save proxy config (and remove api_key.txt if present).
pub fn save_proxy_config(app_dir: &std::path::Path, base_url: &str, token: &str) -> Result<(), String> {
    let proxy_path = app_dir.join("proxy.json");
    let json = serde_json::json!({ "base_url": base_url, "token": token });
    std::fs::write(&proxy_path, json.to_string())
        .map_err(|e| format!("Failed to save proxy config: {}", e))?;
    // Remove API key file if switching to proxy mode
    let key_path = app_dir.join("api_key.txt");
    let _ = std::fs::remove_file(&key_path);
    Ok(())
}

/// Clear all auth config.
pub fn clear_auth_files(app_dir: &std::path::Path) {
    let _ = std::fs::remove_file(app_dir.join("api_key.txt"));
    let _ = std::fs::remove_file(app_dir.join("proxy.json"));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Disable GPU acceleration for WebKit2GTK on Linux to fix GBM/EGL errors
    // This is needed on Fedora 43 and other Linux systems with certain GPU drivers
    #[cfg(target_os = "linux")] {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        std::env::set_var("WEBKIT_DISABLE_GPU_COMPOSITING", "1");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Build native menu bar with View > Zoom controls.
            let app_menu = SubmenuBuilder::new(app, "Noah")
                .about(None)
                .separator()
                .services()
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let view_menu = SubmenuBuilder::new(app, "View")
                .item(&PredefinedMenuItem::fullscreen(app, None)?)
                .build()?;

            let window_menu = SubmenuBuilder::new(app, "Window")
                .minimize()
                .item(&PredefinedMenuItem::maximize(app, None)?)
                .separator()
                .close_window()
                .build()?;

            let menu = MenuBuilder::new(app)
                .item(&app_menu)
                .item(&edit_menu)
                .item(&view_menu)
                .item(&window_menu)
                .build()?;

            app.set_menu(menu)?;

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

            // Initialise the knowledge directory.
            let knowledge_dir = knowledge::init_knowledge_dir(&app_dir)
                .expect("Failed to create knowledge directory");

            // Run file-based migrations (e.g. artifact → knowledge file migration).
            {
                let conn = db_arc.blocking_lock();
                journal::run_file_migrations(&conn, &knowledge_dir)
                    .expect("Failed to run file migrations");
            }

            // Build the tool router and register platform tools.
            let mut router = ToolRouter::new();
            platform::register_platform_tools(&mut router, Some(&db_path));

            // Register knowledge tools.
            router.register(Box::new(knowledge::WriteKnowledgeTool::new(knowledge_dir.clone())));
            router.register(Box::new(knowledge::SearchKnowledgeTool::new(knowledge_dir.clone())));
            router.register(Box::new(knowledge::ReadKnowledgeTool::new(knowledge_dir.clone())));
            router.register(Box::new(knowledge::ListKnowledgeTool::new(knowledge_dir.clone())));

            // Seed built-in playbooks into knowledge/playbooks/ and register activate_playbook.
            let playbook_registry = playbooks::PlaybookRegistry::init(&knowledge_dir)
                .expect("Failed to initialise playbooks");
            router.register(Box::new(playbooks::ActivatePlaybookTool::new(playbook_registry)));

            // Load auth: proxy config, API key file, or env var.
            let auth = load_auth(&app_dir);
            let llm = LlmClient::with_auth(auth);
            let llm_for_monitor = llm.clone();

            let pending_approvals: PendingApprovals =
                Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<bool>>::new()));

            // Gather OS context for the system prompt.
            let os_context = machine_context::MachineContext::load_or_gather(&app_dir)
                .to_prompt_string();

            // Build the orchestrator.
            let orchestrator =
                Orchestrator::new(llm, router, os_context, pending_approvals.clone(), db_arc.clone(), knowledge_dir.clone());
            let cancelled = orchestrator.cancelled_flag();

            // Build the scanner manager with registered scanners.
            let mut scanner_mgr = scanner::ScannerManager::new(db_arc.clone(), Some(app.handle().clone()));
            #[cfg(target_os = "macos")]
            scanner_mgr.register(Box::new(scanner::disk::DiskScanner));
            let scanner_trigger = scanner_mgr.trigger_handle();
            let scanner_pause = scanner_mgr.pause_handle();

            // Manage shared state.
            app.manage(AppState {
                orchestrator: Mutex::new(orchestrator),
                pending_approvals,
                db: db_arc.clone(),
                app_dir,
                knowledge_dir,
                cancelled,
                scanner_trigger,
                scanner_pause,
            });

            // Spawn the proactive health monitor in the background.
            let monitor = proactive::ProactiveMonitor::new(
                llm_for_monitor, db_arc, app.handle().clone(),
            );
            tauri::async_runtime::spawn(async move { monitor.run_forever().await });

            // Spawn the scanner manager: initial light scan after 5 min, then periodic.
            tauri::async_runtime::spawn(async move {
                // Initial delay — let app finish starting.
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                eprintln!("[scanner] starting initial scan");
                scanner_mgr.run_cycle(std::time::Duration::from_secs(30)).await;

                // Then run every 6 hours alongside proactive monitor.
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
                    // Check for on-demand triggers first.
                    scanner_mgr.run_triggered().await;
                    // Regular cycle.
                    scanner_mgr.run_cycle(std::time::Duration::from_secs(60)).await;
                }
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
            commands::settings::redeem_invite_code,
            commands::settings::get_auth_mode,
            commands::settings::clear_auth,
            commands::settings::get_app_version,
            commands::settings::get_telemetry_consent,
            commands::settings::set_telemetry_consent,
            commands::settings::track_event,
            commands::settings::get_feedback_context,
            commands::settings::get_proactive_enabled,
            commands::settings::set_proactive_enabled,
            commands::settings::dismiss_proactive_suggestion,
            commands::settings::act_on_proactive_suggestion,
            commands::knowledge::list_knowledge,
            commands::knowledge::read_knowledge_file,
            commands::knowledge::delete_knowledge_file,
            commands::scanner::trigger_scan,
            commands::scanner::pause_scan,
            commands::scanner::resume_scan,
            commands::scanner::get_scan_jobs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
