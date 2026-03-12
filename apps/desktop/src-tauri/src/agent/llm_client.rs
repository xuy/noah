use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-sonnet-4-20250514";
const TITLE_MODEL: &str = "claude-haiku-4-5-20251001";
const API_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 4096;
/// HTTP request timeout — higher for local LLMs which are slower.
fn request_timeout_secs() -> u64 {
    std::env::var("NOAH_TIMEOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if env_api_url().is_some() { 300 } else { 90 })
}

/// Override API URL via NOAH_API_URL env var (e.g. "http://127.0.0.1:8082").
fn env_api_url() -> Option<String> {
    std::env::var("NOAH_API_URL").ok().filter(|s| !s.is_empty())
}

/// Override model via NOAH_MODEL env var (e.g. "local").
fn effective_model() -> String {
    std::env::var("NOAH_MODEL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| MODEL.to_string())
}

fn effective_model_or(default: &str) -> String {
    std::env::var("NOAH_MODEL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn context_summary_model() -> String {
    TITLE_MODEL.to_string()
}

// ── Diagnostic analysis result ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticAnalysis {
    pub noteworthy: bool,
    pub headline: String,
    pub detail: String,
}

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
    system: Vec<crate::agent::prompts::SystemBlock>,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolDef>,
}

/// Wrap a plain system prompt string into a single SystemBlock (no caching).
fn system_text(s: &str) -> Vec<crate::agent::prompts::SystemBlock> {
    vec![crate::agent::prompts::SystemBlock {
        block_type: "text",
        text: s.to_string(),
        cache_control: None,
    }]
}

// ── LLM Client ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct LlmClient {
    auth: AuthMode,
    client: reqwest::Client,
}

/// Strip markdown code fences (```json ... ```) from an LLM response.
fn strip_markdown_fences(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        // Remove first line (```json or ```) and last line (```).
        let inner = trimmed.strip_prefix("```").unwrap_or(trimmed);
        // Skip the language tag on the first line.
        let inner = match inner.find('\n') {
            Some(pos) => &inner[pos + 1..],
            None => inner,
        };
        // Remove trailing ```.
        let inner = inner.strip_suffix("```").unwrap_or(inner);
        inner.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

/// Map an HTTP status code from the Anthropic API to a user-friendly error message.
fn friendly_api_error(status: reqwest::StatusCode, body: &str) -> String {
    match status.as_u16() {
        401 => {
            "Your API key is invalid or has been revoked. Please check it in Settings.".to_string()
        }
        403 => {
            "Your API key doesn't have permission for this request. Check your Anthropic account."
                .to_string()
        }
        429 => {
            "Too many requests — Claude is rate-limited. Wait a moment and try again.".to_string()
        }
        500 | 502 | 503 => {
            "Claude is having temporary issues. Please try again in a minute.".to_string()
        }
        529 => "Claude is currently overloaded. Please try again in a few minutes.".to_string(),
        _ => format!("Unexpected API error ({}): {}", status, body),
    }
}

pub fn is_context_limit_error(status: reqwest::StatusCode, body: &str) -> bool {
    if !matches!(status.as_u16(), 400 | 413) {
        return false;
    }

    let body = body.to_ascii_lowercase();
    [
        "context window",
        "context_length",
        "prompt is too long",
        "prompt too long",
        "too many tokens",
        "input is too long",
        "maximum context length",
    ]
    .iter()
    .any(|needle| body.contains(needle))
}

impl LlmClient {
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(request_timeout_secs()))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            auth: AuthMode::ApiKey(api_key),
            client,
        }
    }

    pub fn with_auth(auth: AuthMode) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(request_timeout_secs()))
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
        // Local server override needs no auth.
        if env_api_url().is_some() {
            return true;
        }
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

    /// Get the API URL based on auth mode (env override takes priority).
    fn api_url(&self) -> String {
        if let Some(url) = env_api_url() {
            return format!("{}/v1/messages", url.trim_end_matches('/'));
        }
        match &self.auth {
            AuthMode::ApiKey(_) => ANTHROPIC_API_URL.to_string(),
            AuthMode::Proxy { base_url, .. } => {
                format!("{}/v1/messages", base_url.trim_end_matches('/'))
            }
        }
    }

    /// Apply auth headers to a request builder.
    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.auth {
            AuthMode::ApiKey(key) => builder.header("x-api-key", key),
            AuthMode::Proxy { token, .. } => {
                builder.header("Authorization", format!("Bearer {}", token))
            }
        }
    }

    /// Generate a short session title from the first user message using a fast, cheap model.
    pub async fn generate_title(&self, user_message: &str) -> Result<String> {
        let body = ApiRequest {
            model: effective_model_or(TITLE_MODEL),
            max_tokens: 30,
            system: system_text("Generate a short title (max 6 words) for a computer support session based on the user's message. Output only the title, nothing else. No quotes."),
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
        let resp = self
            .apply_auth(builder)
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
            model: effective_model_or(TITLE_MODEL),
            max_tokens: 200,
            system: system_text("Summarize this IT support session in 2-3 short bullet points. Focus on: what was the problem, what was done, and the outcome. Be concise. Use plain language."),
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
        let resp = self
            .apply_auth(builder)
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

    /// Compress earlier session history into a durable diagnostic handoff.
    pub async fn generate_context_summary(
        &self,
        existing_summary: Option<&str>,
        messages_text: &str,
    ) -> Result<String> {
        let mut prompt = String::from(
            "Compress this older IT support session history into a concise handoff for the main model.\n\
             Preserve only durable facts the assistant still needs:\n\
             - current diagnosis or strongest working theory\n\
             - notable tool findings and error messages\n\
             - actions already taken and their results\n\
             - pending user follow-up or approval state if mentioned\n\
             - dead ends worth avoiding repeating\n\
             Omit chit-chat, duplicated logs, and routine command output.\n\
             Use Markdown with these sections when relevant:\n\
             ## Current state\n\
             ## Actions taken\n\
             ## Pending / watch-outs\n\
             Keep it compact and factual.\n",
        );

        if let Some(existing) = existing_summary.filter(|s| !s.trim().is_empty()) {
            prompt.push_str("\nExisting compressed summary:\n");
            prompt.push_str(existing);
            prompt.push_str("\n\nMerge the new history into that summary and return a single updated summary.\n");
        }

        let body = ApiRequest {
            model: context_summary_model(),
            max_tokens: 500,
            system: system_text(&prompt),
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
        let resp = self
            .apply_auth(builder)
            .send()
            .await
            .context("Context summary request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let error_body = resp.text().await.unwrap_or_default();
            anyhow::bail!("{}", friendly_api_error(status, &error_body));
        }

        let response: Response = resp
            .json()
            .await
            .context("Failed to parse context summary response")?;

        let summary = response
            .content
            .iter()
            .find_map(|b| match b {
                ResponseBlock::Text { text } => Some(text.trim().to_string()),
                _ => None,
            })
            .unwrap_or_default();

        Ok(summary)
    }

    /// Analyze diagnostic tool output using Haiku to determine if it's noteworthy.
    pub async fn analyze_diagnostics(
        &self,
        category: &str,
        tool_output: &str,
    ) -> Result<DiagnosticAnalysis> {
        let system = format!(
            "You are a conservative IT health monitor analyzing {} diagnostics.\n\
             Only flag genuinely concerning issues:\n\
             - Disk usage >90% or a single reclaimable folder >5GB\n\
             - Recent crash reports (<24h) for user-facing apps\n\
             - A process consuming >90% CPU persistently\n\
             - Critically low RAM (<500MB free) while idle\n\n\
             Respond in exactly this JSON format (no markdown, no extra text):\n\
             {{\"noteworthy\": true/false, \"headline\": \"~60 char summary\", \"detail\": \"1-2 sentence explanation\"}}\n\n\
             If nothing is concerning, set noteworthy to false with empty headline and detail.",
            category
        );

        let body = ApiRequest {
            model: effective_model_or(TITLE_MODEL),
            max_tokens: 200,
            system: system_text(&system),
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text(tool_output.to_string()),
            }],
            tools: vec![],
        };

        let builder = self
            .client
            .post(self.api_url())
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body);
        let resp = self
            .apply_auth(builder)
            .send()
            .await
            .context("Diagnostic analysis request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let error_body = resp.text().await.unwrap_or_default();
            anyhow::bail!("{}", friendly_api_error(status, &error_body));
        }

        let response: Response = resp
            .json()
            .await
            .context("Failed to parse diagnostic analysis response")?;

        let text = response
            .content
            .iter()
            .find_map(|b| match b {
                ResponseBlock::Text { text } => Some(text.trim().to_string()),
                _ => None,
            })
            .unwrap_or_default();

        // Strip markdown code fences if Haiku wrapped the response.
        let cleaned = strip_markdown_fences(&text);

        // Parse the JSON response from Haiku.
        let analysis: DiagnosticAnalysis =
            serde_json::from_str(&cleaned).unwrap_or(DiagnosticAnalysis {
                noteworthy: false,
                headline: String::new(),
                detail: String::new(),
            });

        Ok(analysis)
    }

    pub async fn send_message(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDef>,
        system: Vec<crate::agent::prompts::SystemBlock>,
    ) -> Result<Response> {
        let body = ApiRequest {
            model: effective_model(),
            max_tokens: MAX_TOKENS,
            system,
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
            let resp = match self.apply_auth(builder).send().await {
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

            if is_context_limit_error(status, &error_body) {
                anyhow::bail!("Context limit exceeded: {}", error_body);
            }

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
        assert!(
            msg.contains("API key"),
            "401 should mention API key: {}",
            msg
        );
        assert!(
            msg.contains("Settings"),
            "401 should mention Settings: {}",
            msg
        );
    }

    #[test]
    fn test_friendly_api_error_429() {
        let msg = friendly_api_error(reqwest::StatusCode::TOO_MANY_REQUESTS, "");
        assert!(
            msg.contains("Too many requests") || msg.contains("rate-limited"),
            "429 should mention rate limit: {}",
            msg
        );
    }

    #[test]
    fn test_friendly_api_error_500() {
        let msg = friendly_api_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "");
        assert!(
            msg.contains("temporary issues") || msg.contains("try again"),
            "500 should be friendly: {}",
            msg
        );
    }

    #[test]
    fn test_friendly_api_error_502() {
        let msg = friendly_api_error(reqwest::StatusCode::BAD_GATEWAY, "");
        assert!(
            msg.contains("temporary issues"),
            "502 should be friendly: {}",
            msg
        );
    }

    #[test]
    fn test_friendly_api_error_503() {
        let msg = friendly_api_error(reqwest::StatusCode::SERVICE_UNAVAILABLE, "");
        assert!(
            msg.contains("temporary issues"),
            "503 should be friendly: {}",
            msg
        );
    }

    #[test]
    fn test_friendly_api_error_529_overloaded() {
        let msg = friendly_api_error(reqwest::StatusCode::from_u16(529).unwrap(), "");
        assert!(
            msg.contains("overloaded"),
            "529 should mention overloaded: {}",
            msg
        );
    }

    #[test]
    fn test_friendly_api_error_unknown_includes_status() {
        let msg = friendly_api_error(reqwest::StatusCode::IM_A_TEAPOT, "brew coffee");
        assert!(
            msg.contains("418"),
            "Unknown error should include status code: {}",
            msg
        );
        assert!(
            msg.contains("brew coffee"),
            "Unknown error should include body: {}",
            msg
        );
    }

    #[test]
    fn test_friendly_api_error_403() {
        let msg = friendly_api_error(reqwest::StatusCode::FORBIDDEN, "");
        assert!(
            msg.contains("permission"),
            "403 should mention permission: {}",
            msg
        );
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

    // ── DiagnosticAnalysis parsing tests ──────────────────────────────

    #[test]
    fn test_diagnostic_analysis_parses_noteworthy() {
        let json = r#"{"noteworthy": true, "headline": "Disk 95% full", "detail": "Your main drive has only 12GB free."}"#;
        let analysis: DiagnosticAnalysis = serde_json::from_str(json).unwrap();
        assert!(analysis.noteworthy);
        assert_eq!(analysis.headline, "Disk 95% full");
        assert_eq!(analysis.detail, "Your main drive has only 12GB free.");
    }

    #[test]
    fn test_diagnostic_analysis_parses_not_noteworthy() {
        let json = r#"{"noteworthy": false, "headline": "", "detail": ""}"#;
        let analysis: DiagnosticAnalysis = serde_json::from_str(json).unwrap();
        assert!(!analysis.noteworthy);
        assert!(analysis.headline.is_empty());
    }

    #[test]
    fn test_diagnostic_analysis_malformed_json_fallback() {
        // The analyze_diagnostics method uses unwrap_or on parse failure.
        // Verify the fallback works.
        let bad_json = "This is not JSON at all";
        let analysis: DiagnosticAnalysis =
            serde_json::from_str(bad_json).unwrap_or(DiagnosticAnalysis {
                noteworthy: false,
                headline: String::new(),
                detail: String::new(),
            });
        assert!(!analysis.noteworthy);
    }

    #[test]
    fn test_diagnostic_analysis_missing_fields_fails() {
        // Partial JSON should fail to parse (all fields are required).
        let partial = r#"{"noteworthy": true}"#;
        let result: Result<DiagnosticAnalysis, _> = serde_json::from_str(partial);
        assert!(result.is_err());
    }

    #[test]
    fn test_diagnostic_analysis_extra_fields_ignored() {
        // Haiku might include extra fields — serde should ignore them.
        let json = r#"{"noteworthy": false, "headline": "", "detail": "", "confidence": 0.9}"#;
        let analysis: DiagnosticAnalysis = serde_json::from_str(json).unwrap();
        assert!(!analysis.noteworthy);
    }

    #[test]
    fn test_diagnostic_analysis_with_markdown_fenced_json() {
        // Haiku sometimes wraps its response in ```json ... ```.
        // Verify strip_markdown_fences handles this.
        let fenced =
            "```json\n{\"noteworthy\": true, \"headline\": \"test\", \"detail\": \"x\"}\n```";
        let cleaned = strip_markdown_fences(fenced);
        let analysis: DiagnosticAnalysis = serde_json::from_str(&cleaned).unwrap();
        assert!(analysis.noteworthy);
        assert_eq!(analysis.headline, "test");
    }

    #[test]
    fn test_strip_markdown_fences_plain_json() {
        let plain = r#"{"noteworthy": false, "headline": "", "detail": ""}"#;
        let cleaned = strip_markdown_fences(plain);
        assert_eq!(cleaned, plain);
    }

    #[test]
    fn test_strip_markdown_fences_bare_backticks() {
        let fenced = "```\n{\"noteworthy\": true, \"headline\": \"a\", \"detail\": \"b\"}\n```";
        let cleaned = strip_markdown_fences(fenced);
        let analysis: DiagnosticAnalysis = serde_json::from_str(&cleaned).unwrap();
        assert!(analysis.noteworthy);
    }

    #[test]
    fn test_strip_markdown_fences_with_whitespace() {
        let fenced =
            "  ```json\n{\"noteworthy\": false, \"headline\": \"\", \"detail\": \"\"}\n```  ";
        let cleaned = strip_markdown_fences(fenced);
        let analysis: DiagnosticAnalysis = serde_json::from_str(&cleaned).unwrap();
        assert!(!analysis.noteworthy);
    }

    #[test]
    fn test_no_raw_error_messages() {
        // All mapped status codes should NOT contain "Anthropic API error" (the old raw format).
        for status in [401u16, 403, 429, 500, 502, 503, 529] {
            let msg =
                friendly_api_error(reqwest::StatusCode::from_u16(status).unwrap(), "raw body");
            assert!(
                !msg.contains("Anthropic API error"),
                "Status {} should not show raw error: {}",
                status,
                msg
            );
        }
    }

    #[test]
    fn test_is_context_limit_error_detects_context_window_failures() {
        assert!(is_context_limit_error(
            reqwest::StatusCode::BAD_REQUEST,
            "prompt is too long for the model context window"
        ));
        assert!(is_context_limit_error(
            reqwest::StatusCode::PAYLOAD_TOO_LARGE,
            "maximum context length exceeded"
        ));
        assert!(!is_context_limit_error(
            reqwest::StatusCode::BAD_REQUEST,
            "invalid request body"
        ));
    }

    #[test]
    fn test_context_summary_model_uses_haiku() {
        assert_eq!(context_summary_model(), TITLE_MODEL);
    }
}
