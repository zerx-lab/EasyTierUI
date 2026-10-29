//! Secrets: network secrets, easytier-web passwords and session cookies, enrollment URLs, private keys.
//!
//! A [`Secret`] never prints its value (`Debug` / `Display` write `***`), compares in constant time, wipes its
//! memory when dropped and does not implement `Serialize`: the only way out is [`Secret::expose`], called where the
//! value goes into the core's config file, a request body or the OS keychain (`rule://easytier-config-security`).

use std::fmt;

use serde::{Deserialize, Deserializer};
use zeroize::Zeroize;

/// Random bytes in a generated secret: 192 bits, above the 128-bit floor.
const GENERATED_BYTES: usize = 24;

/// Why a secret could not be generated.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    /// The platform has no cryptographically secure random source the app can reach (the web build).
    #[error("this platform has no secure random source")]
    NoRandomSource,
    /// The OS random source failed.
    #[error("the system's random source failed: {0}")]
    Random(String),
}

/// How guessable a secret a person typed is. EasyTier hashes network secrets with SipHash (not a password KDF), so
/// anything below [`Strength::Strong`] earns a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strength {
    /// Empty, short or common: trivial to guess offline.
    Weak,
    /// Long enough to resist casual guessing, not a generated secret.
    Fair,
    /// 20 or more characters from a mix of classes, or generated.
    Strong,
}

/// A value that must not leak.
#[derive(Clone, Default)]
pub struct Secret(String);

impl Secret {
    /// Wraps `value`.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The value, for the few places that must hand it on (config file, request body, keychain). Never log, format
    /// into an error, or show the result.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether it holds nothing.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// A fresh secret from the OS CSPRNG: 192 bits, URL-safe base64 without padding (32 characters).
    ///
    /// # Errors
    ///
    /// [`SecretError::NoRandomSource`] on the web build, [`SecretError::Random`] if the OS source fails.
    pub fn generate() -> Result<Self, SecretError> {
        let mut bytes = [0_u8; GENERATED_BYTES];
        fill_random(&mut bytes)?;
        let encoded = base64_url(&bytes);
        bytes.zeroize();
        Ok(Self(encoded))
    }

    /// How guessable it is, for a hint under a field the user types into.
    pub fn strength(&self) -> Strength {
        let value = self.0.as_str();
        let length = value.chars().count();
        if length < 12 || is_common(value) {
            return Strength::Weak;
        }
        let classes = [
            value.chars().any(|c| c.is_ascii_lowercase()),
            value.chars().any(|c| c.is_ascii_uppercase()),
            value.chars().any(|c| c.is_ascii_digit()),
            value.chars().any(|c| !c.is_ascii_alphanumeric()),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        if length >= 20 && classes >= 3 {
            Strength::Strong
        } else {
            Strength::Fair
        }
    }
}

impl PartialEq for Secret {
    /// Constant time in the length of the shorter value: no early exit on the first differing byte.
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (self.0.as_bytes(), other.0.as_bytes());
        let mut diff = u8::from(a.len() != b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }
}

impl Eq for Secret {}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self)
    }
}

/// Values people type that every guesser tries first.
fn is_common(value: &str) -> bool {
    const COMMON: [&str; 12] = [
        "password",
        "12345678",
        "123456789",
        "1234567890",
        "qwerty",
        "admin",
        "easytier",
        "secret",
        "changeme",
        "letmein",
        "11111111",
        "00000000",
    ];
    let lower = value.to_lowercase();
    COMMON.iter().any(|common| lower.contains(common))
}

/// URL-safe base64 without padding.
fn base64_url(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn fill_random(bytes: &mut [u8]) -> Result<(), SecretError> {
    getrandom::fill(bytes).map_err(|error| SecretError::Random(error.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn fill_random(_bytes: &mut [u8]) -> Result<(), SecretError> {
    Err(SecretError::NoRandomSource)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_prints_its_value() {
        let secret = Secret::new("example-secret-do-not-use");
        assert_eq!(format!("{secret}"), "***");
        assert_eq!(format!("{secret:?}"), "Secret(***)");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn generated_secrets_are_long_url_safe_and_distinct() {
        let a = Secret::generate().unwrap();
        let b = Secret::generate().unwrap();
        assert_eq!(a.expose().len(), 32);
        assert!(
            a.expose()
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert_ne!(a, b);
        assert_eq!(a.strength(), Strength::Strong);
    }

    #[test]
    fn typed_secrets_are_graded() {
        assert_eq!(Secret::new("").strength(), Strength::Weak);
        assert_eq!(Secret::new("hunter2").strength(), Strength::Weak);
        assert_eq!(
            Secret::new("my-easytier-net-2026").strength(),
            Strength::Weak
        );
        assert_eq!(
            Secret::new("correct horse battery").strength(),
            Strength::Fair
        );
        assert_eq!(
            Secret::new("Tr0ub4dor&3-horse-staple").strength(),
            Strength::Strong
        );
    }

    #[test]
    fn equality_needs_equal_length_and_bytes() {
        assert_eq!(Secret::new("abc"), Secret::new("abc"));
        assert_ne!(Secret::new("abc"), Secret::new("abd"));
        assert_ne!(Secret::new("abc"), Secret::new("abcd"));
    }
}
