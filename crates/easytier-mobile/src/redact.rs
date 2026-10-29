//! Keeps secrets out of everything the bridge hands back: log lines and error messages.
//!
//! EasyTier prints and returns text that can contain the config (its parse error echoes the whole file; some
//! messages carry values). Instead of trusting each message, the secret values of every running instance's config
//! are registered here and replaced by `***` wherever text leaves the bridge.

use std::sync::{Mutex, PoisonError};

/// What replaces a secret.
const MASK: &str = "***";

/// `(instance name, secret value)` of every instance that is starting or running.
static SECRETS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Whether a config key holds a secret by its name (`network_secret`, `local_private_key`, `credential_file` ...).
fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["secret", "password", "private_key", "token", "credential"]
        .iter()
        .any(|needle| key.contains(needle))
}

/// Every string value under a secret-looking key anywhere in `table`.
pub(crate) fn secrets_in(table: &toml::Table) -> Vec<String> {
    fn walk(value: &toml::Value, secret_context: bool, out: &mut Vec<String>) {
        match value {
            toml::Value::String(text) if secret_context && !text.is_empty() => {
                out.push(text.clone());
            }
            toml::Value::Table(table) => {
                for (key, inner) in table {
                    walk(inner, secret_context || is_secret_key(key), out);
                }
            }
            toml::Value::Array(items) => {
                for item in items {
                    walk(item, secret_context, out);
                }
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    for (key, value) in table {
        walk(value, is_secret_key(key), &mut out);
    }
    out.sort();
    out.dedup();
    out
}

/// Registers `secrets` as belonging to `instance` (replacing what it registered before).
pub(crate) fn register(instance: &str, secrets: Vec<String>) {
    let mut guard = SECRETS.lock().unwrap_or_else(PoisonError::into_inner);
    guard.retain(|(owner, _)| owner != instance);
    guard.extend(
        secrets
            .into_iter()
            .map(|secret| (instance.to_owned(), secret)),
    );
}

/// Forgets what `instance` registered (it has stopped; nothing more is printed for it).
pub(crate) fn forget(instance: &str) {
    SECRETS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(owner, _)| owner != instance);
}

/// `text` with every registered secret and every extra secret replaced by `***`.
pub(crate) fn scrub_with(text: &str, extra: &[String]) -> String {
    let guard = SECRETS.lock().unwrap_or_else(PoisonError::into_inner);
    let mut out = text.to_owned();
    for secret in guard
        .iter()
        .map(|(_, secret)| secret)
        .chain(extra.iter())
        .filter(|secret| !secret.is_empty())
    {
        if out.contains(secret.as_str()) {
            out = out.replace(secret.as_str(), MASK);
        }
    }
    out
}

/// `text` with every registered secret replaced by `***`.
pub(crate) fn scrub(text: &str) -> String {
    scrub_with(text, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_found_by_key_name_at_any_depth() {
        let table: toml::Table = r#"
            hostname = "phone"
            [network_identity]
            network_name = "home"
            network_secret = "example-secret-do-not-use"
            [[peer]]
            uri = "tcp://203.0.113.5:11010"
            peer_public_key = "AAAA"
            [secure_mode]
            local_private_key = "example-key-do-not-use"
        "#
        .parse()
        .unwrap();
        let found = secrets_in(&table);
        assert!(found.contains(&"example-secret-do-not-use".to_owned()));
        assert!(found.contains(&"example-key-do-not-use".to_owned()));
        assert!(!found.contains(&"phone".to_owned()));
        assert!(!found.contains(&"home".to_owned()));
    }

    #[test]
    fn registered_and_extra_secrets_are_masked() {
        register("p-test", vec!["registered-secret".to_owned()]);
        let out = scrub_with(
            "a registered-secret and an extra-secret",
            &["extra-secret".to_owned()],
        );
        assert_eq!(out, "a *** and an ***");
        forget("p-test");
        assert_eq!(scrub("registered-secret"), "registered-secret");
    }
}
