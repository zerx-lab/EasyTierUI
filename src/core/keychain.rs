//! Where secrets rest: the OS keychain (macOS / iOS Keychain, Windows Credential Manager, Linux Secret Service), or
//! memory on platforms without one the app can reach (Android, the web) — those secrets last until the app quits,
//! and the UI says so through [`SecretStore::persistent`].

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

use super::secret::Secret;

/// The keychain service every entry of the app is filed under.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
const SERVICE: &str = "EasyTierUI";

/// Why the keychain could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeychainError {
    /// The keychain refused or failed; the message names the platform's reason, never the secret.
    #[error("the system keychain failed: {0}")]
    Platform(String),
}

/// A place secrets rest, keyed by an account name the caller chooses (`profile/<id>/network-secret`,
/// `server/<id>/session`, `host/admin-password`).
pub trait SecretStore: Send + Sync {
    /// The secret under `account`, if any.
    ///
    /// # Errors
    ///
    /// When the store cannot be read.
    fn get(&self, account: &str) -> Result<Option<Secret>, KeychainError>;

    /// Keeps `secret` under `account`, replacing what was there.
    ///
    /// # Errors
    ///
    /// When the store cannot be written.
    fn set(&self, account: &str, secret: &Secret) -> Result<(), KeychainError>;

    /// Forgets `account`; forgetting a missing one succeeds.
    ///
    /// # Errors
    ///
    /// When the store cannot be written.
    fn delete(&self, account: &str) -> Result<(), KeychainError>;

    /// Whether secrets survive the app quitting.
    fn persistent(&self) -> bool;
}

/// Secrets in memory: platforms without a reachable keychain, and tests.
#[derive(Default)]
pub struct MemoryStore(Mutex<BTreeMap<String, Secret>>);

impl SecretStore for MemoryStore {
    fn get(&self, account: &str) -> Result<Option<Secret>, KeychainError> {
        let entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(entries.get(account).cloned())
    }

    fn set(&self, account: &str, secret: &Secret) -> Result<(), KeychainError> {
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        entries.insert(account.to_owned(), secret.clone());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), KeychainError> {
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        entries.remove(account);
        Ok(())
    }

    fn persistent(&self) -> bool {
        false
    }
}

/// The OS keychain through the `keyring` crate.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
pub struct OsKeychain;

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
impl OsKeychain {
    fn entry(account: &str) -> Result<keyring::Entry, KeychainError> {
        keyring::Entry::new(SERVICE, account).map_err(platform_error)
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
fn platform_error(error: keyring::Error) -> KeychainError {
    // `keyring::Error`'s messages name the platform failure, never the stored value.
    KeychainError::Platform(error.to_string())
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
impl SecretStore for OsKeychain {
    fn get(&self, account: &str) -> Result<Option<Secret>, KeychainError> {
        match Self::entry(account)?.get_password() {
            Ok(value) => Ok(Some(Secret::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(platform_error(error)),
        }
    }

    fn set(&self, account: &str, secret: &Secret) -> Result<(), KeychainError> {
        Self::entry(account)?
            .set_password(secret.expose())
            .map_err(platform_error)
    }

    fn delete(&self, account: &str) -> Result<(), KeychainError> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(platform_error(error)),
        }
    }

    fn persistent(&self) -> bool {
        true
    }
}

/// The store this platform offers: the OS keychain where the app can reach one, memory elsewhere.
pub fn system_store() -> Arc<dyn SecretStore> {
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    ))]
    {
        Arc::new(OsKeychain)
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    )))]
    {
        Arc::new(MemoryStore::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_keeps_replaces_and_forgets() {
        let store = MemoryStore::default();
        assert_eq!(store.get("a").unwrap(), None);
        store.set("a", &Secret::new("one")).unwrap();
        store.set("a", &Secret::new("two")).unwrap();
        assert_eq!(store.get("a").unwrap(), Some(Secret::new("two")));
        store.delete("a").unwrap();
        store.delete("a").unwrap();
        assert_eq!(store.get("a").unwrap(), None);
        assert!(!store.persistent());
    }
}
