// Re-export noah-core modules so existing `crate::` paths in commands/ still work.
pub use noah_core::agent;
pub use noah_core::config;
pub use noah_core::events;
pub use noah_core::knowledge;
pub use noah_core::machine_context;
pub use noah_core::playbooks;
pub use noah_core::safety;
pub use noah_core::ui_parsing;
pub use noah_core::ui_tools;
pub use noah_core::web_fetch;

// Desktop-only modules (Tauri-dependent).
mod commands;
pub mod mobile_server;
pub mod tauri_events;
mod platform;
mod proactive;
mod scanner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;
use tauri::menu::{MenuBuilder, SubmenuBuilder, PredefinedMenuItem};
use tokio::sync::{broadcast, oneshot, Mutex};

use agent::llm_client::LlmClient;
use agent::orchestrator::{Orchestrator, PendingApprovals};
use agent::tool_router::ToolRouter;
use safety::journal;

/// Shared application state managed by Tauri.
pub struct AppState {
    pub orchestrator: Arc<Mutex<Orchestrator>>,
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
    /// Mobile server state — shared with the embedded axum server.
    pub mobile_server: Arc<mobile_server::MobileServerState>,
    /// Port the mobile server is listening on.
    pub mobile_server_port: u16,
}

/// Migrate user data from the old `com.itman.app` directory to the new location.
fn migrate_old_data_dir(new_dir: &std::path::Path) {
    let Some(parent) = new_dir.parent() else { return };
    let old_dir = parent.join("com.itman.app");

    if !old_dir.is_dir() {
        return;
    }

    if new_dir.join("journal.db").exists() {
        eprintln!(
            "[migrate] New data dir already has journal.db, skipping migration from {:?}",
            old_dir
        );
        return;
    }

    eprintln!(
        "[migrate] Moving data from {:?} to {:?}",
        old_dir, new_dir
    );

    if let Err(e) = copy_dir_recursive(&old_dir, new_dir) {
        eprintln!("[migrate] Error copying data: {}. Old dir preserved.", e);
        return;
    }

    if let Err(e) = std::fs::remove_dir_all(&old_dir) {
        eprintln!("[migrate] Could not remove old dir {:?}: {}", old_dir, e);
    } else {
        eprintln!("[migrate] Migration complete, old dir removed.");
    }
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if !dst_path.exists() {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")] {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        std::env::set_var("WEBKIT_DISABLE_GPU_COMPOSITING", "1");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            if cfg!(target_os = "macos") {
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
            }

            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data directory");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");

            migrate_old_data_dir(&app_dir);

            let db_path = app_dir.join("journal.db");
            let db_path_str = db_path.to_str().expect("Invalid database path");

            let db = match journal::init_db(db_path_str) {
                Ok(db) => db,
                Err(_) => {
                    let bak_path = app_dir.join("journal.db.bak");
                    let _ = std::fs::rename(&db_path, &bak_path);
                    eprintln!(
                        "Warning: journal.db was corrupted. Backed up to journal.db.bak and starting fresh."
                    );
                    journal::init_db(db_path_str)
                        .expect("Failed to create fresh journal database")
                }
            };

            let db_arc = Arc::new(Mutex::new(db));

            let knowledge_dir = knowledge::init_knowledge_dir(&app_dir)
                .expect("Failed to create knowledge directory");

            {
                let conn = db_arc.blocking_lock();
                journal::run_file_migrations(&conn, &knowledge_dir)
                    .expect("Failed to run file migrations");
            }

            let mut router = ToolRouter::new();
            platform::register_platform_tools(&mut router, Some(&db_path));

            router.register(Box::new(knowledge::WriteKnowledgeTool::new(knowledge_dir.clone())));
            router.register(Box::new(knowledge::KnowledgeSearchTool::new(knowledge_dir.clone())));
            router.register(Box::new(knowledge::KnowledgeReadTool::new(knowledge_dir.clone())));

            ui_tools::register_ui_tools(&mut router);

            router.register(Box::new(web_fetch::WebFetchTool));

            let bundled_playbooks = app
                .path()
                .resource_dir()
                .expect("Failed to resolve resource directory")
                .join("playbooks");
            let playbook_registry = playbooks::PlaybookRegistry::init(&knowledge_dir, &bundled_playbooks)
                .expect("Failed to initialise playbooks");
            router.register(Box::new(playbooks::ActivatePlaybookTool::new(playbook_registry)));

            let auth = config::load_auth(&app_dir);
            let llm = LlmClient::with_auth(auth);
            let llm_for_monitor = llm.clone();

            let pending_approvals: PendingApprovals =
                Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<bool>>::new()));

            let os_context = machine_context::MachineContext::load_or_gather(&app_dir)
                .to_prompt_string();

            let orchestrator =
                Orchestrator::new(llm, router, os_context, pending_approvals.clone(), db_arc.clone(), knowledge_dir.clone());
            let cancelled = orchestrator.cancelled_flag();
            let orchestrator_arc = Arc::new(Mutex::new(orchestrator));

            let mut scanner_mgr = scanner::ScannerManager::new(db_arc.clone(), Some(app.handle().clone()));
            #[cfg(target_os = "macos")]
            scanner_mgr.register(Box::new(scanner::disk::DiskScanner));
            let scanner_trigger = scanner_mgr.trigger_handle();
            let scanner_pause = scanner_mgr.pause_handle();

            let ctx_dir = app_dir.clone();

            // Set up mobile server
            let (sse_tx, _) = broadcast::channel::<mobile_server::SseEvent>(64);

            // Restore pairing from DB if previously paired
            let restored_pairing = {
                let conn = db_arc.blocking_lock();
                journal::get_setting(&conn, "paired_device")
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str::<mobile_server::PairedDevice>(&s).ok())
            };

            let mobile_state = Arc::new(mobile_server::MobileServerState {
                orchestrator: orchestrator_arc.clone(),
                db: db_arc.clone(),
                pending_approvals: pending_approvals.clone(),
                pairing: Mutex::new(mobile_server::PairingState {
                    pending_token: None,
                    paired_device: restored_pairing,
                    failed_attempts: 0,
                }),
                sse_tx,
                app_dir: app_dir.clone(),
                app_handle: Some(app.handle().clone()),
                port: std::sync::atomic::AtomicU16::new(0),
            });

            let mobile_state_for_server = mobile_state.clone();
            let mobile_server_port = tauri::async_runtime::block_on(async {
                mobile_server::start_server(mobile_state_for_server).await.unwrap_or_else(|e| {
                    eprintln!("[mobile-server] Failed to start: {}", e);
                    0
                })
            });
            mobile_state.port.store(mobile_server_port, std::sync::atomic::Ordering::Relaxed);

            app.manage(AppState {
                orchestrator: orchestrator_arc,
                pending_approvals,
                db: db_arc.clone(),
                app_dir,
                knowledge_dir,
                cancelled,
                scanner_trigger,
                scanner_pause,
                mobile_server: mobile_state,
                mobile_server_port,
            });

            tauri::async_runtime::spawn(async move {
                tokio::task::spawn_blocking(move || {
                    machine_context::MachineContext::refresh_if_stale(&ctx_dir);
                }).await.ok();
            });

            let monitor = proactive::ProactiveMonitor::new(
                llm_for_monitor, db_arc, app.handle().clone(),
            );
            tauri::async_runtime::spawn(async move { monitor.run_forever().await });

            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                eprintln!("[scanner] starting initial scan");
                scanner_mgr.run_cycle(std::time::Duration::from_secs(30)).await;

                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
                    scanner_mgr.run_triggered().await;
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
            commands::session::rename_session,
            commands::session::list_sessions,
            commands::session::get_session_messages,
            commands::session::get_session_summary,
            commands::session::export_session,
            commands::session::mark_resolved,
            commands::agent::send_message,
            commands::agent::send_message_v2,
            commands::agent::send_user_event,
            commands::agent::record_action_confirmation,
            commands::agent::store_secret,
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
            commands::settings::set_locale,
            commands::settings::set_session_mode,
            commands::knowledge::list_knowledge,
            commands::knowledge::read_knowledge_file,
            commands::knowledge::delete_knowledge_file,
            commands::scanner::trigger_scan,
            commands::scanner::pause_scan,
            commands::scanner::resume_scan,
            commands::scanner::get_scan_jobs,
            commands::pairing::generate_pairing_qr,
            commands::pairing::get_pairing_status,
            commands::pairing::unpair_device,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_migrate_copies_files_and_removes_old_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let parent = tmp.path();

        let old_dir = parent.join("com.itman.app");
        fs::create_dir_all(old_dir.join("knowledge")).unwrap();
        fs::write(old_dir.join("journal.db"), b"fake-db-content").unwrap();
        fs::write(old_dir.join("api_key.txt"), b"sk-ant-test").unwrap();
        fs::write(old_dir.join("knowledge/note.md"), b"# My note").unwrap();

        let new_dir = parent.join("app.onnoah.desktop");
        fs::create_dir_all(&new_dir).unwrap();

        migrate_old_data_dir(&new_dir);

        assert_eq!(fs::read_to_string(new_dir.join("journal.db")).unwrap(), "fake-db-content");
        assert_eq!(fs::read_to_string(new_dir.join("api_key.txt")).unwrap(), "sk-ant-test");
        assert_eq!(fs::read_to_string(new_dir.join("knowledge/note.md")).unwrap(), "# My note");

        assert!(!old_dir.exists());
    }

    #[test]
    fn test_migrate_skips_when_new_dir_has_journal() {
        let tmp = tempfile::tempdir().unwrap();
        let parent = tmp.path();

        let old_dir = parent.join("com.itman.app");
        fs::create_dir_all(&old_dir).unwrap();
        fs::write(old_dir.join("journal.db"), b"old-data").unwrap();

        let new_dir = parent.join("app.onnoah.desktop");
        fs::create_dir_all(&new_dir).unwrap();
        fs::write(new_dir.join("journal.db"), b"new-data").unwrap();

        migrate_old_data_dir(&new_dir);

        assert_eq!(fs::read_to_string(new_dir.join("journal.db")).unwrap(), "new-data");
        assert!(old_dir.exists());
    }

    #[test]
    fn test_migrate_noop_when_no_old_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let new_dir = tmp.path().join("app.onnoah.desktop");
        fs::create_dir_all(&new_dir).unwrap();

        migrate_old_data_dir(&new_dir);
    }

    #[test]
    fn test_migrate_does_not_overwrite_existing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let parent = tmp.path();

        let old_dir = parent.join("com.itman.app");
        fs::create_dir_all(&old_dir).unwrap();
        fs::write(old_dir.join("api_key.txt"), b"old-key").unwrap();
        fs::write(old_dir.join("journal.db"), b"old-db").unwrap();

        let new_dir = parent.join("app.onnoah.desktop");
        fs::create_dir_all(&new_dir).unwrap();
        fs::write(new_dir.join("api_key.txt"), b"new-key").unwrap();

        migrate_old_data_dir(&new_dir);

        assert_eq!(fs::read_to_string(new_dir.join("api_key.txt")).unwrap(), "new-key");
        assert_eq!(fs::read_to_string(new_dir.join("journal.db")).unwrap(), "old-db");
    }
}
