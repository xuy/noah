use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-sonnet-4-20250514";
const TITLE_MODEL: &str = "claude-haiku-4-5-20251001";
const API_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 4096;
const REQUEST_TIMEOUT_SECS: u64 = 90;

// ── Auth mode ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum AuthMode {
    ApiKey(String),
    Proxy { base_url: String, token: String },
}

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

#[derive(Clone)]
pub struct LlmClient {
    auth: AuthMode,
    client: reqwest::Client,
}

/// Map an HTTP status code from the Anthropic API to a user-friendly error message.
fn friendly_api_error(status: reqwest::StatusCode, body: &str) -> String {
    match status.as_u16() {
        401 => "Your API key is invalid or has been revoked. Please check it in Settings.".to_string(),
        403 => "Your API key doesn't have permission for this request. Check your Anthropic account.".to_string(),
        429 => "Too many requests — Claude is rate-limited. Wait a moment and try again.".to_string(),
        500 | 502 | 503 => "Claude is having temporary issues. Please try again in a minute.".to_string(),
        529 => "Claude is currently overloaded. Please try again in a few minutes.".to_string(),
        _ => format!("Unexpected API error ({}): {}", status, body),
    }
}

impl LlmClient {
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            auth: AuthMode::ApiKey(api_key),
            client,
        }
    }

    pub fn with_auth(auth: AuthMode) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { auth, client }
    }

    pub fn set_api_key(&mut self, key: String) {
        self.auth = AuthMode::ApiKey(key);
    }

    pub fn set_auth(&mut self, auth: AuthMode) {
        self.auth = auth;
    }

    pub fn has_api_key(&self) -> bool {
        self.has_auth()
    }

    pub fn has_auth(&self) -> bool {
        match &self.auth {
            AuthMode::ApiKey(key) => !key.is_empty(),
            AuthMode::Proxy { token, .. } => !token.is_empty(),
        }
    }

    pub fn auth_mode_name(&self) -> &str {
        match &self.auth {
            AuthMode::ApiKey(_) => "api_key",
            AuthMode::Proxy { .. } => "proxy",
        }
    }

    /// Get the API URL based on auth mode.
    fn api_url(&self) -> String {
        match &self.auth {
            AuthMode::ApiKey(_) => ANTHROPIC_API_URL.to_string(),
            AuthMode::Proxy { base_url, .. } => format!("{}/v1/messages", base_url.trim_end_matches('/')),
        }
    }

    /// Apply auth headers to a request builder.
    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.auth {
            AuthMode::ApiKey(key) => builder.header("x-api-key", key),
            AuthMode::Proxy { token, .. } => builder.header("Authorization", format!("Bearer {}", token)),
        }
    }

    /// Generate a short session title from the first user message using a fast, cheap model.
    pub async fn generate_title(&self, user_message: &str) -> Result<String> {
        let body = ApiRequest {
            model: TITLE_MODEL.to_string(),
            max_tokens: 30,
            system: "Generate a short title (max 6 words) for a computer support session based on the user's message. Output only the title, nothing else. No quotes.".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text(user_message.to_string()),
            }],
            tools: vec![],
        };

        let builder = self
            .client
            .post(self.api_url())
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body);
        let resp = self.apply_auth(builder)
            .send()
            .await
            .context("Title generation request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let error_body = resp.text().await.unwrap_or_default();
            anyhow::bail!("{}", friendly_api_error(status, &error_body));
        }

        let response: Response = resp
            .json()
            .await
            .context("Failed to parse title response")?;

        let title = response
            .content
            .iter()
            .find_map(|b| match b {
                ResponseBlock::Text { text } => Some(text.trim().to_string()),
                _ => None,
            })
            .unwrap_or_else(|| user_message.chars().take(60).collect());

        Ok(title)
    }

    /// Generate a brief session summary using Haiku.
    pub async fn generate_session_summary(&self, messages_text: &str) -> Result<String> {
        let body = ApiRequest {
            model: TITLE_MODEL.to_string(),
            max_tokens: 200,
            system: "Summarize this IT support session in 2-3 short bullet points. Focus on: what was the problem, what was done, and the outcome. Be concise. Use plain language.".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text(messages_text.to_string()),
            }],
            tools: vec![],
        };

        let builder = self
            .client
            .post(self.api_url())
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body);
        let resp = self.apply_auth(builder)
            .send()
            .await
            .context("Summary generation request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let error_body = resp.text().await.unwrap_or_default();
            anyhow::bail!("{}", friendly_api_error(status, &error_body));
        }

        let response: Response = resp
            .json()
            .await
            .context("Failed to parse summary response")?;

        let summary = response
            .content
            .iter()
            .find_map(|b| match b {
                ResponseBlock::Text { text } => Some(text.trim().to_string()),
                _ => None,
            })
            .unwrap_or_else(|| "Session completed.".to_string());

        Ok(summary)
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

        let max_retries = 3u32;
        let mut last_error: Option<anyhow::Error> = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                // Exponential backoff: 1s, 2s, 4s
                let delay = std::time::Duration::from_secs(1 << (attempt - 1));
                tokio::time::sleep(delay).await;
            }

            let builder = self
                .client
                .post(self.api_url())
                .header("anthropic-version", API_VERSION)
                .header("content-type", "application/json")
                .json(&body);
            let resp = match self.apply_auth(builder)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let err = if e.is_timeout() {
                        anyhow::anyhow!("Claude is taking too long to respond. Please try again.")
                    } else {
                        anyhow::anyhow!("Can't reach Claude — check your internet connection.")
                    };
                    // Network errors are retryable
                    last_error = Some(err);
                    continue;
                }
            };

            let status = resp.status();
            if status.is_success() {
                let response: Response = resp
                    .json()
                    .await
                    .context("Failed to parse Anthropic API response")?;
                return Ok(response);
            }

            let error_body = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());

            // Only retry on retryable status codes
            let retryable = matches!(status.as_u16(), 429 | 500 | 502 | 503 | 529);
            let friendly = friendly_api_error(status, &error_body);

            if !retryable || attempt == max_retries {
                anyhow::bail!("{}", friendly);
            }

            last_error = Some(anyhow::anyhow!("{}", friendly));
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Request failed after retries")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_friendly_api_error_401() {
        let msg = friendly_api_error(reqwest::StatusCode::UNAUTHORIZED, "");
        assert!(msg.contains("API key"), "401 should mention API key: {}", msg);
        assert!(msg.contains("Settings"), "401 should mention Settings: {}", msg);
    }

    #[test]
    fn test_friendly_api_error_429() {
        let msg = friendly_api_error(reqwest::StatusCode::TOO_MANY_REQUESTS, "");
        assert!(msg.contains("Too many requests") || msg.contains("rate-limited"),
            "429 should mention rate limit: {}", msg);
    }

    #[test]
    fn test_friendly_api_error_500() {
        let msg = friendly_api_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "");
        assert!(msg.contains("temporary issues") || msg.contains("try again"),
            "500 should be friendly: {}", msg);
    }

    #[test]
    fn test_friendly_api_error_502() {
        let msg = friendly_api_error(reqwest::StatusCode::BAD_GATEWAY, "");
        assert!(msg.contains("temporary issues"),
            "502 should be friendly: {}", msg);
    }

    #[test]
    fn test_friendly_api_error_503() {
        let msg = friendly_api_error(reqwest::StatusCode::SERVICE_UNAVAILABLE, "");
        assert!(msg.contains("temporary issues"),
            "503 should be friendly: {}", msg);
    }

    #[test]
    fn test_friendly_api_error_529_overloaded() {
        let msg = friendly_api_error(
            reqwest::StatusCode::from_u16(529).unwrap(),
            "",
        );
        assert!(msg.contains("overloaded"),
            "529 should mention overloaded: {}", msg);
    }

    #[test]
    fn test_friendly_api_error_unknown_includes_status() {
        let msg = friendly_api_error(reqwest::StatusCode::IM_A_TEAPOT, "brew coffee");
        assert!(msg.contains("418"), "Unknown error should include status code: {}", msg);
        assert!(msg.contains("brew coffee"), "Unknown error should include body: {}", msg);
    }

    #[test]
    fn test_friendly_api_error_403() {
        let msg = friendly_api_error(reqwest::StatusCode::FORBIDDEN, "");
        assert!(msg.contains("permission"),
            "403 should mention permission: {}", msg);
    }

    #[test]
    fn test_client_has_timeout() {
        // Verify the client is constructed with a timeout (it won't use default infinite).
        let client = LlmClient::new("test-key".to_string());
        assert!(client.has_api_key());
        // Can't introspect reqwest timeout, but at least verify construction doesn't panic.
    }

    #[test]
    fn test_client_set_and_check_api_key() {
        let mut client = LlmClient::new(String::new());
        assert!(!client.has_api_key());
        client.set_api_key("sk-ant-test".to_string());
        assert!(client.has_api_key());
    }

    #[test]
    fn test_no_raw_error_messages() {
        // All mapped status codes should NOT contain "Anthropic API error" (the old raw format).
        for status in [401u16, 403, 429, 500, 502, 503, 529] {
            let msg = friendly_api_error(
                reqwest::StatusCode::from_u16(status).unwrap(),
                "raw body",
            );
            assert!(!msg.contains("Anthropic API error"),
                "Status {} should not show raw error: {}", status, msg);
        }
    }
}
