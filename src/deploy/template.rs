//! The artifact type and the small template machinery shared by the renderers.

/// What an artifact is written in, for syntax highlighting and the file's extension in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// Docker Compose.
    Yaml,
    /// systemd units and env files.
    Ini,
    /// A Caddyfile.
    Caddyfile,
    /// An nginx server block.
    Nginx,
    /// A POSIX / bash script.
    Sh,
    /// The README.
    Md,
}

/// One rendered file. Nothing is written anywhere: the page shows it, the user copies or saves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Suggested file name, no directory.
    pub file_name: String,
    /// Unix permission bits the file must get when written (`0o600` for a file with secrets, `0o755` for scripts).
    pub mode: Option<u32>,
    /// Syntax of [`Artifact::text`].
    pub language: Language,
    /// File content.
    pub text: String,
}

impl Artifact {
    pub(super) fn new(
        file_name: &str,
        mode: Option<u32>,
        language: Language,
        text: String,
    ) -> Self {
        Self {
            file_name: file_name.to_owned(),
            mode,
            language,
            text,
        }
    }
}

/// Replaces `{{key}}` markers in `template` with the matching value in one pass (a value is never scanned again,
/// so a value cannot inject a marker). Unknown markers stay as written; the tests assert none remain.
pub(super) fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((head, tail)) = rest.split_once("{{") {
        out.push_str(head);
        match tail.split_once("}}") {
            Some((key, after)) => {
                match vars.iter().find(|(name, _)| *name == key) {
                    Some((_, value)) => out.push_str(value),
                    None => {
                        out.push_str("{{");
                        out.push_str(key);
                        out.push_str("}}");
                    }
                }
                rest = after;
            }
            None => {
                out.push_str("{{");
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Quotes `value` for a POSIX shell: single quotes, with embedded single quotes closed and reopened.
pub(super) fn sh_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len().saturating_add(2));
    out.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// A YAML double-quoted scalar for `value`.
pub(super) fn yaml_str(value: &str) -> String {
    let mut out = String::with_capacity(value.len().saturating_add(2));
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A Caddyfile / nginx double-quoted string.
pub(super) fn conf_str(value: &str) -> String {
    let mut out = String::with_capacity(value.len().saturating_add(2));
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_is_single_pass() {
        assert_eq!(
            fill("a {{x}} b {{y}} {{z", &[("x", "{{y}}"), ("y", "2")]),
            "a {{y}} b 2 {{z"
        );
        assert_eq!(fill("{{nope}}", &[]), "{{nope}}");
    }

    #[test]
    fn quoting() {
        assert_eq!(sh_quote("a'b"), "'a'\\''b'");
        assert_eq!(yaml_str("a\"b\\\n"), "\"a\\\"b\\\\\\n\"");
        assert_eq!(conf_str("a\"b"), "\"a\\\"b\"");
    }
}
