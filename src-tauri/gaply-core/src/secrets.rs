//! Secure secret storage via the OS keychain, through the `keyring` crate.
//!
//! Cross-platform: the same code runs on all three desktop targets, with
//! keyring abstracting the native backend (configured per platform in
//! Cargo.toml):
//!   * macOS   — login Keychain (Security framework)
//!   * Windows — Windows Credential Manager
//!   * Linux   — freedesktop Secret Service (gnome-keyring / KWallet)
//!
//! No platform-specific code is required for the happy path. The only
//! practical difference is that the Linux Secret Service needs a running
//! keyring daemon; when it is absent (some minimal distros, headless CI) the
//! store is unreachable — that is surfaced as a clear [`GaplyError::Keychain`]
//! with a hint, never a panic (see [`map_keyring_err`]).
//!
//! # Security rule
//!
//! API keys (Claude, OpenAI, any future external service) live ONLY in the OS
//! keychain here in the Rust core, or in a future proxy's server-side
//! environment. They must never appear in frontend/React code, in
//! localStorage, or in the build bundle. External API calls originate from the
//! Rust core, which is the only place that reads a raw key back
//! ([`get_secret`]) to attach it to an outbound request.
//!
//! The frontend may ask the core to STORE a key the user typed
//! ([`store_secret`]), to CHECK whether one exists ([`has_secret`]), or to
//! DELETE one ([`delete_secret`]) — but it can never retrieve the raw value.

use keyring::Entry;

use crate::error::GaplyError;

/// Keychain service name namespacing all Gaply secrets.
const SERVICE: &str = "ai.gaply.app";

fn entry(name: &str) -> Result<Entry, GaplyError> {
    if name.trim().is_empty() {
        return Err(GaplyError::Validation("secret name must not be empty".into()));
    }
    Entry::new(SERVICE, name).map_err(map_keyring_err)
}

/// Map a keyring error to a clear [`GaplyError`], never a panic. The
/// "store unreachable" cases (notably a Linux Secret Service with no running
/// daemon) get an actionable hint.
fn map_keyring_err(e: keyring::Error) -> GaplyError {
    match e {
        keyring::Error::NoStorageAccess(inner) => GaplyError::Keychain(format!(
            "OS credential store is not accessible: {inner}. On Linux a Secret Service \
             daemon (e.g. gnome-keyring or KWallet) must be running."
        )),
        keyring::Error::PlatformFailure(inner) => {
            GaplyError::Keychain(format!("OS credential store failure: {inner}"))
        }
        other => GaplyError::Keychain(other.to_string()),
    }
}

/// Store (or overwrite) a named secret in the OS keychain.
#[tracing::instrument(skip(value), fields(name = %name))]
pub fn store_secret(name: &str, value: &str) -> Result<(), GaplyError> {
    if value.is_empty() {
        return Err(GaplyError::Validation("secret value must not be empty".into()));
    }
    entry(name)?.set_password(value).map_err(map_keyring_err)?;
    // never log the value
    tracing::info!("secret stored");
    Ok(())
}

/// Retrieve a named secret. INTERNAL to the Rust core — do not expose the raw
/// value to the frontend. Returns a typed `NotFound` (never panics) when the
/// secret does not exist.
#[tracing::instrument]
pub fn get_secret(name: &str) -> Result<String, GaplyError> {
    match entry(name)?.get_password() {
        Ok(v) => Ok(v),
        Err(keyring::Error::NoEntry) => {
            Err(GaplyError::NotFound { entity: "secret", id: name.to_string() })
        }
        Err(e) => Err(map_keyring_err(e)),
    }
}

/// Whether a named secret exists — safe to expose to the frontend (reveals
/// presence, never the value).
#[tracing::instrument]
pub fn has_secret(name: &str) -> Result<bool, GaplyError> {
    match entry(name)?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(map_keyring_err(e)),
    }
}

/// Delete a named secret. Absence is reported as a typed `NotFound`.
#[tracing::instrument]
pub fn delete_secret(name: &str) -> Result<(), GaplyError> {
    match entry(name)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => {
            Err(GaplyError::NotFound { entity: "secret", id: name.to_string() })
        }
        Err(e) => Err(map_keyring_err(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unique per run so parallel tests and repeat runs never collide in the
    // real login keychain; every entry is deleted before the test returns.
    fn unique(name: &str) -> String {
        format!(
            "gaply-test-{}-{}-{:?}",
            name,
            std::process::id(),
            std::thread::current().id()
        )
    }

    #[test]
    fn store_and_retrieve_roundtrip() {
        let key = unique("claude-api-key");
        store_secret(&key, "sk-ant-not-a-real-key").unwrap();

        assert_eq!(get_secret(&key).unwrap(), "sk-ant-not-a-real-key");
        assert!(has_secret(&key).unwrap());

        // overwrite works
        store_secret(&key, "sk-ant-rotated").unwrap();
        assert_eq!(get_secret(&key).unwrap(), "sk-ant-rotated");

        // cleanup: the test-scoped entry is removed
        delete_secret(&key).unwrap();
        assert!(!has_secret(&key).unwrap());
    }

    #[test]
    fn missing_secret_returns_clear_error_not_panic() {
        let err = get_secret(&unique("does-not-exist")).unwrap_err();
        assert_eq!(err.code(), "not_found");
        assert!(err.to_string().contains("secret not found"));
    }

    #[test]
    fn delete_missing_secret_is_not_found() {
        let err = delete_secret(&unique("never-stored")).unwrap_err();
        assert!(matches!(err, GaplyError::NotFound { entity: "secret", .. }));
    }

    #[test]
    fn empty_name_and_value_are_validation_errors() {
        assert!(matches!(store_secret("", "v").unwrap_err(), GaplyError::Validation(_)));
        assert!(matches!(store_secret("k", "").unwrap_err(), GaplyError::Validation(_)));
        assert!(matches!(get_secret("").unwrap_err(), GaplyError::Validation(_)));
    }
}
