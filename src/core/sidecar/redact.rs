//! Redaction of everything the core or its CLI prints before it reaches a log line, an error message or the UI.
//!
//! Two layers: the instance's *known* secrets (every secret-looking string value of its rendered config, the
//! config-server URL) are replaced wherever they occur; and `key = value` / `"key": "value"` shapes whose key names
//! a secret are replaced even for values the app does not know (a secret the core generated or an echoed config).

use toml::Value;

use crate::core::secret::Secret;

/// Secrets shorter than this are not replaced by value: a three-character "secret" would blank unrelated words.
/// The key-shape layer still covers them.
const MIN_LITERAL_CHARS: usize = 4;

/// Key fragments that mark the value after them as secret (matched ASCII case-insensitively inside a key).
const SECRET_KEYS: [&str; 8] = [
    "secret",
    "password",
    "passwd",
    "private_key",
    "private-key",
    "token",
    "credential",
    "passphrase",
];

/// What replaces a redacted value.
pub const MASK: &str = "***";

/// Redacts text for one instance.
#[derive(Debug, Default)]
pub struct Redactor {
    /// Longest first, so a secret that contains another is replaced whole.
    literals: Vec<Secret>,
}

impl Redactor {
    /// A redactor that knows no secret by value (the key-shape layer still applies).
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `secret` (ignored when empty or very short).
    pub fn with_secret(mut self, secret: &str) -> Self {
        self.add(secret);
        self
    }

    /// Adds every secret-looking string value of a rendered EasyTier config (`network_secret`, key material,
    /// credentials) found anywhere in the TOML; text that does not parse adds nothing.
    pub fn with_config_toml(mut self, config: &str) -> Self {
        if let Ok(table) = config.parse::<toml::Table>() {
            for (key, value) in &table {
                self.collect(key, value);
            }
        }
        self
    }

    fn collect(&mut self, key: &str, value: &Value) {
        match value {
            Value::String(text) if is_secret_key(key) => self.add(text),
            Value::Table(table) => {
                for (child_key, child) in table {
                    self.collect(child_key, child);
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.collect(key, item);
                }
            }
            _ => {}
        }
    }

    fn add(&mut self, secret: &str) {
        if secret.chars().count() < MIN_LITERAL_CHARS || self.knows(secret) {
            return;
        }
        self.literals.push(Secret::new(secret));
        self.literals
            .sort_by_key(|literal| std::cmp::Reverse(literal.expose().len()));
    }

    fn knows(&self, secret: &str) -> bool {
        self.literals
            .iter()
            .any(|literal| literal.expose() == secret)
    }

    /// `text` with every known secret and every secret-keyed value replaced by [`MASK`].
    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for literal in &self.literals {
            out = out.replace(literal.expose(), MASK);
        }
        redact_keyed_values(&out)
    }
}

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SECRET_KEYS.iter().any(|fragment| lower.contains(fragment))
}

fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Whether `lower` has `key` starting at `at`.
fn key_at(lower: &[char], at: usize, key: &str) -> bool {
    key.chars().enumerate().all(|(offset, expected)| {
        at.checked_add(offset).and_then(|i| lower.get(i)) == Some(&expected)
    })
}

/// Replaces the value after every `<secret-ish key><quote?> (=|:) <value>` with [`MASK`].
fn redact_keyed_values(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = chars.iter().map(char::to_ascii_lowercase).collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0_usize;
    while let Some(&c) = chars.get(i) {
        let key_len = SECRET_KEYS
            .iter()
            .find(|key| key_at(&lower, i, key))
            .map(|key| key.chars().count());
        let Some(key_len) = key_len else {
            out.push(c);
            i = i.saturating_add(1);
            continue;
        };
        match keyed_value_span(&chars, i.saturating_add(key_len)) {
            Some((value_start, value_end)) => {
                out.extend(chars.get(i..value_start).unwrap_or_default());
                out.push_str(MASK);
                i = value_end;
            }
            None => {
                out.push(c);
                i = i.saturating_add(1);
            }
        }
    }
    out
}

/// After a secret key fragment ending at `from`: the span of its value, if the text is `key<…> = value`.
fn keyed_value_span(chars: &[char], from: usize) -> Option<(usize, usize)> {
    let mut at = from;
    while chars.get(at).copied().is_some_and(is_key_char) {
        at = at.saturating_add(1);
    }
    while matches!(chars.get(at), Some('"' | '\'')) {
        at = at.saturating_add(1);
    }
    while matches!(chars.get(at), Some(' ' | '\t')) {
        at = at.saturating_add(1);
    }
    if !matches!(chars.get(at), Some('=' | ':')) {
        return None;
    }
    at = at.saturating_add(1);
    if chars.get(at) == Some(&'>') {
        at = at.saturating_add(1);
    }
    while matches!(chars.get(at), Some(' ' | '\t')) {
        at = at.saturating_add(1);
    }
    let start = at;
    let end = match chars.get(at) {
        Some(&quote @ ('"' | '\'')) => {
            let mut end = at.saturating_add(1);
            while chars.get(end).is_some_and(|c| *c != quote) {
                end = end.saturating_add(1);
            }
            // Include the closing quote when there is one.
            if chars.get(end) == Some(&quote) {
                end = end.saturating_add(1);
            }
            end
        }
        _ => {
            let mut end = at;
            while chars
                .get(end)
                .is_some_and(|c| !c.is_whitespace() && !matches!(c, ',' | ';' | '}'))
            {
                end = end.saturating_add(1);
            }
            end
        }
    };
    (end > start).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "example-secret-do-not-use";

    #[test]
    fn known_secrets_are_replaced_anywhere() {
        let redactor = Redactor::new().with_secret(SECRET);
        let line = format!("joining with {SECRET}, again {SECRET}");
        assert_eq!(redactor.redact(&line), "joining with ***, again ***");
    }

    #[test]
    fn config_secrets_are_collected_by_key() {
        let config = format!(
            "hostname = \"example-host-a\"\n[network_identity]\nnetwork_name = \"example-net\"\nnetwork_secret = \"{SECRET}\"\n"
        );
        let redactor = Redactor::new().with_config_toml(&config);
        let shown = redactor.redact(&format!("peer says {SECRET} on example-net"));
        assert!(!shown.contains(SECRET));
        assert!(shown.contains("example-net"));
    }

    #[test]
    fn key_shapes_are_redacted_without_knowing_the_value() {
        let redactor = Redactor::new();
        assert_eq!(
            redactor.redact("network_secret = \"abc def\" rest"),
            "network_secret = *** rest"
        );
        assert_eq!(
            redactor.redact(r#"{"network_secret":"hunter2","name":"x"}"#),
            r#"{"network_secret":***,"name":"x"}"#
        );
        assert_eq!(
            redactor.redact("network_secret: Some(abc)"),
            "network_secret: ***"
        );
        assert_eq!(
            redactor.redact("--password=abc other"),
            "--password=*** other"
        );
        assert_eq!(
            redactor.redact("no secret here, secret_len is 5"),
            "no secret here, secret_len is 5"
        );
    }

    #[test]
    fn short_secrets_are_not_replaced_by_value() {
        let redactor = Redactor::new().with_secret("a");
        assert_eq!(redactor.redact("a banana"), "a banana");
    }

    #[test]
    fn the_cli_node_fixture_loses_its_secret() {
        let node = include_str!("../../../tests/fixtures/easytier-2.6.4/cli-node.json");
        let shown = Redactor::new().redact(node);
        assert!(!shown.contains(SECRET), "{shown}");
    }

    #[test]
    fn debug_output_hides_literals() {
        let redactor = Redactor::new().with_secret(SECRET);
        assert!(!format!("{redactor:?}").contains(SECRET));
    }
}
