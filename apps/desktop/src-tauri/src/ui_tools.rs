use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use itman_tools::{SafetyTier, Tool, ToolResult};

fn action_type_valid(v: &str) -> bool {
    matches!(v, "RUN_STEP" | "OPEN_SECURE_FORM" | "OPENCLAW_SECURE_CAPTURE")
}

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = s.find(start)?;
    let rest = &s[i + start.len()..];
    let j = rest.find(end)?;
    Some(rest[..j].trim())
}

fn normalize_action_from_input(input: &Value) -> Result<(String, String)> {
    if let Some(action) = input.get("action").and_then(|v| v.as_object()) {
        let label = action
            .get("label")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("missing action.label"))?;
        let action_type = action
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("missing action.type"))?;
        return Ok((label.to_string(), action_type.to_string()));
    }

    if let Some(action_raw) = input.get("action").and_then(|v| v.as_str()) {
        let action_type = extract_between(action_raw, r#"name="type">"#, "<")
            .or_else(|| extract_between(action_raw, "type>", "<"))
            .or_else(|| {
                if action_raw.to_uppercase().contains("OPEN_SECURE_FORM") {
                    Some("OPEN_SECURE_FORM")
                } else if action_raw.to_uppercase().contains("OPENCLAW_SECURE_CAPTURE") {
                    Some("OPEN_SECURE_FORM")
                } else if action_raw.to_uppercase().contains("RUN_STEP") {
                    Some("RUN_STEP")
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow!("missing action.type"))?;

        let label = input
            .get("label")
            .and_then(|v| v.as_str())
            .or_else(|| extract_between(action_raw, r#"name="label">"#, "<"))
            .or_else(|| extract_between(action_raw, "label>", "<"))
            .ok_or_else(|| anyhow!("missing action.label"))?;

        return Ok((label.to_string(), action_type.to_string()));
    }

    Err(anyhow!("missing action"))
}

pub fn ui_payload_from_tool_call(name: &str, input: &Value) -> Result<String> {
    match name {
        "ui_spa" => {
            let situation = input
                .get("situation_md")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing situation_md"))?;
            let plan = input
                .get("plan_md")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing plan_md"))?;
            let (label, action_type) = normalize_action_from_input(input)?;
            if !action_type_valid(&action_type) {
                return Err(anyhow!("invalid action.type"));
            }
            Ok(json!({
                "kind": "spa",
                "situation": situation,
                "plan": plan,
                "action": {
                    "label": label,
                    "type": action_type
                }
            })
            .to_string())
        }
        "ui_user_question" => {
            let questions = input
                .get("questions")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow!("missing questions"))?;
            let mut out = Vec::new();
            for q in questions {
                let question = q
                    .get("question_md")
                    .or_else(|| q.get("question"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("question missing question_md"))?;
                let header = q
                    .get("header")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("question missing header"))?;
                let options = q
                    .get("options")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("question missing options"))?;
                let mut out_options = Vec::new();
                for opt in options {
                    let label = opt
                        .get("label")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow!("option missing label"))?;
                    let description = opt
                        .get("description")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow!("option missing description"))?;
                    out_options.push(json!({ "label": label, "description": description }));
                }
                out.push(json!({
                    "question": question,
                    "header": header,
                    "multiSelect": q.get("multiSelect").and_then(|v| v.as_bool()).unwrap_or(false),
                    "options": out_options
                }));
            }
            Ok(json!({
                "kind": "user_question",
                "questions": out
            })
            .to_string())
        }
        "ui_info" => {
            let summary = input
                .get("summary_md")
                .or_else(|| input.get("summary"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing summary_md"))?;
            Ok(json!({ "kind": "info", "summary": summary }).to_string())
        }
        "ui_done" => {
            let summary = input
                .get("summary_md")
                .or_else(|| input.get("summary"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing summary_md"))?;
            Ok(json!({ "kind": "done", "summary": summary }).to_string())
        }
        _ => Err(anyhow!("not a ui tool")),
    }
}

struct UiSpaTool;
struct UiUserQuestionTool;
struct UiInfoTool;
struct UiDoneTool;

#[async_trait]
impl Tool for UiSpaTool {
    fn name(&self) -> &str { "ui_spa" }
    fn description(&self) -> &str {
        "Emit a Situation/Plan/Action (SPA) panel for the UI. `situation_md` and `plan_md` are Markdown strings."
    }
    fn input_schema(&self) -> Value {
        json!({
          "type":"object",
          "properties":{
            "situation_md":{"type":"string","description":"Situation text in Markdown format."},
            "plan_md":{"type":"string","description":"Plan text in Markdown format."},
            "action":{
              "type":"object",
              "properties":{
                "label":{"type":"string","description":"Human-readable button label, e.g. 'Open Secure Form'."},
                "type":{"type":"string","enum":["RUN_STEP","OPEN_SECURE_FORM"]}
              },
              "required":["label","type"],
              "additionalProperties":false
            }
          },
          "required":["situation_md","plan_md","action"],
          "additionalProperties":false
        })
    }
    fn safety_tier(&self) -> SafetyTier { SafetyTier::ReadOnly }
    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let payload = ui_payload_from_tool_call(self.name(), input)?;
        Ok(ToolResult::read_only(payload.clone(), serde_json::from_str(&payload)?))
    }
}

#[async_trait]
impl Tool for UiUserQuestionTool {
    fn name(&self) -> &str { "ui_user_question" }
    fn description(&self) -> &str {
        "Ask one or more structured user questions in the UI with selectable options."
    }
    fn input_schema(&self) -> Value {
        json!({
          "type":"object",
          "properties":{
            "questions":{
              "type":"array",
              "minItems":1,
              "items":{
                "type":"object",
                "properties":{
                  "id":{"type":"string"},
                  "header":{"type":"string"},
                  "question_md":{"type":"string","description":"Question prompt in Markdown format."},
                  "multiSelect":{"type":"boolean"},
                  "options":{
                    "type":"array",
                    "minItems":2,
                    "items":{
                      "type":"object",
                      "properties":{
                        "label":{"type":"string"},
                        "description":{"type":"string"}
                      },
                      "required":["label","description"],
                      "additionalProperties":false
                    }
                  }
                },
                "required":["header","question_md","options"],
                "additionalProperties":false
              }
            }
          },
          "required":["questions"],
          "additionalProperties":false
        })
    }
    fn safety_tier(&self) -> SafetyTier { SafetyTier::ReadOnly }
    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let payload = ui_payload_from_tool_call(self.name(), input)?;
        Ok(ToolResult::read_only(payload.clone(), serde_json::from_str(&payload)?))
    }
}

#[async_trait]
impl Tool for UiInfoTool {
    fn name(&self) -> &str { "ui_info" }
    fn description(&self) -> &str { "Emit an informational card in Markdown." }
    fn input_schema(&self) -> Value {
        json!({
          "type":"object",
          "properties":{"summary_md":{"type":"string","description":"Summary in Markdown format."}},
          "required":["summary_md"],
          "additionalProperties":false
        })
    }
    fn safety_tier(&self) -> SafetyTier { SafetyTier::ReadOnly }
    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let payload = ui_payload_from_tool_call(self.name(), input)?;
        Ok(ToolResult::read_only(payload.clone(), serde_json::from_str(&payload)?))
    }
}

#[async_trait]
impl Tool for UiDoneTool {
    fn name(&self) -> &str { "ui_done" }
    fn description(&self) -> &str { "Emit a completion card in Markdown." }
    fn input_schema(&self) -> Value {
        json!({
          "type":"object",
          "properties":{"summary_md":{"type":"string","description":"Completion summary in Markdown format."}},
          "required":["summary_md"],
          "additionalProperties":false
        })
    }
    fn safety_tier(&self) -> SafetyTier { SafetyTier::ReadOnly }
    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let payload = ui_payload_from_tool_call(self.name(), input)?;
        Ok(ToolResult::read_only(payload.clone(), serde_json::from_str(&payload)?))
    }
}

pub fn register_ui_tools(router: &mut crate::agent::tool_router::ToolRouter) {
    router.register(Box::new(UiSpaTool));
    router.register(Box::new(UiUserQuestionTool));
    router.register(Box::new(UiInfoTool));
    router.register(Box::new(UiDoneTool));
}
