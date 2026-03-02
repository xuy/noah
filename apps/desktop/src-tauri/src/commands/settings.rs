use tauri::State;

use crate::AppState;

#[tauri::command]
pub async fn has_api_key(state: State<'_, AppState>) -> Result<bool, String> {
    let orch = state.orchestrator.lock().await;
    Ok(orch.has_api_key())
}

#[tauri::command]
pub async fn set_api_key(state: State<'_, AppState>, api_key: String) -> Result<(), String> {
    // Save to disk so it persists across restarts.
    crate::save_api_key(&state.app_dir, &api_key)?;

    // Update the in-memory LLM client.
    let mut orch = state.orchestrator.lock().await;
    orch.set_api_key(api_key);

    Ok(())
}

#[tauri::command]
pub async fn get_app_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}
