use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

const CONFIG_FILE: &str = "dashboard.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub dashboard_url: String,
    pub device_token: String,
    pub device_id: String,
    pub linked_at: String,
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

/// Exchange a link code with the dashboard API to get a device token.
pub async fn link_device(
    dashboard_url: &str,
    code: &str,
) -> Result<(String, String)> {
    let url = format!("{}/devices/link", dashboard_url.trim_end_matches('/'));

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
        "code": code,
        "device_name": hostname,
        "device_os": os_name,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("Failed to connect to dashboard")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Link failed ({}): {}", status, text);
    }

    #[derive(Deserialize)]
    struct LinkResponse {
        device_id: String,
        device_token: String,
    }

    let data: LinkResponse = resp.json().await.context("Invalid response from dashboard")?;
    Ok((data.device_id, data.device_token))
}

/// Push a health checkin to the dashboard.
pub async fn push_checkin(config: &DashboardConfig, score: i32, grade: &str, categories_json: &str) -> Result<()> {
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

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Checkin failed: {}", text);
    }

    Ok(())
}
