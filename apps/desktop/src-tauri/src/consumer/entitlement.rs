use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::client::Entitlement;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntitlement {
    pub entitlement: Entitlement,
    pub fetched_at: i64,
}

const OFFLINE_GRACE_MS: i64 = 72 * 60 * 60 * 1000;

fn cache_path(app_dir: &Path) -> PathBuf {
    app_dir.join("entitlement_cache.json")
}

pub fn load_cached(app_dir: &Path) -> Option<CachedEntitlement> {
    let contents = std::fs::read_to_string(cache_path(app_dir)).ok()?;
    serde_json::from_str(&contents).ok()
}

pub fn save_cached(app_dir: &Path, ent: &Entitlement) -> Result<(), String> {
    let cached = CachedEntitlement {
        entitlement: ent.clone(),
        fetched_at: chrono::Utc::now().timestamp_millis(),
    };
    let json = serde_json::to_string_pretty(&cached).map_err(|e| e.to_string())?;
    std::fs::write(cache_path(app_dir), json).map_err(|e| e.to_string())
}

pub fn clear_cache(app_dir: &Path) {
    let _ = std::fs::remove_file(cache_path(app_dir));
}

pub fn is_within_offline_grace(cached: &CachedEntitlement) -> bool {
    let now = chrono::Utc::now().timestamp_millis();
    (now - cached.fetched_at) <= OFFLINE_GRACE_MS
}

/// Compute whether the user is currently paywalled.
pub fn is_paywalled(ent: &Entitlement) -> bool {
    match ent.status.as_str() {
        "none" | "trialing" => false,
        "active" => ent.usage_used >= ent.usage_limit,
        _ => true,
    }
}
