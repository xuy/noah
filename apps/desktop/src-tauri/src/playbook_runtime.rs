use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::agent::llm_client::{ContentBlock, Message, MessageContent};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FsmStateSpec {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub llm_guidance: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FsmTransitionSpec {
    #[serde(default)]
    pub id: Option<String>,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub llm_guidance: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FsmTerminalSpec {
    #[serde(default)]
    pub states: Vec<String>,
    #[serde(default)]
    pub goal: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FsmGuards {
    #[serde(default)]
    pub blocked_commands: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FsmSpec {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub machine: String,
    pub initial_state: String,
    #[serde(default)]
    pub states: HashMap<String, FsmStateSpec>,
    #[serde(default)]
    pub events: HashMap<String, Value>,
    #[serde(default)]
    pub transitions: Vec<FsmTransitionSpec>,
    #[serde(default)]
    pub terminal: FsmTerminalSpec,
    #[serde(default)]
    pub guards: FsmGuards,
}

#[derive(Debug, Clone, Serialize)]
pub struct FsmSnapshot {
    pub machine: String,
    pub state: String,
    pub terminal: bool,
    pub state_summary: String,
    pub next: Vec<FsmNextTransition>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FsmNextTransition {
    pub to: String,
    pub goal: String,
    pub acceptance: Vec<String>,
    pub triggers: Vec<String>,
}

fn extract_playbook_name_from_frontmatter(content: &str) -> Option<String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_first = &trimmed[3..];
    let end = after_first.find("\n---")?;
    let yaml = &after_first[..end];
    yaml.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("name:").map(|v| v.trim().to_string()))
}

fn load_playbook_content(knowledge_dir: &Path, playbook_name: &str) -> Option<String> {
    let dir = knowledge_dir.join("playbooks");
    let entries = std::fs::read_dir(&dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "md") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let stem_match = path
            .file_stem()
            .map(|s| s.to_string_lossy() == playbook_name)
            .unwrap_or(false);
        let name_match = extract_playbook_name_from_frontmatter(&content)
            .map(|n| n == playbook_name)
            .unwrap_or(false);
        if stem_match || name_match {
            return Some(content);
        }
    }
    None
}

fn extract_fsm_block(markdown: &str) -> Option<String> {
    let idx = markdown.find("## FSM")?;
    let after = &markdown[idx..];
    let fence_start = after.find("```")?;
    let after_fence = &after[fence_start + 3..];
    let first_newline = after_fence.find('\n')?;
    let body_start = first_newline + 1;
    let rest = &after_fence[body_start..];
    let end = rest.find("\n```")?;
    Some(rest[..end].trim().to_string())
}

fn load_fsm_spec(knowledge_dir: &Path, playbook_name: &str) -> Option<FsmSpec> {
    let content = load_playbook_content(knowledge_dir, playbook_name)?;
    let fsm_raw = extract_fsm_block(&content)?;
    serde_json::from_str::<FsmSpec>(&fsm_raw).ok()
}

fn push_text_event(events: &mut Vec<String>, text: &str) {
    let lower = text.trim().to_lowercase();
    if ["go ahead", "yes", "okay", "ok", "continue"].contains(&lower.as_str()) {
        events.push("user_confirm".to_string());
    }
    if lower.contains("skip this optional step") {
        events.push("user_skip_optional".to_string());
    }
    if lower.contains("credentials were submitted via noah secure form") {
        events.push("secure_form_submitted".to_string());
    }
}

fn collect_events(messages: &[Message]) -> Vec<String> {
    let mut events = Vec::new();
    for message in messages {
        if let MessageContent::Text(text) = &message.content {
            push_text_event(&mut events, text);
        }
        if let MessageContent::Blocks(blocks) = &message.content {
            for block in blocks {
                if let ContentBlock::ToolUse { name, input, .. } = block {
                    if name == "fsm_emit_event" {
                        if let Some(event) = input.get("event").and_then(|v| v.as_str()) {
                            let e = event.trim();
                            if !e.is_empty() {
                                events.push(e.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    events
}

fn compute_state(spec: &FsmSpec, events: &[String]) -> String {
    let mut state = spec.initial_state.clone();
    for event in events {
        if let Some(next) = spec
            .transitions
            .iter()
            .find(|t| t.from == state && t.triggers.iter().any(|tr| tr == event))
        {
            state = next.to.clone();
        }
    }
    state
}

pub fn snapshot_for_playbook(
    active_playbook: Option<&str>,
    messages: &[Message],
    knowledge_dir: &Path,
) -> Option<FsmSnapshot> {
    let name = active_playbook?;
    let spec = load_fsm_spec(knowledge_dir, name)?;
    let events = collect_events(messages);
    let state = compute_state(&spec, &events);
    let terminal = spec.terminal.states.iter().any(|s| s == &state);
    let state_summary = spec
        .states
        .get(&state)
        .map(|s| s.summary.clone())
        .unwrap_or_default();
    let next = spec
        .transitions
        .iter()
        .filter(|t| t.from == state)
        .map(|t| FsmNextTransition {
            to: t.to.clone(),
            goal: t.goal.clone(),
            acceptance: t.acceptance.clone(),
            triggers: t.triggers.clone(),
        })
        .collect::<Vec<_>>();

    Some(FsmSnapshot {
        machine: if spec.machine.is_empty() {
            name.to_string()
        } else {
            spec.machine
        },
        state,
        terminal,
        state_summary,
        next,
    })
}

pub fn governance_overlay(
    active_playbook: Option<&str>,
    messages: &[Message],
    knowledge_dir: &Path,
) -> String {
    let Some(name) = active_playbook else {
        return String::new();
    };
    let Some(snapshot) = snapshot_for_playbook(active_playbook, messages, knowledge_dir) else {
        return format!(
            "\n\n## Playbook Governance Mode\nActive playbook: `{}`.\nTreat this playbook as binding protocol until its completion criteria are met.",
            name
        );
    };

    let mut out = format!(
        "\n\n## Playbook Governance Mode\nActive playbook: `{}`\nFSM state: `{}`\nState summary: {}\n",
        snapshot.machine,
        snapshot.state,
        if snapshot.state_summary.is_empty() {
            "(no summary provided)"
        } else {
            snapshot.state_summary.as_str()
        }
    );

    if snapshot.terminal {
        out.push_str("Terminal condition reached for FSM.\n");
    } else if !snapshot.next.is_empty() {
        out.push_str("Possible next transitions:\n");
        for t in snapshot.next.iter().take(3) {
            out.push_str(&format!(
                "- `{}` -> `{}`: {} | triggers: {}\n",
                snapshot.state,
                t.to,
                if t.goal.is_empty() { "(goal unspecified)" } else { &t.goal },
                if t.triggers.is_empty() { "(none)".to_string() } else { t.triggers.join(", ") }
            ));
            if !t.acceptance.is_empty() {
                out.push_str(&format!("  acceptance: {}\n", t.acceptance.join(" | ")));
            }
        }
    }

    out.push_str("When a milestone is reached, call `fsm_emit_event` before your final ui_* response.");
    out
}

pub fn validate_final_response(
    active_playbook: Option<&str>,
    messages: &[Message],
    _user_message: &str,
    visible_text: &str,
    knowledge_dir: &Path,
) -> Option<String> {
    let snapshot = snapshot_for_playbook(active_playbook, messages, knowledge_dir)?;
    let lower = visible_text.to_lowercase();
    let is_done = lower.contains("[done]")
        || lower.contains(r#"\"kind\":\"done\""#)
        || lower.contains(r#"\"kind\": \"done\""#);
    if is_done && !snapshot.terminal {
        return Some(format!(
            "Policy guard: `ui_done` is not allowed before terminal state. Current state is `{}`.",
            snapshot.state
        ));
    }
    None
}

pub fn blocked_shell_command_feedback(
    active_playbook: Option<&str>,
    messages: &[Message],
    command: &str,
    knowledge_dir: &Path,
) -> Option<String> {
    let name = active_playbook?;
    let snapshot = snapshot_for_playbook(active_playbook, messages, knowledge_dir)?;
    let spec = load_fsm_spec(knowledge_dir, name)?;
    let blocked = spec
        .guards
        .blocked_commands
        .get(&snapshot.state)
        .or_else(|| spec.guards.blocked_commands.get("*"))?;
    let lower = command.to_lowercase();
    let hit = blocked.iter().find(|pattern| lower.contains(&pattern.to_lowercase()))?;
    Some(format!(
        "COMMAND NOT EXECUTED: blocked by FSM guard in state `{}` (pattern: `{}`). Choose another approach and continue with one ui_* response.",
        snapshot.state, hit
    ))
}

pub fn fsm_tool_response(
    tool_name: &str,
    active_playbook: Option<&str>,
    messages: &[Message],
    _input: &Value,
    knowledge_dir: &Path,
) -> Option<String> {
    match tool_name {
        "fsm_get_state" | "fsm_emit_event" | "fsm_next" => {
            let snapshot = snapshot_for_playbook(active_playbook, messages, knowledge_dir)?;
            Some(json!(snapshot).to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fsm_block() {
        let md = "## FSM\n```json\n{\"version\":1,\"machine\":\"x\",\"initial_state\":\"A\",\"states\":{},\"events\":{},\"transitions\":[],\"terminal\":{\"states\":[\"A\"]},\"guards\":{}}\n```";
        let raw = extract_fsm_block(md).expect("fsm block");
        let parsed: FsmSpec = serde_json::from_str(&raw).expect("valid json");
        assert_eq!(parsed.initial_state, "A");
    }

    #[test]
    fn transitions_on_events() {
        let spec = FsmSpec {
            initial_state: "A".to_string(),
            transitions: vec![FsmTransitionSpec {
                from: "A".to_string(),
                to: "B".to_string(),
                triggers: vec!["go".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let state = compute_state(&spec, &["go".to_string()]);
        assert_eq!(state, "B");
    }
}
