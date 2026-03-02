use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-sonnet-4-20250514";
const API_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 4096;

// ── Request types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

// ── Response types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<ResponseBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ── API request body ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolDef>,
}

// ── LLM Client ─────────────────────────────────────────────────────────

pub struct LlmClient {
    api_key: String,
    client: reqwest::Client,
}

impl LlmClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    pub fn set_api_key(&mut self, key: String) {
        self.api_key = key;
    }

    pub fn has_api_key(&self) -> bool {
        !self.api_key.is_empty()
    }

    pub async fn send_message(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDef>,
        system: &str,
    ) -> Result<Response> {
        let body = ApiRequest {
            model: MODEL.to_string(),
            max_tokens: MAX_TOKENS,
            system: system.to_string(),
            messages,
            tools,
        };

        let resp = self
            .client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        let status = resp.status();
        if !status.is_success() {
            let error_body = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            anyhow::bail!("Anthropic API error ({}): {}", status, error_body);
        }

        let response: Response = resp
            .json()
            .await
            .context("Failed to parse Anthropic API response")?;

        Ok(response)
    }
}
