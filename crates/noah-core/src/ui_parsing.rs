use serde::{Deserialize, Serialize};

// ── Types ──

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssistantActionType {
    RunStep,
    WaitForUser,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantQuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantTextInput {
    pub placeholder: Option<String>,
    pub default: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantSecureInput {
    pub placeholder: Option<String>,
    pub secret_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantQuestion {
    pub question: String,
    pub header: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<AssistantQuestionOption>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_input: Option<AssistantTextInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure_input: Option<AssistantSecureInput>,
    #[serde(rename = "multiSelect", skip_serializing_if = "Option::is_none")]
    pub multi_select: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantCardAction {
    pub label: String,
    #[serde(rename = "type")]
    pub action_type: AssistantActionType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaybookProgress {
    pub step: u32,
    pub total: u32,
    pub label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantSpaUi {
    pub kind: String,
    pub situation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    pub action: AssistantCardAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<PlaybookProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qr_data: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantUserQuestionUi {
    pub kind: String,
    pub questions: Vec<AssistantQuestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<PlaybookProgress>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssistantInfoUi {
    pub kind: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<PlaybookProgress>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum AssistantUiPayload {
    Spa(AssistantSpaUi),
    UserQuestion(AssistantUserQuestionUi),
    Done(AssistantInfoUi),
    Info(AssistantInfoUi),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SendMessageV2Result {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_ui: Option<AssistantUiPayload>,
}

// ── Parsing ──

fn parse_between<'a>(s: &'a str, start: &str, end: &str) -> Option<String> {
    let i = s.find(start)?;
    let rest = &s[i + start.len()..];
    let j = rest.find(end)?;
    Some(rest[..j].trim().to_string())
}

fn parse_action_label(s: &str) -> Option<String> {
    let marker = "[ACTION:";
    let i = s.find(marker)?;
    let rest = &s[i + marker.len()..];
    let j = rest.find(']')?;
    Some(rest[..j].trim().to_string())
}

fn parse_progress(v: &serde_json::Value) -> Option<PlaybookProgress> {
    let p = v.get("progress")?;
    Some(PlaybookProgress {
        step: p.get("step")?.as_u64()? as u32,
        total: p.get("total")?.as_u64()? as u32,
        label: p.get("label")?.as_str()?.to_string(),
    })
}

fn parse_assistant_ui_json(text: &str) -> Option<AssistantUiPayload> {
    let candidate = if let Some(start) = text.find("```json") {
        let rest = &text[start + "```json".len()..];
        let end = rest.find("```")?;
        rest[..end].trim().to_string()
    } else if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        text[start..=end].trim().to_string()
    } else {
        text.trim().to_string()
    };
    if !candidate.starts_with('{') {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&candidate).ok()?;
    let kind = v.get("kind")?.as_str()?.to_lowercase();
    let progress = parse_progress(&v);
    match kind.as_str() {
        "done" | "info" => {
            let summary = v.get("summary")?.as_str()?.to_string();
            Some(if kind == "done" {
                AssistantUiPayload::Done(AssistantInfoUi { kind, summary, progress })
            } else {
                AssistantUiPayload::Info(AssistantInfoUi { kind, summary, progress })
            })
        }
        "spa" => {
            let situation = v.get("situation")?.as_str()?.to_string();
            let plan = v.get("plan").and_then(|v| v.as_str()).map(|s| s.to_string());
            let action_v = v.get("action")?;
            let label = action_v.get("label")?.as_str()?.to_string();
            let action_type = action_v
                .get("type")
                .and_then(|x| x.as_str())
                .map(|s| s.to_uppercase())
                .and_then(|s| match s.as_str() {
                    "RUN_STEP" => Some(AssistantActionType::RunStep),
                    "WAIT_FOR_USER" => Some(AssistantActionType::WaitForUser),
                    _ => None,
                })
                .unwrap_or(AssistantActionType::RunStep);
            let qr_data = v.get("qr_data").and_then(|v| v.as_str()).map(|s| s.to_string());
            Some(AssistantUiPayload::Spa(AssistantSpaUi {
                kind: "spa".to_string(),
                situation,
                plan,
                action: AssistantCardAction {
                    label,
                    action_type,
                },
                progress,
                qr_data,
            }))
        }
        "user_question" => {
            let questions = v
                .get("questions")
                .and_then(|q| q.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|q| {
                            let question = q.get("question")?.as_str()?.to_string();
                            let header = q.get("header")?.as_str()?.to_string();
                            let multi_select = q
                                .get("multiSelect")
                                .and_then(|v| v.as_bool());

                            let options = q
                                .get("options")
                                .and_then(|o| o.as_array())
                                .map(|opts| {
                                    opts.iter()
                                        .filter_map(|o| {
                                            Some(AssistantQuestionOption {
                                                label: o.get("label")?.as_str()?.to_string(),
                                                description: o.get("description")?.as_str()?.to_string(),
                                            })
                                        })
                                        .collect::<Vec<_>>()
                                });

                            let text_input = q.get("text_input").map(|ti| {
                                AssistantTextInput {
                                    placeholder: ti.get("placeholder").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                    default: ti.get("default").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                }
                            });

                            let secure_input = q.get("secure_input").and_then(|si| {
                                Some(AssistantSecureInput {
                                    placeholder: si.get("placeholder").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                    secret_name: si.get("secret_name")?.as_str()?.to_string(),
                                })
                            });

                            Some(AssistantQuestion {
                                question,
                                header,
                                options,
                                text_input,
                                secure_input,
                                multi_select,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Some(AssistantUiPayload::UserQuestion(AssistantUserQuestionUi {
                kind: "user_question".to_string(),
                questions,
                progress,
            }))
        }
        _ => None,
    }
}

pub fn parse_assistant_ui(text: &str) -> Option<AssistantUiPayload> {
    if let Some(ui) = parse_assistant_ui_json(text) {
        return Some(ui);
    }
    if text.contains("[DONE]") {
        let summary = text
            .split_once("[DONE]")
            .map(|(_, s)| s.trim().to_string())
            .unwrap_or_default();
        return Some(AssistantUiPayload::Done(AssistantInfoUi {
            kind: "done".to_string(),
            summary,
            progress: None,
        }));
    }
    if text.contains("[INFO]") {
        let summary = text
            .split_once("[INFO]")
            .map(|(_, s)| s.trim().to_string())
            .unwrap_or_default();
        return Some(AssistantUiPayload::Info(AssistantInfoUi {
            kind: "info".to_string(),
            summary,
            progress: None,
        }));
    }

    let situation = parse_between(text, "[SITUATION]", "[PLAN]")?;
    let plan = parse_between(text, "[PLAN]", "[ACTION:")?;
    let label = parse_action_label(text)?;

    Some(AssistantUiPayload::Spa(AssistantSpaUi {
        kind: "spa".to_string(),
        situation,
        plan: Some(plan),
        action: AssistantCardAction {
            label,
            action_type: AssistantActionType::RunStep,
        },
        progress: None,
        qr_data: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_card_ui() {
        let text = "[SITUATION]\nA\n[PLAN]\nB\n[ACTION:Do it]";
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(card.situation, "A");
                assert_eq!(card.plan, Some("B".to_string()));
                assert_eq!(card.action.label, "Do it");
                assert_eq!(card.action.action_type, AssistantActionType::RunStep);
            }
            _ => panic!("expected card ui"),
        }
    }

    #[test]
    fn parses_done_ui() {
        let text = "[DONE]\nAll set";
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Done(done)) => assert_eq!(done.summary, "All set"),
            _ => panic!("expected done ui"),
        }
    }

    #[test]
    fn parses_json_card_ui() {
        let text = r#"{"kind":"spa","situation":"CPU is high","plan":"Stop heavy app","action":{"label":"Stop App","type":"RUN_STEP"}}"#;
        let ui = parse_assistant_ui(text);
        match ui {
            Some(AssistantUiPayload::Spa(card)) => {
                assert_eq!(card.action.label, "Stop App");
                assert_eq!(card.situation, "CPU is high");
            }
            _ => panic!("expected json card ui"),
        }
    }

    #[test]
    fn parses_plain_text_returns_none() {
        let text = "Hello, how can I help you today?";
        let ui = parse_assistant_ui(text);
        assert!(ui.is_none(), "plain text should return None");
    }
}
