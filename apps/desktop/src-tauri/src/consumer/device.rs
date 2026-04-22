//! Stable device identifier, file-based.
//!
//! Stored as `device_id.txt` in the app data directory. Sent as
//! `X-Device-Id` on anonymous backend calls. Random UUIDv4 — not
//! sensitive, not correlatable to the user; purely a handle so the
//! server can track one anonymous trial per install.

use std::path::{Path, PathBuf};
use uuid::Uuid;

const FILE: &str = "device_id.txt";

fn path(app_dir: &Path) -> PathBuf {
    app_dir.join(FILE)
}

pub fn get_device_id(app_dir: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path(app_dir)) {
        Ok(s) => {
            let id = s.trim().to_string();
            if id.is_empty() { Ok(None) } else { Ok(Some(id)) }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read device_id: {e}")),
    }
}

pub fn ensure_device_id(app_dir: &Path) -> Result<String, String> {
    if let Some(existing) = get_device_id(app_dir)? {
        return Ok(existing);
    }
    let id = Uuid::new_v4().to_string();
    std::fs::write(path(app_dir), &id).map_err(|e| format!("write device_id: {e}"))?;
    Ok(id)
}

pub fn delete_device_id(app_dir: &Path) -> Result<(), String> {
    match std::fs::remove_file(path(app_dir)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("delete device_id: {e}")),
    }
}
