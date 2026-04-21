use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

pub fn base_url() -> String {
    std::env::var("NOAH_CONSUMER_URL")
        .unwrap_or_else(|_| "http://localhost:8787".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entitlement {
    pub plan: Option<String>,
    pub status: String,
    pub trial_started_at: Option<i64>,
    pub trial_ends_at: Option<i64>,
    pub period_start: Option<i64>,
    pub period_end: Option<i64>,
    pub usage_used: i64,
    pub usage_limit: i64,
    pub fix_count_total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixCompletedResponse {
    pub fix_count_total: i64,
    pub usage_used: i64,
    pub entitlement: Entitlement,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("reqwest client build")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicLinkResponse {
    pub ok: bool,
    /// Set when the server chooses to issue a session immediately
    /// (current behavior — lets the user proceed without clicking
    /// the emailed link first). Absent means the old "check your
    /// inbox to finish" flow is in effect.
    #[serde(default)]
    pub session_token: Option<String>,
}

pub async fn request_magic_link(email: &str) -> Result<MagicLinkResponse> {
    let resp = client()
        .post(format!("{}/auth/request", base_url()))
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("auth/request failed: {status} — {text}"));
    }
    Ok(resp.json().await?)
}

pub async fn fetch_entitlement(token: &str) -> Result<Entitlement> {
    let resp = client()
        .get(format!("{}/entitlement", base_url()))
        .bearer_auth(token)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("entitlement fetch failed: {}", resp.status()));
    }
    let ent: Entitlement = resp.json().await?;
    Ok(ent)
}

pub async fn notify_issue_started(token: &str) -> Result<Entitlement> {
    let resp = client()
        .post(format!("{}/events/issue-started", base_url()))
        .bearer_auth(token)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("issue-started failed: {}", resp.status()));
    }
    Ok(resp.json().await?)
}

pub async fn notify_fix_completed(token: &str) -> Result<FixCompletedResponse> {
    let resp = client()
        .post(format!("{}/events/fix-completed", base_url()))
        .bearer_auth(token)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("fix-completed failed: {}", resp.status()));
    }
    Ok(resp.json().await?)
}

pub async fn billing_checkout_url(token: &str, plan: &str) -> Result<String> {
    let resp = client()
        .post(format!("{}/billing/checkout", base_url()))
        .bearer_auth(token)
        .json(&serde_json::json!({ "plan": plan }))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("checkout failed: {}", resp.status()));
    }
    let body: serde_json::Value = resp.json().await?;
    body.get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("checkout response missing url"))
}

pub async fn billing_portal_url(token: &str) -> Result<String> {
    let resp = client()
        .post(format!("{}/billing/portal", base_url()))
        .bearer_auth(token)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("portal failed: {}", resp.status()));
    }
    let body: serde_json::Value = resp.json().await?;
    body.get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("portal response missing url"))
}
