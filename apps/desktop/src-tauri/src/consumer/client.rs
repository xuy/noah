use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

pub fn base_url() -> String {
    // Release builds talk to production by default; debug builds talk to
    // a local dev server. Either way, NOAH_CONSUMER_URL env overrides
    // (dev-reset-auth.sh sets it; staging tests set it).
    //
    // 8788 is the dev port — 8787 is a common local collision (the
    // agent-native-channel project uses it).
    if let Ok(url) = std::env::var("NOAH_CONSUMER_URL") {
        return url;
    }
    if cfg!(debug_assertions) {
        "http://localhost:8788".to_string()
    } else {
        "https://noah-consumer.fly.dev".to_string()
    }
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

/// Either a signed-in session token or an anonymous device id.
/// Mirrors the server's `requireSessionOrDevice` middleware.
#[derive(Debug, Clone)]
pub enum Auth<'a> {
    Session(&'a str),
    Device(&'a str),
}

fn apply_auth(
    builder: reqwest::RequestBuilder,
    auth: &Auth<'_>,
) -> reqwest::RequestBuilder {
    match auth {
        Auth::Session(t) => builder.bearer_auth(t),
        Auth::Device(d) => builder.header("X-Device-Id", *d),
    }
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

pub async fn fetch_entitlement(auth: &Auth<'_>) -> Result<Entitlement> {
    let req = client().get(format!("{}/entitlement", base_url()));
    let resp = apply_auth(req, auth).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("entitlement fetch failed: {}", resp.status()));
    }
    Ok(resp.json().await?)
}

pub async fn notify_issue_started(auth: &Auth<'_>) -> Result<Entitlement> {
    let req = client().post(format!("{}/events/issue-started", base_url()));
    let resp = apply_auth(req, auth).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("issue-started failed: {}", resp.status()));
    }
    Ok(resp.json().await?)
}

pub async fn notify_fix_completed(auth: &Auth<'_>) -> Result<FixCompletedResponse> {
    let req = client().post(format!("{}/events/fix-completed", base_url()));
    let resp = apply_auth(req, auth).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("fix-completed failed: {}", resp.status()));
    }
    Ok(resp.json().await?)
}

/// `/billing/checkout` accepts either session or device auth — a paying
/// user can be a signed-in account or an anonymous device completing
/// their trial's payment moment.
pub async fn billing_checkout_url(auth: &Auth<'_>, plan: &str) -> Result<String> {
    let req = client()
        .post(format!("{}/billing/checkout", base_url()))
        .json(&serde_json::json!({ "plan": plan }));
    let resp = apply_auth(req, auth).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("checkout failed: {}", resp.status()));
    }
    let body: serde_json::Value = resp.json().await?;
    body.get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("checkout response missing url"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmCheckoutResponse {
    pub ok: bool,
    #[serde(default)]
    pub session_token: Option<String>,
    #[serde(default)]
    pub entitlement: Option<Entitlement>,
}

/// After the user returns from Stripe Checkout via the noah://subscribed
/// deep link, hand the Checkout session id to the server. It verifies
/// with Stripe, upserts a user by the customer email, links the device's
/// entitlement row to that user, and mints a session token so the app
/// becomes signed-in AND paid in one step.
pub async fn confirm_checkout(checkout_session_id: &str) -> Result<ConfirmCheckoutResponse> {
    let resp = client()
        .post(format!("{}/billing/confirm", base_url()))
        .json(&serde_json::json!({ "checkout_session_id": checkout_session_id }))
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("confirm failed: {status} — {body}"));
    }
    Ok(resp.json().await?)
}

/// `/billing/portal` stays session-only — only signed-in users have a
/// Stripe customer to manage.
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
