use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

const CONFIG_FILE: &str = "dashboard.json";

fn default_fleet_name() -> String {
    "My Fleet".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub dashboard_url: String,
    pub device_token: String,
    pub device_id: String,
    #[serde(default = "default_fleet_name")]
    pub fleet_name: String,
    pub linked_at: String,
    #[serde(default)]
    pub enabled_categories: Option<Vec<String>>,
}

impl DashboardConfig {
    /// Load config from app data dir, if linked.
    pub fn load(app_dir: &Path) -> Option<Self> {
        let path = app_dir.join(CONFIG_FILE);
        let contents = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&contents).ok()
    }

    /// Save config to app data dir.
    pub fn save(&self, app_dir: &Path) -> Result<()> {
        let path = app_dir.join(CONFIG_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json).context("Failed to save dashboard config")?;
        Ok(())
    }

    /// Remove config (unlink).
    pub fn remove(app_dir: &Path) {
        let path = app_dir.join(CONFIG_FILE);
        let _ = std::fs::remove_file(path);
    }
}

/// Parse an enrollment URL into (base_url, token).
/// Accepts formats:
///   https://dashboard.example.com/enroll/abc123
///   https://dashboard.example.com/enroll/abc123/
/// Returns (https://dashboard.example.com, abc123)
pub fn parse_enrollment_url(input: &str) -> Result<(String, String)> {
    let trimmed = input.trim().trim_end_matches('/');

    // Find /enroll/ in the URL
    if let Some(pos) = trimmed.find("/enroll/") {
        let base_url = trimmed[..pos].to_string();
        let token = trimmed[pos + 8..].to_string(); // skip "/enroll/"
        if base_url.is_empty() || token.is_empty() {
            anyhow::bail!("Invalid enrollment URL — expected format: https://your-dashboard/enroll/TOKEN");
        }
        return Ok((base_url, token));
    }

    anyhow::bail!("Invalid enrollment URL — expected format: https://your-dashboard/enroll/TOKEN")
}

/// Enroll this device with the fleet dashboard using an enrollment token.
pub async fn enroll_device(
    base_url: &str,
    enrollment_token: &str,
) -> Result<(String, String, String, Option<Vec<String>>)> {
    let url = format!("{}/devices/enroll", base_url.trim_end_matches('/'));

    let os_name = if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    };

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "Unknown Device".to_string());

    let body = serde_json::json!({
        "token": enrollment_token,
        "device_name": hostname,
        "device_os": os_name,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("Failed to connect to fleet dashboard")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Enrollment failed ({}): {}", status, text);
    }

    #[derive(Deserialize)]
    struct EnrollResponse {
        device_id: String,
        device_token: String,
        fleet_name: Option<String>,
        enabled_categories: Option<Vec<String>>,
    }

    let data: EnrollResponse = resp.json().await.context("Invalid response from fleet dashboard")?;
    Ok((data.device_id, data.device_token, data.fleet_name.unwrap_or_else(|| "My Fleet".to_string()), data.enabled_categories))
}

/// Push a health checkin to the dashboard.
/// Pass `app_dir` so we can auto-unlink if the device token has been revoked (admin removed device).
pub async fn push_checkin(config: &DashboardConfig, score: i32, grade: &str, categories_json: &str, app_dir: Option<&Path>) -> Result<Option<Vec<String>>> {
    let url = format!("{}/dashboard/checkin", config.dashboard_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "health_score": score,
        "health_grade": grade,
        "categories": categories_json,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.device_token))
        .json(&body)
        .send()
        .await
        .context("Failed to push checkin")?;

    if resp.status().as_u16() == 401 {
        // Device token is invalid — admin removed this device from the fleet.
        if let Some(dir) = app_dir {
            eprintln!("[fleet] device token revoked, auto-unlinking");
            DashboardConfig::remove(dir);
        }
        anyhow::bail!("Device removed from fleet");
    }

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Checkin failed: {}", text);
    }

    // Parse policy + assigned playbooks from checkin response.
    #[derive(Deserialize)]
    struct CheckinResponse {
        enabled_categories: Option<Vec<String>>,
        fleet_name: Option<String>,
        #[serde(default)]
        assigned_playbooks: Option<Vec<AssignedPlaybook>>,
    }

    let data: CheckinResponse = resp.json().await.unwrap_or(CheckinResponse { enabled_categories: None, fleet_name: None, assigned_playbooks: None });

    // Update fleet name from server (handles admin renames)
    if let (Some(ref name), Some(dir)) = (&data.fleet_name, app_dir) {
        if let Some(mut cfg) = DashboardConfig::load(dir) {
            if cfg.fleet_name != *name {
                cfg.fleet_name = name.clone();
                let _ = cfg.save(dir);
            }
        }
    }

    // Persist assigned playbooks to disk so they're available for auto-heal and manual runs
    if let Some(ref playbooks) = data.assigned_playbooks {
        if !playbooks.is_empty() {
            if let Some(dir) = app_dir {
                let pb_path = dir.join("fleet_playbooks.json");
                let _ = std::fs::write(&pb_path, serde_json::to_string_pretty(playbooks).unwrap_or_default());
            }
        }
    }

    Ok(data.enabled_categories)
}

/// A playbook assigned to this device via fleet (from group or direct assignment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignedPlaybook {
    pub slug: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetAction {
    pub id: String,
    pub check_id: String,
    pub check_label: String,
    pub action_hint: String,
    pub created_at: String,
    #[serde(default = "default_action_type")]
    pub action_type: String,
    pub playbook_slug: Option<String>,
    pub playbook_content: Option<String>,
    pub issue_id: Option<String>,
}

fn default_action_type() -> String {
    "hint".to_string()
}

/// Poll for pending remediation actions from the fleet.
/// Pass `app_dir` so we can auto-unlink if the device token has been revoked.
pub async fn poll_actions(config: &DashboardConfig, app_dir: Option<&Path>) -> Result<Vec<FleetAction>> {
    let url = format!("{}/dashboard/actions/pending", config.dashboard_url.trim_end_matches('/'));

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.device_token))
        .send()
        .await
        .context("Failed to poll actions")?;

    if resp.status().as_u16() == 401 {
        if let Some(dir) = app_dir {
            eprintln!("[fleet] device token revoked during action poll, auto-unlinking");
            DashboardConfig::remove(dir);
        }
        return Ok(Vec::new());
    }

    if !resp.status().is_success() {
        return Ok(Vec::new());
    }

    #[derive(Deserialize)]
    struct PollResponse {
        actions: Vec<FleetAction>,
    }

    let data: PollResponse = resp.json().await.unwrap_or(PollResponse { actions: Vec::new() });
    Ok(data.actions)
}

/// Report action status back to the fleet.
pub async fn report_action_status(config: &DashboardConfig, action_id: &str, status: &str) -> Result<()> {
    let url = format!("{}/dashboard/actions/{}/status", config.dashboard_url.trim_end_matches('/'), action_id);

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.device_token))
        .json(&serde_json::json!({ "status": status }))
        .send()
        .await
        .context("Failed to report action status")?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Report action failed: {}", text);
    }

    Ok(())
}

/// Push verification results after a remediation.
pub async fn push_verification(
    config: &DashboardConfig,
    action_id: &str,
    score_after: i32,
) -> Result<()> {
    let url = format!(
        "{}/dashboard/actions/{}/verify",
        config.dashboard_url.trim_end_matches('/'),
        action_id,
    );

    let body = serde_json::json!({
        "score_after": score_after,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.device_token))
        .json(&body)
        .send()
        .await
        .context("Failed to push verification")?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Verification push failed: {}", text);
    }

    Ok(())
}

/// Push an auto-heal event to the fleet dashboard.
pub async fn push_auto_heal_event(
    config: &DashboardConfig,
    check_id: &str,
    playbook_slug: &str,
    score_before: i32,
    score_after: i32,
) -> Result<()> {
    let url = format!(
        "{}/dashboard/auto-heal",
        config.dashboard_url.trim_end_matches('/'),
    );

    let body = serde_json::json!({
        "check_id": check_id,
        "playbook_slug": playbook_slug,
        "score_before": score_before,
        "score_after": score_after,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.device_token))
        .json(&body)
        .send()
        .await
        .context("Failed to push auto-heal event")?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Auto-heal event push failed: {}", text);
    }

    Ok(())
}

/// Push a session report to the fleet dashboard.
/// Called when a session ends or when resolved status changes.
pub async fn push_session_report(
    config: &DashboardConfig,
    session_id: &str,
    title: Option<&str>,
    summary: Option<&str>,
    message_count: i32,
    resolved: Option<bool>,
    started_at: &str,
    ended_at: Option<&str>,
) -> Result<()> {
    let url = format!(
        "{}/dashboard/session-report",
        config.dashboard_url.trim_end_matches('/'),
    );

    let mut body = serde_json::json!({
        "session_id": session_id,
        "message_count": message_count,
        "started_at": started_at,
    });

    if let Some(t) = title {
        body["title"] = serde_json::Value::String(t.to_string());
    }
    if let Some(s) = summary {
        body["summary"] = serde_json::Value::String(s.to_string());
    }
    if let Some(r) = resolved {
        body["resolved"] = serde_json::Value::Bool(r);
    }
    if let Some(e) = ended_at {
        body["ended_at"] = serde_json::Value::String(e.to_string());
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.device_token))
        .json(&body)
        .send()
        .await
        .context("Failed to push session report")?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Session report push failed: {}", text);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_enrollment_url_valid() {
        let (base, token) = parse_enrollment_url("https://dash.onnoah.app/enroll/abc123def456").unwrap();
        assert_eq!(base, "https://dash.onnoah.app");
        assert_eq!(token, "abc123def456");
    }

    #[test]
    fn parse_enrollment_url_trailing_slash() {
        let (base, token) = parse_enrollment_url("https://example.com/enroll/tok123/").unwrap();
        assert_eq!(base, "https://example.com");
        assert_eq!(token, "tok123");
    }

    #[test]
    fn parse_enrollment_url_invalid() {
        assert!(parse_enrollment_url("https://example.com").is_err());
        assert!(parse_enrollment_url("not-a-url").is_err());
        assert!(parse_enrollment_url("https://example.com/enroll/").is_err());
    }
}
