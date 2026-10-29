//! easytier-web's captured output: a bounded ring of redacted lines.

use std::{
    collections::VecDeque,
    io::{BufRead as _, Read as _},
    sync::{Mutex, PoisonError},
};

use crate::core::state::{LogLine, LogStream};

/// Lines kept; older ones are dropped.
const CAPACITY: usize = 500;
/// Characters kept per line.
const MAX_LINE_CHARS: usize = 2000;
/// Bytes read at once for one line, so a program that never prints a newline cannot grow memory.
const MAX_LINE_BYTES: u64 = 16 * 1024;

/// Names whose value is a secret when written `name=value`, `name: value` or `"name":"value"`.
const SECRET_KEYS: [&str; 10] = [
    "password",
    "passwd",
    "secret",
    "token",
    "cookie",
    "authorization",
    "bearer",
    "api_key",
    "api-key",
    "credential",
];

/// A bounded, shared ring of output lines. Every line is redacted on the way in.
#[derive(Debug, Default)]
pub struct LogRing {
    lines: Mutex<VecDeque<LogLine>>,
}

impl LogRing {
    /// Adds a line (redacted, truncated by characters), dropping the oldest when full.
    pub fn push(&self, stream: LogStream, text: &str) {
        let mut redacted = redact(text);
        if redacted.chars().count() > MAX_LINE_CHARS {
            redacted = redacted.chars().take(MAX_LINE_CHARS).collect();
        }
        let line = LogLine {
            at_ms: nsh_core::time::unix_ms(),
            stream,
            text: redacted,
        };
        let mut lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        if lines.len() >= CAPACITY {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    /// All kept lines, oldest first.
    pub fn snapshot(&self) -> Vec<LogLine> {
        let lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        lines.iter().cloned().collect()
    }

    /// The last `count` lines as text, for a failure message.
    pub fn tail_text(&self, count: usize) -> String {
        let lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        let skip = lines.len().saturating_sub(count);
        lines
            .iter()
            .skip(skip)
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Reads `source` to its end, one line at a time, into the ring. Runs on a reader thread; ends when the process
    /// closes the stream.
    pub fn pump(&self, stream: LogStream, source: impl std::io::Read) {
        let mut reader = std::io::BufReader::new(source);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader
                .by_ref()
                .take(MAX_LINE_BYTES)
                .read_until(b'\n', &mut buffer)
            {
                Ok(0) => return,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&buffer);
                    self.push(stream, text.trim_end_matches(['\r', '\n']));
                }
                Err(error) => {
                    log::debug!("easytier-web output ended: {error}");
                    return;
                }
            }
        }
    }
}

/// Replaces the value of anything that looks like `password=…`, `"token": "…"`, `Cookie: …` with `***`.
///
/// Web output is not expected to carry secrets; this is the belt for the day a version prints a request.
pub fn redact(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut cursor = 0_usize;
    while let Some(rest) = lower.get(cursor..) {
        let Some((offset, key)) = SECRET_KEYS
            .iter()
            .filter_map(|key| rest.find(key).map(|at| (at, *key)))
            .min_by_key(|(at, _)| *at)
        else {
            break;
        };
        let key_end = cursor.saturating_add(offset).saturating_add(key.len());
        // Skip the rest of an identifier (`password_hash`), quotes and spaces, then need a separator.
        let mut at = key_end;
        while bytes
            .get(at)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
        {
            at = at.saturating_add(1);
        }
        while bytes
            .get(at)
            .is_some_and(|b| matches!(b, b'"' | b'\'' | b' '))
        {
            at = at.saturating_add(1);
        }
        if !bytes.get(at).is_some_and(|b| matches!(b, b'=' | b':')) {
            out.push_str(line.get(cursor..key_end).unwrap_or_default());
            cursor = key_end;
            continue;
        }
        at = at.saturating_add(1);
        while bytes
            .get(at)
            .is_some_and(|b| matches!(b, b'"' | b'\'' | b' '))
        {
            at = at.saturating_add(1);
        }
        let value_start = at;
        while bytes.get(at).is_some_and(|b| {
            !b.is_ascii_whitespace()
                && !matches!(b, b'"' | b'\'' | b',' | b';' | b'&' | b'}' | b')')
        }) {
            at = at.saturating_add(1);
        }
        if at == value_start {
            out.push_str(line.get(cursor..value_start).unwrap_or_default());
            cursor = value_start;
            continue;
        }
        out.push_str(line.get(cursor..value_start).unwrap_or_default());
        out.push_str("***");
        cursor = at;
    }
    out.push_str(line.get(cursor..).unwrap_or_default());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_values_are_masked() {
        assert_eq!(redact("login password=hunter2 ok"), "login password=*** ok");
        assert_eq!(
            redact(r#"{"username":"admin","password":"21232f29"}"#),
            r#"{"username":"admin","password":"***"}"#
        );
        assert_eq!(redact("Cookie: id=abcdef; Path=/"), "Cookie: ***; Path=/");
        assert_eq!(
            redact("user_token: \"alice\" connected"),
            "user_token: \"***\" connected"
        );
        assert_eq!(
            redact("password_hash=xyz and token=abc"),
            "password_hash=*** and token=***"
        );
    }

    #[test]
    fn ordinary_lines_pass_unchanged_including_cjk() {
        for line in [
            "Database not found, creating a new one",
            "listening on 127.0.0.1:11211",
            "tokenizer ready",
            "密码已更新 password",
            "secret:",
        ] {
            assert_eq!(redact(line), line);
        }
    }

    #[test]
    fn cjk_before_a_secret_keeps_char_boundaries() {
        assert_eq!(redact("用户 password=abc 完成"), "用户 password=*** 完成");
    }

    #[test]
    fn the_ring_is_bounded_and_truncates_by_characters() {
        let ring = LogRing::default();
        for n in 0..(CAPACITY + 10) {
            ring.push(LogStream::Stdout, &format!("line {n}"));
        }
        let kept = ring.snapshot();
        assert_eq!(kept.len(), CAPACITY);
        assert_eq!(kept.first().unwrap().text, "line 10");
        ring.push(LogStream::Stderr, &"字".repeat(MAX_LINE_CHARS + 50));
        let last = ring.snapshot().pop().unwrap();
        assert_eq!(last.text.chars().count(), MAX_LINE_CHARS);
        assert_eq!(ring.tail_text(2).lines().count(), 2);
    }

    #[test]
    fn pump_splits_lines_and_survives_invalid_utf8() {
        let ring = LogRing::default();
        let input: &[u8] = b"first\r\nsecond password=abc\n\xff\xfe bad\nlast";
        ring.pump(LogStream::Stdout, input);
        let texts: Vec<String> = ring.snapshot().into_iter().map(|line| line.text).collect();
        assert_eq!(texts.len(), 4);
        assert_eq!(texts.first().map(String::as_str), Some("first"));
        assert_eq!(
            texts.get(1).map(String::as_str),
            Some("second password=***")
        );
        assert_eq!(texts.get(3).map(String::as_str), Some("last"));
    }
}
