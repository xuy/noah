use keyring::Entry;
use uuid::Uuid;

const SERVICE: &str = "app.onnoah.noah";
const ACCOUNT: &str = "device_id";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("keychain init: {e}"))
}

pub fn get_device_id() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(t) if t.is_empty() => Ok(None),
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("keychain get: {e}")),
    }
}

/// Returns the existing device id from the keychain, or generates a
/// fresh UUIDv4 and persists it. Stable for the life of this machine /
/// keychain entry.
pub fn ensure_device_id() -> Result<String, String> {
    if let Some(existing) = get_device_id()? {
        return Ok(existing);
    }
    let id = Uuid::new_v4().to_string();
    entry()?
        .set_password(&id)
        .map_err(|e| format!("keychain set: {e}"))?;
    Ok(id)
}
