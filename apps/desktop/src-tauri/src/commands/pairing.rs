use serde::{Deserialize, Serialize};
use tauri::State;

use crate::mobile_server::{self, PairingQrData};
use crate::safety::journal;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct PairingStatus {
    pub paired: bool,
    pub device_name: Option<String>,
    pub paired_at: Option<String>,
    pub server_port: u16,
}

#[tauri::command]
pub async fn generate_pairing_qr(state: State<'_, AppState>) -> Result<PairingQrData, String> {
    mobile_server::generate_pairing_data(&state.mobile_server, state.mobile_server_port)
        .await
        .map_err(|e| format!("Failed to generate pairing data: {}", e))
}

#[tauri::command]
pub async fn get_pairing_status(state: State<'_, AppState>) -> Result<PairingStatus, String> {
    let pairing = state.mobile_server.pairing.lock().await;
    Ok(PairingStatus {
        paired: pairing.paired_device.is_some(),
        device_name: pairing.paired_device.as_ref().map(|d| d.device_name.clone()),
        paired_at: pairing.paired_device.as_ref().map(|d| d.paired_at.clone()),
        server_port: state.mobile_server_port,
    })
}

#[tauri::command]
pub async fn unpair_device(state: State<'_, AppState>) -> Result<(), String> {
    let mut pairing = state.mobile_server.pairing.lock().await;
    pairing.paired_device = None;

    // Remove from DB
    let conn = state.db.lock().await;
    let _ = journal::set_setting(&conn, "paired_device", "");

    Ok(())
}
