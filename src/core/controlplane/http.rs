//! A small blocking HTTP/1.1 client for the hosted easytier-web on `127.0.0.1`.
//!
//! The control-plane host talks to its own child over loopback only, from a worker thread (`work::off_thread`), and
//! must do so in tests against the real binary without an NSH engine. A general client (TLS, other hosts, the web
//! build) is the `remote` module's job; this one refuses any address that is not loopback.

use std::{
    fmt,
    io::{Read as _, Write as _},
    net::{IpAddr, SocketAddr, TcpStream},
    time::Duration,
};

use zeroize::Zeroize as _;

use super::error::HostError;
use crate::core::secret::Secret;

/// How long a connection may take to open.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// How long one read or write may stall.
const IO_TIMEOUT: Duration = Duration::from_secs(10);
/// The largest response accepted: every reply this module reads is a few bytes.
const MAX_RESPONSE: u64 = 1024 * 1024;
/// The session cookie's name in easytier-web.
const SESSION_COOKIE: &str = "id";

/// A parsed response. Its headers may hold the session cookie, so `Debug` is written by hand without them.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Response {
    /// The status code.
    pub status: u16,
    /// Header names (lower-cased) and values.
    pub headers: Vec<(String, String)>,
    /// The body, with chunked encoding removed.
    pub body: Vec<u8>,
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

impl Response {
    /// The first header called `name` (lower-case).
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The session cookie a login response sets, if any.
    pub fn session_cookie(&self) -> Option<Secret> {
        self.headers
            .iter()
            .filter(|(key, _)| key == "set-cookie")
            .find_map(|(_, value)| parse_set_cookie(value))
    }

    /// The `message` of a `{"message": ".."}` error body, for log context (never a credential).
    pub fn message(&self) -> String {
        serde_json::from_slice::<serde_json::Value>(&self.body)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .map(|text| text.chars().take(200).collect())
            })
            .unwrap_or_default()
    }
}

/// `id=<value>; HttpOnly; ...` → the value, if the cookie is the session cookie.
fn parse_set_cookie(header: &str) -> Option<Secret> {
    let first = header.split(';').next()?;
    let (name, value) = first.split_once('=')?;
    (name.trim() == SESSION_COOKIE && !value.trim().is_empty()).then(|| Secret::new(value.trim()))
}

/// The request bytes. Holds the cookie and body, so the caller wipes the result when done.
fn build_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    cookie: Option<&Secret>,
    body: Option<&Secret>,
) -> Vec<u8> {
    let mut text = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nAccept: application/json\r\n"
    );
    if let Some(cookie) = cookie {
        text.push_str("Cookie: ");
        text.push_str(SESSION_COOKIE);
        text.push('=');
        text.push_str(cookie.expose());
        text.push_str("\r\n");
    }
    match body {
        Some(body) => {
            text.push_str("Content-Type: application/json\r\nContent-Length: ");
            text.push_str(&body.expose().len().to_string());
            text.push_str("\r\n\r\n");
            text.push_str(body.expose());
        }
        None if method == "POST" || method == "PUT" => {
            text.push_str("Content-Length: 0\r\n\r\n");
        }
        None => text.push_str("\r\n"),
    }
    let bytes = text.as_bytes().to_vec();
    text.zeroize();
    bytes
}

/// Parses a complete response (read until the server closed the connection).
fn parse_response(raw: &[u8]) -> Result<Response, HostError> {
    let malformed = |what: &str| HostError::Api(format!("malformed HTTP response: {what}"));
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| malformed("no header end"))?;
    let head = raw.get(..split).ok_or_else(|| malformed("head"))?;
    let rest = split
        .checked_add(4)
        .and_then(|start| raw.get(start..))
        .ok_or_else(|| malformed("body"))?;
    let head = String::from_utf8_lossy(head);
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or_else(|| malformed("status line"))?;
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| malformed("status code"))?;
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect();
    let mut response = Response {
        status,
        headers,
        body: Vec::new(),
    };
    let chunked = response
        .header("transfer-encoding")
        .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"));
    response.body = if chunked {
        decode_chunked(rest).ok_or_else(|| malformed("chunked body"))?
    } else {
        rest.to_vec()
    };
    Ok(response)
}

/// Decodes a chunked body; `None` when it is cut off or malformed.
fn decode_chunked(mut data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = data.windows(2).position(|window| window == b"\r\n")?;
        let size_line = String::from_utf8_lossy(data.get(..line_end)?);
        let size_text = size_line.split(';').next()?.trim();
        let size = usize::from_str_radix(size_text, 16).ok()?;
        data = data.get(line_end.checked_add(2)?..)?;
        if size == 0 {
            return Some(out);
        }
        out.extend_from_slice(data.get(..size)?);
        data = data.get(size.checked_add(2)?..)?;
    }
}

/// A client for one loopback address.
#[derive(Debug, Clone, Copy)]
pub(super) struct Loopback {
    addr: SocketAddr,
}

impl Loopback {
    /// A client for `addr`.
    ///
    /// # Errors
    ///
    /// [`HostError::Api`] when `addr` is not a loopback address: this client never leaves the machine.
    pub fn new(addr: SocketAddr) -> Result<Self, HostError> {
        match addr.ip() {
            IpAddr::V4(ip) if ip.is_loopback() => Ok(Self { addr }),
            IpAddr::V6(ip) if ip.is_loopback() => Ok(Self { addr }),
            IpAddr::V4(_) | IpAddr::V6(_) => Err(HostError::Api(
                "the control plane is managed over loopback only".to_owned(),
            )),
        }
    }

    /// Sends one request and reads the whole response.
    ///
    /// # Errors
    ///
    /// [`HostError::Api`] when the connection, the exchange or the response format fails.
    pub fn send(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&Secret>,
        body: Option<&Secret>,
    ) -> Result<Response, HostError> {
        let io_error = |error: std::io::Error| HostError::Api(error.to_string());
        let mut stream =
            TcpStream::connect_timeout(&self.addr, CONNECT_TIMEOUT).map_err(io_error)?;
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .map_err(io_error)?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .map_err(io_error)?;
        let mut request = build_request(self.addr, method, path, cookie, body);
        let written = stream.write_all(&request);
        request.zeroize();
        written.map_err(io_error)?;
        let mut raw = Vec::new();
        stream
            .take(MAX_RESPONSE)
            .read_to_end(&mut raw)
            .map_err(io_error)?;
        parse_response(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_carry_cookie_and_body_with_a_length() {
        let addr: SocketAddr = "127.0.0.1:11211".parse().unwrap();
        let body = Secret::new(r#"{"new_password":"abc"}"#);
        let cookie = Secret::new("session-value");
        let request = build_request(
            addr,
            "PUT",
            "/api/v1/auth/password",
            Some(&cookie),
            Some(&body),
        );
        let text = String::from_utf8(request).unwrap();
        assert!(text.starts_with("PUT /api/v1/auth/password HTTP/1.1\r\n"));
        assert!(text.contains("Host: 127.0.0.1:11211\r\n"));
        assert!(text.contains("Cookie: id=session-value\r\n"));
        assert!(text.contains("Content-Length: 22\r\n\r\n{\"new_password\":\"abc\"}"));
        let get = String::from_utf8(build_request(addr, "GET", "/x", None, None)).unwrap();
        assert!(get.ends_with("Accept: application/json\r\n\r\n"));
    }

    #[test]
    fn parses_status_headers_and_set_cookie() {
        let raw = b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nset-cookie: id=abc123; HttpOnly; SameSite=Lax; Path=/\r\ncontent-length: 2\r\n\r\n{}";
        let response = parse_response(raw).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"{}");
        assert_eq!(response.session_cookie().unwrap().expose(), "abc123");
    }

    #[test]
    fn other_cookies_are_not_the_session() {
        let raw = b"HTTP/1.1 200 OK\r\nSet-Cookie: theme=dark\r\n\r\n";
        assert!(parse_response(raw).unwrap().session_cookie().is_none());
    }

    #[test]
    fn error_bodies_yield_their_message() {
        let raw = b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 34\r\n\r\n{\"message\":\"Invalid credentials\"}";
        let response = parse_response(raw).unwrap();
        assert_eq!(response.status, 401);
        assert_eq!(response.message(), "Invalid credentials");
        let empty = parse_response(b"HTTP/1.1 401 Unauthorized\r\n\r\n").unwrap();
        assert_eq!(empty.message(), "");
    }

    #[test]
    fn decodes_chunked_bodies() {
        let raw = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n";
        assert_eq!(parse_response(raw).unwrap().body, b"{}");
        let cut = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n5\r\n{}";
        assert!(parse_response(cut).is_err());
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(parse_response(b"").is_err());
        assert!(parse_response(b"not http\r\n\r\n").is_err());
    }

    #[test]
    fn only_loopback_is_reachable() {
        assert!(Loopback::new("127.0.0.1:1".parse().unwrap()).is_ok());
        assert!(Loopback::new("[::1]:1".parse().unwrap()).is_ok());
        assert!(Loopback::new("192.0.2.1:1".parse().unwrap()).is_err());
    }
}
