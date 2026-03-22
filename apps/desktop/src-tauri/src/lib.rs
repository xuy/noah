pub mod agent;
mod commands;
mod dashboard_link;
pub mod debug_runner;
mod knowledge;
mod machine_context;
mod system_snapshot;
mod platform;
mod playbooks;
pub(crate) mod fleet_policy;
mod autoheal;
mod proactive;
mod safety;
mod scanner;
pub mod ui_tools;
mod web_fetch;

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

/// Migrate user data from the old `com.itman.app` directory to the new location.
///
/// Tauri derives the app data dir from `identifier` in tauri.conf.json.
/// We renamed from `com.itman.app` → `app.onnoah.desktop`, so existing users
/// have their DB, keys, and knowledge under the old path. This function copies
/// all files from the old dir into the new dir (without overwriting), then
/// removes the old directory.
fn migrate_old_data_dir(new_dir: &std::path::Path) {
    // Build the old path by replacing the last component.
    let Some(parent) = new_dir.parent() else { return };
    let old_dir = parent.join("com.itman.app");

    if !old_dir.is_dir() {
        return;
    }

    // If the new dir already has a journal.db, the user has already used the
    // new version — don't overwrite their data.
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

    // Copy everything from old → new recursively.
    if let Err(e) = copy_dir_recursive(&old_dir, new_dir) {
        eprintln!("[migrate] Error copying data: {}. Old dir preserved.", e);
        return;
    }

    // Remove old dir after successful copy.
    if let Err(e) = std::fs::remove_dir_all(&old_dir) {
        eprintln!("[migrate] Could not remove old dir {:?}: {}", old_dir, e);
    } else {
        eprintln!("[migrate] Migration complete, old dir removed.");
    }
}

/// Recursively copy contents of `src` into `dst`, skipping files that already exist.
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
            // Native menu bar — macOS only.
            // On Linux/Windows the WM provides window controls and the menu bar
            // just wastes vertical space with macOS-specific items (Services, Hide, etc.).
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

            // Initialise the journal database.
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data directory");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");

            // Migrate data from old app identifiers (com.itman.app) if present.
            migrate_old_data_dir(&app_dir);

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
            router.register(Box::new(knowledge::KnowledgeSearchTool::new(knowledge_dir.clone())));
            router.register(Box::new(knowledge::KnowledgeReadTool::new(knowledge_dir.clone())));

            // Register UI tools.
            ui_tools::register_ui_tools(&mut router);

            // Register web_fetch tool.
            router.register(Box::new(web_fetch::WebFetchTool));

            // Seed bundled playbooks into knowledge/playbooks/ and register activate_playbook.
            // Playbooks ship as Tauri bundled resources (plain files, not compiled in).
            let bundled_playbooks = app
                .path()
                .resource_dir()
                .expect("Failed to resolve resource directory")
                .join("playbooks");
            let playbook_registry = playbooks::PlaybookRegistry::init(&knowledge_dir, &bundled_playbooks)
                .expect("Failed to initialise playbooks");
            router.register(Box::new(playbooks::ActivatePlaybookTool::new(playbook_registry)));

            // Load auth: proxy config, API key file, or env var.
            let auth = load_auth(&app_dir);
            let llm = LlmClient::with_auth(auth);
            let llm_for_monitor = llm.clone();
            let llm_for_autoheal = llm.clone();

            let pending_approvals: PendingApprovals =
                Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<bool>>::new()));

            // Gather OS context for the system prompt.
            let machine_ctx = machine_context::MachineContext::load_or_gather(&app_dir)
                .to_prompt_string();
            let snapshot_ctx = system_snapshot::SystemSnapshot::load_or_gather(&app_dir)
                .to_prompt_string();
            let os_context = if snapshot_ctx.is_empty() {
                machine_ctx
            } else {
                format!("{}\n{}", machine_ctx, snapshot_ctx)
            };

            // Build the orchestrator.
            let orchestrator =
                Orchestrator::new(llm, router, os_context, pending_approvals.clone(), db_arc.clone(), knowledge_dir.clone());
            let cancelled = orchestrator.cancelled_flag();

            // Build the scanner manager with registered scanners.
            let mut scanner_mgr = scanner::ScannerManager::new(db_arc.clone(), Some(app.handle().clone()));
            #[cfg(target_os = "macos")]
            scanner_mgr.register(Box::new(scanner::disk::DiskScanner));
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            scanner_mgr.register(Box::new(scanner::security::SecurityScanner));
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            scanner_mgr.register(Box::new(scanner::updates::UpdateScanner));
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            scanner_mgr.register(Box::new(scanner::backups::BackupScanner));
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            scanner_mgr.register(Box::new(scanner::performance::PerformanceScanner));
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            scanner_mgr.register(Box::new(scanner::network::NetworkScanner));
            let scanner_trigger = scanner_mgr.trigger_handle();
            let scanner_pause = scanner_mgr.pause_handle();

            // Clone app_dir before moving into AppState (needed for background refresh).
            let ctx_dir = app_dir.clone();
            let snap_dir = app_dir.clone();
            let health_db = db_arc.clone();
            let health_app_dir = app_dir.clone();
            let autoheal_db = db_arc.clone();
            let autoheal_app_dir = app_dir.clone();

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

            // Refresh machine context and system snapshot in background (avoids
            // blocking main thread with slow commands on Windows).
            tauri::async_runtime::spawn(async move {
                tokio::task::spawn_blocking(move || {
                    machine_context::MachineContext::refresh_if_stale(&ctx_dir);
                }).await.ok();
            });
            tauri::async_runtime::spawn(async move {
                tokio::task::spawn_blocking(move || {
                    system_snapshot::SystemSnapshot::refresh_if_stale(&snap_dir);
                }).await.ok();
            });

            // Spawn the proactive health monitor in the background.
            let monitor = proactive::ProactiveMonitor::new(
                llm_for_monitor, db_arc, app.handle().clone(),
            );
            tauri::async_runtime::spawn(async move { monitor.run_forever().await });

            // Spawn the auto-heal monitor in the background.
            let autoheal_monitor = autoheal::AutoHealMonitor::new(
                llm_for_autoheal, autoheal_db, app.handle().clone(), autoheal_app_dir,
            );
            tauri::async_runtime::spawn(async move { autoheal_monitor.run_forever().await });

            // Spawn the scanner manager: initial light scan after 5 min, then periodic.
            tauri::async_runtime::spawn(async move {
                // Initial delay — let app finish starting.
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                eprintln!("[scanner] starting initial scan");
                scanner_mgr.run_cycle(std::time::Duration::from_secs(30)).await;

                // Run health scanners and auto-sync to fleet.
                {
                    let db_for_health = health_db.clone();
                    let dir_for_health = health_app_dir.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        let conn = db_for_health.blocking_lock();
                        let budget = std::time::Duration::from_secs(120);

                        let enabled = {
                            crate::dashboard_link::DashboardConfig::load(&dir_for_health)
                                .and_then(|c| c.enabled_categories)
                                .map(|cats| cats.iter().filter_map(|s| match s.as_str() {
                                    "security" => Some(noah_health::Category::Security),
                                    "updates" => Some(noah_health::Category::Updates),
                                    "performance" => Some(noah_health::Category::Performance),
                                    "backups" => Some(noah_health::Category::Backups),
                                    "network" => Some(noah_health::Category::Network),
                                    _ => None,
                                }).collect::<Vec<_>>())
                        };

                        let should_scan = |cat: noah_health::Category| -> bool {
                            match &enabled {
                                None => true,
                                Some(cats) => cats.contains(&cat),
                            }
                        };

                        use crate::scanner::Scanner;
                        if should_scan(noah_health::Category::Security) {
                            let _ = crate::scanner::security::SecurityScanner.tick(budget, &conn);
                        }
                        if should_scan(noah_health::Category::Updates) {
                            let _ = crate::scanner::updates::UpdateScanner.tick(budget, &conn);
                        }
                        if should_scan(noah_health::Category::Backups) {
                            let _ = crate::scanner::backups::BackupScanner.tick(budget, &conn);
                        }
                        if should_scan(noah_health::Category::Performance) {
                            let _ = crate::scanner::performance::PerformanceScanner.tick(budget, &conn);
                        }
                        if should_scan(noah_health::Category::Network) {
                            let _ = crate::scanner::network::NetworkScanner.tick(budget, &conn);
                        }

                        // Compute and persist health score.
                        let mut all_checks = Vec::new();
                        let scan_types = [("security", noah_health::Category::Security), ("updates", noah_health::Category::Updates), ("backups", noah_health::Category::Backups), ("performance", noah_health::Category::Performance), ("network", noah_health::Category::Network)];
                        for (scan_type, category) in &scan_types {
                            if let Ok(results) = crate::safety::journal::query_scan_results(&conn, scan_type, None, None, None, 100) {
                                for r in &results {
                                    let status = match r.value_text.as_deref() {
                                        Some("pass") => noah_health::CheckStatus::Pass,
                                        Some("warn") => noah_health::CheckStatus::Warn,
                                        _ => noah_health::CheckStatus::Fail,
                                    };
                                    all_checks.push(noah_health::CheckResult {
                                        id: r.path.clone().unwrap_or_default(),
                                        category: *category,
                                        label: r.key.clone().unwrap_or_default(),
                                        status,
                                        detail: r.metadata.clone().unwrap_or_default(),
                                    });
                                }
                            }
                        }

                        if !all_checks.is_empty() {
                            let score = noah_health::compute_score(all_checks, None, enabled.as_deref());
                            let record = journal::HealthScoreRecord {
                                id: uuid::Uuid::new_v4().to_string(),
                                score: score.overall_score as i32,
                                grade: score.overall_grade.to_string(),
                                categories: serde_json::to_string(&score.categories).unwrap_or_default(),
                                computed_at: score.computed_at.clone(),
                                device_id: score.device_id.clone(),
                            };
                            let _ = journal::insert_health_score(&conn, &record);

                            // Return score + dir for async fleet sync.
                            Some((score, dir_for_health))
                        } else {
                            None
                        }
                    }).await.ok().flatten().map(|(score, app_dir)| {
                        // Auto-sync to fleet if linked.
                        if let Some(config) = crate::dashboard_link::DashboardConfig::load(&app_dir) {
                            let cats = serde_json::to_string(&score.categories).unwrap_or_else(|_| "[]".to_string());
                            let s = score.overall_score as i32;
                            let g = score.overall_grade.to_string();
                            let sync_app_dir = app_dir.clone();
                            tokio::spawn(async move {
                                match crate::dashboard_link::push_checkin(&config, s, &g, &cats, Some(&sync_app_dir)).await {
                                    Ok(Some(new_cats)) => {
                                        // Update enabled_categories from fleet policy.
                                        if let Some(mut cfg) = crate::dashboard_link::DashboardConfig::load(&sync_app_dir) {
                                            cfg.enabled_categories = Some(new_cats);
                                            let _ = cfg.save(&sync_app_dir);
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(e) => eprintln!("[health] auto fleet sync failed: {}", e),
                                }
                            });
                        }
                    });
                }
                eprintln!("[health] auto health check complete");

                // Then run every 6 hours alongside proactive monitor.
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
                    // Check for on-demand triggers first.
                    scanner_mgr.run_triggered().await;
                    // Regular cycle.
                    scanner_mgr.run_cycle(std::time::Duration::from_secs(60)).await;

                    // Run health scanners and auto-sync to fleet.
                    {
                        let db_for_health = health_db.clone();
                        let dir_for_health = health_app_dir.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            let conn = db_for_health.blocking_lock();
                            let budget = std::time::Duration::from_secs(120);

                            let enabled = {
                                crate::dashboard_link::DashboardConfig::load(&dir_for_health)
                                    .and_then(|c| c.enabled_categories)
                                    .map(|cats| cats.iter().filter_map(|s| match s.as_str() {
                                        "security" => Some(noah_health::Category::Security),
                                        "updates" => Some(noah_health::Category::Updates),
                                        "performance" => Some(noah_health::Category::Performance),
                                        "backups" => Some(noah_health::Category::Backups),
                                        "network" => Some(noah_health::Category::Network),
                                        _ => None,
                                    }).collect::<Vec<_>>())
                            };

                            let should_scan = |cat: noah_health::Category| -> bool {
                                match &enabled {
                                    None => true,
                                    Some(cats) => cats.contains(&cat),
                                }
                            };

                            use crate::scanner::Scanner;
                            if should_scan(noah_health::Category::Security) {
                                let _ = crate::scanner::security::SecurityScanner.tick(budget, &conn);
                            }
                            if should_scan(noah_health::Category::Updates) {
                                let _ = crate::scanner::updates::UpdateScanner.tick(budget, &conn);
                            }
                            if should_scan(noah_health::Category::Backups) {
                                let _ = crate::scanner::backups::BackupScanner.tick(budget, &conn);
                            }
                            if should_scan(noah_health::Category::Performance) {
                                let _ = crate::scanner::performance::PerformanceScanner.tick(budget, &conn);
                            }
                            if should_scan(noah_health::Category::Network) {
                                let _ = crate::scanner::network::NetworkScanner.tick(budget, &conn);
                            }

                            // Compute and persist health score.
                            let mut all_checks = Vec::new();
                            let scan_types = [("security", noah_health::Category::Security), ("updates", noah_health::Category::Updates), ("backups", noah_health::Category::Backups), ("performance", noah_health::Category::Performance), ("network", noah_health::Category::Network)];
                            for (scan_type, category) in &scan_types {
                                if let Ok(results) = crate::safety::journal::query_scan_results(&conn, scan_type, None, None, None, 100) {
                                    for r in &results {
                                        let status = match r.value_text.as_deref() {
                                            Some("pass") => noah_health::CheckStatus::Pass,
                                            Some("warn") => noah_health::CheckStatus::Warn,
                                            _ => noah_health::CheckStatus::Fail,
                                        };
                                        all_checks.push(noah_health::CheckResult {
                                            id: r.path.clone().unwrap_or_default(),
                                            category: *category,
                                            label: r.key.clone().unwrap_or_default(),
                                            status,
                                            detail: r.metadata.clone().unwrap_or_default(),
                                        });
                                    }
                                }
                            }

                            if !all_checks.is_empty() {
                                let score = noah_health::compute_score(all_checks, None, enabled.as_deref());
                                let record = journal::HealthScoreRecord {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    score: score.overall_score as i32,
                                    grade: score.overall_grade.to_string(),
                                    categories: serde_json::to_string(&score.categories).unwrap_or_default(),
                                    computed_at: score.computed_at.clone(),
                                    device_id: score.device_id.clone(),
                                };
                                let _ = journal::insert_health_score(&conn, &record);

                                Some((score, dir_for_health))
                            } else {
                                None
                            }
                        }).await.ok().flatten().map(|(score, app_dir)| {
                            if let Some(config) = crate::dashboard_link::DashboardConfig::load(&app_dir) {
                                let cats = serde_json::to_string(&score.categories).unwrap_or_else(|_| "[]".to_string());
                                let s = score.overall_score as i32;
                                let g = score.overall_grade.to_string();
                                let sync_app_dir = app_dir.clone();
                                tokio::spawn(async move {
                                    match crate::dashboard_link::push_checkin(&config, s, &g, &cats, Some(&sync_app_dir)).await {
                                        Ok(Some(new_cats)) => {
                                            if let Some(mut cfg) = crate::dashboard_link::DashboardConfig::load(&sync_app_dir) {
                                                cfg.enabled_categories = Some(new_cats);
                                                let _ = cfg.save(&sync_app_dir);
                                            }
                                        }
                                        Ok(None) => {}
                                        Err(e) => eprintln!("[health] auto fleet sync failed: {}", e),
                                    }
                                });
                            }
                        });
                    }
                    eprintln!("[health] auto health check complete");
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
            commands::settings::get_auto_heal_enabled,
            commands::settings::set_auto_heal_enabled,
            commands::settings::dismiss_proactive_suggestion,
            commands::settings::act_on_proactive_suggestion,
            commands::settings::set_locale,
            commands::settings::set_session_mode,
            commands::settings::link_dashboard,
            commands::settings::unlink_dashboard,
            commands::settings::get_dashboard_status,
            commands::knowledge::list_knowledge,
            commands::knowledge::read_knowledge_file,
            commands::knowledge::delete_knowledge_file,
            commands::scanner::trigger_scan,
            commands::scanner::pause_scan,
            commands::scanner::resume_scan,
            commands::scanner::get_scan_jobs,
            commands::health::get_health_score,
            commands::health::run_health_check,
            commands::health::get_health_history,
            commands::health::export_health_report,
            commands::health::open_health_fix,
            commands::health::get_fleet_actions,
            commands::health::resolve_fleet_action,
            commands::health::start_fleet_playbook,
            commands::health::verify_remediation,
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

        // Set up old dir with some files.
        let old_dir = parent.join("com.itman.app");
        fs::create_dir_all(old_dir.join("knowledge")).unwrap();
        fs::write(old_dir.join("journal.db"), b"fake-db-content").unwrap();
        fs::write(old_dir.join("api_key.txt"), b"sk-ant-test").unwrap();
        fs::write(old_dir.join("knowledge/note.md"), b"# My note").unwrap();

        // New dir exists but is empty (no journal.db).
        let new_dir = parent.join("app.onnoah.desktop");
        fs::create_dir_all(&new_dir).unwrap();

        migrate_old_data_dir(&new_dir);

        // Files should be in the new dir.
        assert_eq!(fs::read_to_string(new_dir.join("journal.db")).unwrap(), "fake-db-content");
        assert_eq!(fs::read_to_string(new_dir.join("api_key.txt")).unwrap(), "sk-ant-test");
        assert_eq!(fs::read_to_string(new_dir.join("knowledge/note.md")).unwrap(), "# My note");

        // Old dir should be gone.
        assert!(!old_dir.exists());
    }

    #[test]
    fn test_migrate_skips_when_new_dir_has_journal() {
        let tmp = tempfile::tempdir().unwrap();
        let parent = tmp.path();

        // Old dir with data.
        let old_dir = parent.join("com.itman.app");
        fs::create_dir_all(&old_dir).unwrap();
        fs::write(old_dir.join("journal.db"), b"old-data").unwrap();

        // New dir already has its own journal.db.
        let new_dir = parent.join("app.onnoah.desktop");
        fs::create_dir_all(&new_dir).unwrap();
        fs::write(new_dir.join("journal.db"), b"new-data").unwrap();

        migrate_old_data_dir(&new_dir);

        // New data should be untouched.
        assert_eq!(fs::read_to_string(new_dir.join("journal.db")).unwrap(), "new-data");
        // Old dir should still exist (not removed since migration was skipped).
        assert!(old_dir.exists());
    }

    #[test]
    fn test_migrate_noop_when_no_old_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let new_dir = tmp.path().join("app.onnoah.desktop");
        fs::create_dir_all(&new_dir).unwrap();

        // Should not panic or error.
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

        // New dir has api_key.txt but no journal.db → migration runs.
        let new_dir = parent.join("app.onnoah.desktop");
        fs::create_dir_all(&new_dir).unwrap();
        fs::write(new_dir.join("api_key.txt"), b"new-key").unwrap();

        migrate_old_data_dir(&new_dir);

        // Existing file should not be overwritten.
        assert_eq!(fs::read_to_string(new_dir.join("api_key.txt")).unwrap(), "new-key");
        // But new files should be copied.
        assert_eq!(fs::read_to_string(new_dir.join("journal.db")).unwrap(), "old-db");
    }
}
