use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use itman_tools::{SafetyTier, Tool, ToolResult};

struct FsmGetStateTool;
struct FsmEmitEventTool;
struct FsmNextTool;

#[async_trait]
impl Tool for FsmGetStateTool {
    fn name(&self) -> &str { "fsm_get_state" }
    fn description(&self) -> &str {
        "Get current FSM state for the active playbook, including next transitions."
    }
    fn input_schema(&self) -> Value {
        json!({
          "type":"object",
          "properties":{},
          "additionalProperties": false
        })
    }
    fn safety_tier(&self) -> SafetyTier { SafetyTier::ReadOnly }
    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        Ok(ToolResult::read_only(
            "FSM response is resolved by runtime.".to_string(),
            json!({"ok": true}),
        ))
    }
}

#[async_trait]
impl Tool for FsmEmitEventTool {
    fn name(&self) -> &str { "fsm_emit_event" }
    fn description(&self) -> &str {
        "Emit a domain event to advance playbook FSM state when milestone is reached."
    }
    fn input_schema(&self) -> Value {
        json!({
          "type":"object",
          "properties":{
            "event": {"type":"string", "description":"Event name declared by playbook FSM, e.g. install_verified."},
            "metadata": {"type":"object"}
          },
          "required": ["event"],
          "additionalProperties": false
        })
    }
    fn safety_tier(&self) -> SafetyTier { SafetyTier::ReadOnly }
    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        Ok(ToolResult::read_only(
            "FSM event accepted by runtime.".to_string(),
            json!({"ok": true}),
        ))
    }
}

#[async_trait]
impl Tool for FsmNextTool {
    fn name(&self) -> &str { "fsm_next" }
    fn description(&self) -> &str {
        "Get concise next-step requirements from active playbook FSM."
    }
    fn input_schema(&self) -> Value {
        json!({
          "type":"object",
          "properties":{},
          "additionalProperties": false
        })
    }
    fn safety_tier(&self) -> SafetyTier { SafetyTier::ReadOnly }
    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        Ok(ToolResult::read_only(
            "FSM next-step response is resolved by runtime.".to_string(),
            json!({"ok": true}),
        ))
    }
}

pub fn register_fsm_tools(router: &mut crate::agent::tool_router::ToolRouter) {
    router.register(Box::new(FsmGetStateTool));
    router.register(Box::new(FsmEmitEventTool));
    router.register(Box::new(FsmNextTool));
}
