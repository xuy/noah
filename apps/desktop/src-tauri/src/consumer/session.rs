use keyring::Entry;

const SERVICE: &str = "app.onnoah.noah";
const ACCOUNT: &str = "session_token";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("keychain init: {e}"))
}

pub fn set_session_token(token: &str) -> Result<(), String> {
    entry()?
        .set_password(token)
        .map_err(|e| format!("keychain set: {e}"))
}

pub fn get_session_token() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(t) if t.is_empty() => Ok(None),
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("keychain get: {e}")),
    }
}

pub fn delete_session_token() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keychain delete: {e}")),
    }
}

pub fn has_session() -> bool {
    matches!(get_session_token(), Ok(Some(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Touches the real keychain — run manually with `cargo test -- --ignored`.
    fn roundtrip() {
        let token = "test-token-abc";
        set_session_token(token).unwrap();
        assert_eq!(get_session_token().unwrap().as_deref(), Some(token));
        assert!(has_session());
        delete_session_token().unwrap();
        assert_eq!(get_session_token().unwrap(), None);
        assert!(!has_session());
    }
}
