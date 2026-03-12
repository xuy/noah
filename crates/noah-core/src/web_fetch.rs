use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use noah_tools::{SafetyTier, Tool, ToolResult};

const MAX_BODY_BYTES: usize = 512 * 1024; // 512KB
const MAX_OUTPUT_CHARS: usize = 100_000;
const TIMEOUT_SECS: u64 = 30;
const USER_AGENT: &str = "Noah/0.13 (Desktop IT Agent)";

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch the content of a web page given its URL. Returns the page text (HTML is converted to readable text). Use this when the user provides a URL you need to read."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                }
            },
            "required": ["url"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if url.is_empty() {
            bail!("Missing required parameter: url");
        }

        // Validate URL scheme
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Ok(ToolResult::read_only(
                "Invalid URL: only http:// and https:// URLs are supported.".to_string(),
                json!({"error": "invalid_scheme"}),
            ));
        }

        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;

        let response = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                let msg = if e.is_timeout() {
                    "Request timed out after 30 seconds.".to_string()
                } else if e.is_connect() {
                    format!("Could not connect to the server: {}", e)
                } else {
                    format!("Failed to fetch URL: {}", e)
                };
                return Ok(ToolResult::read_only(msg, json!({"error": "fetch_failed"})));
            }
        };

        let status = response.status();
        if !status.is_success() {
            return Ok(ToolResult::read_only(
                format!("Server returned HTTP {} {}.", status.as_u16(), status.canonical_reason().unwrap_or("")),
                json!({"error": "http_error", "status": status.as_u16()}),
            ));
        }

        // Check content type
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let is_html = content_type.contains("text/html");
        let is_text = content_type.starts_with("text/")
            || content_type.contains("json")
            || content_type.contains("xml")
            || content_type.contains("javascript")
            || content_type.is_empty(); // assume text if no content-type

        if !is_text && !is_html {
            return Ok(ToolResult::read_only(
                format!("Cannot read binary content (type: {}). Only text and HTML pages are supported.", content_type),
                json!({"error": "binary_content"}),
            ));
        }

        // Read body with size limit
        let bytes = match read_limited_body(response, MAX_BODY_BYTES).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolResult::read_only(
                    format!("Failed to read response body: {}", e),
                    json!({"error": "read_failed"}),
                ));
            }
        };

        let body = String::from_utf8_lossy(&bytes).to_string();

        // Convert HTML to text, or use raw for other text types
        let mut text = if is_html {
            html2text::from_read(body.as_bytes(), 100)
        } else {
            body
        };

        // Truncate if needed
        let mut truncated = false;
        if text.len() > MAX_OUTPUT_CHARS {
            // Find a safe char boundary
            let mut end = MAX_OUTPUT_CHARS;
            while !text.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            text.truncate(end);
            truncated = true;
        }

        if truncated {
            text.push_str("\n\n(content truncated — original page was longer than 100K characters)");
        }

        Ok(ToolResult::read_only(
            text,
            json!({"url": url, "truncated": truncated}),
        ))
    }
}

/// Read response body up to `limit` bytes.
async fn read_limited_body(response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    // Check content-length header first for early rejection
    if let Some(len) = response.content_length() {
        if len as usize > limit {
            bail!("Response too large ({} bytes, limit is {} bytes)", len, limit);
        }
    }

    let bytes = response.bytes().await?;
    if bytes.len() > limit {
        bail!("Response too large ({} bytes, limit is {} bytes)", bytes.len(), limit);
    }
    Ok(bytes.to_vec())
}
