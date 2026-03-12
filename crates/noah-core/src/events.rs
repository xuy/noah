use serde_json::Value;

use crate::agent::orchestrator::ApprovalRequest;

/// Trait for emitting events to the frontend (or any observer).
///
/// This replaces the direct dependency on `tauri::AppHandle<R>` so the
/// orchestrator core can run without Tauri (e.g. in mobile, CLI, or tests).
pub trait EventEmitter: Send + Sync {
    fn emit_debug(&self, event_type: &str, summary: &str, detail: Value);
    fn emit_approval_request(&self, request: &ApprovalRequest) -> anyhow::Result<()>;
    fn emit_approval_timeout(&self, approval_id: &str);
}

/// No-op emitter that logs to stderr. Used by the debug runner and tests.
pub struct StderrEventEmitter;

impl EventEmitter for StderrEventEmitter {
    fn emit_debug(&self, event_type: &str, summary: &str, _detail: Value) {
        eprintln!("[debug:{}] {}", event_type, summary);
    }

    fn emit_approval_request(&self, request: &ApprovalRequest) -> anyhow::Result<()> {
        eprintln!(
            "[approval-request] {} — {} ({})",
            request.approval_id, request.tool_name, request.description
        );
        Ok(())
    }

    fn emit_approval_timeout(&self, approval_id: &str) {
        eprintln!("[approval-timeout] {}", approval_id);
    }
}
