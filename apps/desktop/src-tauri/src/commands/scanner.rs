use tauri::State;

use crate::safety::journal;
use crate::AppState;

/// Trigger an on-demand scan for the given scan type.
#[tauri::command]
pub async fn trigger_scan(
    state: State<'_, AppState>,
    scan_type: String,
) -> Result<String, String> {
    {
        // Unpause if paused.
        let mut paused = state.scanner_pause.lock().unwrap();
        paused.remove(&scan_type);
    }
    {
        let mut trigger = state.scanner_trigger.lock().unwrap();
        *trigger = Some(scan_type.clone());
    }
    Ok(format!("Scan triggered for {}", scan_type))
}

/// Pause a running scan.
#[tauri::command]
pub async fn pause_scan(
    state: State<'_, AppState>,
    scan_type: String,
) -> Result<(), String> {
    let mut paused = state.scanner_pause.lock().unwrap();
    paused.insert(scan_type);
    Ok(())
}

/// Resume a paused scan.
#[tauri::command]
pub async fn resume_scan(
    state: State<'_, AppState>,
    scan_type: String,
) -> Result<(), String> {
    let mut paused = state.scanner_pause.lock().unwrap();
    paused.remove(&scan_type);
    Ok(())
}

/// Get all scan job records (for the Diagnostics UI).
#[tauri::command]
pub async fn get_scan_jobs(
    state: State<'_, AppState>,
) -> Result<Vec<journal::ScanJobRecord>, String> {
    let conn = state.db.lock().await;
    journal::list_scan_jobs(&conn).map_err(|e| format!("Failed to list scan jobs: {}", e))
}
