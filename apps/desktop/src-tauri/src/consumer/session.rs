//! Session-token storage, file-based.
//!
//! Stores the signed-in session token as a plain file in the app
//! data directory. Previously used the macOS Keychain, but that
//! caused unnecessary auth prompts and the session token isn't
//! sensitive enough to warrant the ceremony — it's functionally
//! equivalent to a password-manager cookie, and it already lived
//! next to `api_key.txt` (plain) in earlier versions.

use std::path::{Path, PathBuf};

const FILE: &str = "session.txt";

fn path(app_dir: &Path) -> PathBuf {
    app_dir.join(FILE)
}

pub fn set_session_token(app_dir: &Path, token: &str) -> Result<(), String> {
    std::fs::write(path(app_dir), token.trim())
        .map_err(|e| format!("write session: {e}"))
}

pub fn get_session_token(app_dir: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path(app_dir)) {
        Ok(s) => {
            let t = s.trim().to_string();
            if t.is_empty() { Ok(None) } else { Ok(Some(t)) }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read session: {e}")),
    }
}

pub fn delete_session_token(app_dir: &Path) -> Result<(), String> {
    match std::fs::remove_file(path(app_dir)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("delete session: {e}")),
    }
}

pub fn has_session(app_dir: &Path) -> bool {
    matches!(get_session_token(app_dir), Ok(Some(_)))
}
