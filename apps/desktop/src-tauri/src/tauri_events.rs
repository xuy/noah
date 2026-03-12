use serde::Serialize;
use serde_json::Value;
use tauri::Emitter;

use noah_core::agent::orchestrator::ApprovalRequest;
use noah_core::events::EventEmitter;

/// A debug event emitted to the frontend for observability.
#[derive(Debug, Clone, Serialize)]
struct DebugEvent {
    timestamp: String,
    event_type: String,
    summary: String,
    detail: Value,
}

/// Implements `EventEmitter` for Tauri's `AppHandle`, bridging
/// noah-core's platform-agnostic events to Tauri's event system.
pub struct TauriEventEmitter<R: tauri::Runtime> {
    app_handle: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> TauriEventEmitter<R> {
    pub fn new(app_handle: tauri::AppHandle<R>) -> Self {
        Self { app_handle }
    }
}

impl<R: tauri::Runtime> EventEmitter for TauriEventEmitter<R> {
    fn emit_debug(&self, event_type: &str, summary: &str, detail: Value) {
        let event = DebugEvent {
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            event_type: event_type.to_string(),
            summary: summary.to_string(),
            detail,
        };
        let _ = self.app_handle.emit("debug-log", &event);
    }

    fn emit_approval_request(&self, request: &ApprovalRequest) -> anyhow::Result<()> {
        self.app_handle
            .emit("approval-request", request)
            .map_err(|e| anyhow::anyhow!("Failed to emit approval-request: {}", e))
    }

    fn emit_approval_timeout(&self, approval_id: &str) {
        let _ = self.app_handle.emit(
            "approval-timeout",
            serde_json::json!({ "approval_id": approval_id }),
        );
    }
}
