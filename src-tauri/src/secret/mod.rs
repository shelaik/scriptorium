//! API keys live in the OS credential vault (Windows Credential Manager via
//! DPAPI), not in the SQLite settings table — so they're encrypted at rest, never
//! land in backups/sync, and are never handed to the webview.

use keyring::Entry;

const SERVICE: &str = "Scriptorium";

/// The secret keys this app manages.
pub const KEYS: &[&str] = &["openalex_key", "ads_token", "s2_key", "core_key", "github_token"];

pub fn is_known(name: &str) -> bool {
    KEYS.contains(&name)
}

fn entry(name: &str) -> Option<Entry> {
    Entry::new(SERVICE, name).ok()
}

/// Read a stored secret (None if unset or unavailable).
pub fn get(name: &str) -> Option<String> {
    let v = entry(name)?.get_password().ok()?;
    (!v.is_empty()).then_some(v)
}

pub fn has(name: &str) -> bool {
    get(name).is_some()
}

/// Store (or, if `value` is empty, delete) a secret.
pub fn set(name: &str, value: &str) -> Result<(), String> {
    let e = entry(name).ok_or_else(|| "vault non disponibile".to_string())?;
    if value.trim().is_empty() {
        return delete_entry(&e);
    }
    e.set_password(value.trim()).map_err(|x| x.to_string())
}

pub fn delete(name: &str) -> Result<(), String> {
    match entry(name) {
        Some(e) => delete_entry(&e),
        None => Ok(()),
    }
}

fn delete_entry(e: &Entry) -> Result<(), String> {
    match e.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(x) => Err(x.to_string()),
    }
}
