use crate::agent::llm_client::AuthMode;

/// Load auth: proxy.json first, then api_key.txt, then env var.
pub fn load_auth(app_dir: &std::path::Path) -> AuthMode {
    // Check for proxy config first
    let proxy_path = app_dir.join("proxy.json");
    if let Ok(contents) = std::fs::read_to_string(&proxy_path) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&contents) {
            if let (Some(base_url), Some(token)) = (
                parsed.get("base_url").and_then(|v| v.as_str()),
                parsed.get("token").and_then(|v| v.as_str()),
            ) {
                if !token.is_empty() {
                    return AuthMode::Proxy {
                        base_url: base_url.to_string(),
                        token: token.to_string(),
                    };
                }
            }
        }
    }

    // Fall back to API key file
    let key_path = app_dir.join("api_key.txt");
    if let Ok(contents) = std::fs::read_to_string(&key_path) {
        let key = contents.trim().to_string();
        if !key.is_empty() {
            return AuthMode::ApiKey(key);
        }
    }

    // Fall back to environment variable
    AuthMode::ApiKey(std::env::var("ANTHROPIC_API_KEY").unwrap_or_default())
}

/// Save API key to config file (and remove proxy.json if present).
pub fn save_api_key(app_dir: &std::path::Path, key: &str) -> Result<(), String> {
    let key_path = app_dir.join("api_key.txt");
    std::fs::write(&key_path, key).map_err(|e| format!("Failed to save API key: {}", e))?;
    // Remove proxy config if switching to API key mode
    let proxy_path = app_dir.join("proxy.json");
    let _ = std::fs::remove_file(&proxy_path);
    Ok(())
}

/// Save proxy config (and remove api_key.txt if present).
pub fn save_proxy_config(app_dir: &std::path::Path, base_url: &str, token: &str) -> Result<(), String> {
    let proxy_path = app_dir.join("proxy.json");
    let json = serde_json::json!({ "base_url": base_url, "token": token });
    std::fs::write(&proxy_path, json.to_string())
        .map_err(|e| format!("Failed to save proxy config: {}", e))?;
    // Remove API key file if switching to proxy mode
    let key_path = app_dir.join("api_key.txt");
    let _ = std::fs::remove_file(&key_path);
    Ok(())
}

/// Clear all auth config.
pub fn clear_auth_files(app_dir: &std::path::Path) {
    let _ = std::fs::remove_file(app_dir.join("api_key.txt"));
    let _ = std::fs::remove_file(app_dir.join("proxy.json"));
}
