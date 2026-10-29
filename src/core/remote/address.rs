//! Where an easytier-web server is: the serving origin (the web build) or a validated absolute base URL.

use std::fmt;

use url::{Host, Url};

use super::error::RemoteError;
use crate::core::target::TargetOrigin;

/// The REST API's prefix on the server.
const API_PREFIX: &str = "/api/v1";

/// An easytier-web server's REST base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerAddress {
    /// The origin that served this page (the web build, behind the reverse proxy that also serves EasyTier's
    /// REST API): requests use relative URLs, which the browser resolves against the page. On a native build a
    /// relative URL cannot be fetched, so the request fails with [`RemoteError::Network`].
    SameOrigin,
    /// An absolute base URL (`https://et.example.com`, `http://127.0.0.1:11211`, optionally with a path prefix
    /// behind a reverse proxy), validated and normalized: no trailing slash, no credentials, query or fragment.
    Base(String),
}

impl ServerAddress {
    /// Validates what a person typed or the store saved.
    ///
    /// The scheme must be `https`, except `http` to a loopback host (`127.0.0.1`, `::1`, `localhost`): the login
    /// sends an unsalted MD5 of the password, which protects nothing in transit. The base must not carry a user
    /// name, password, query or fragment (credentials in a URL end up in logs and history).
    ///
    /// # Errors
    ///
    /// [`RemoteError::InvalidAddress`] when it is not an `http(s)` URL with a host or carries credentials, a query
    /// or a fragment; [`RemoteError::InsecureTransport`] for `http` to a non-loopback host.
    pub fn parse(text: &str) -> Result<Self, RemoteError> {
        let text = text.trim();
        let url = Url::parse(text).map_err(|_| {
            RemoteError::InvalidAddress(
                "not a URL; start it with https:// (or http:// for this machine)".into(),
            )
        })?;
        let secure = match url.scheme() {
            "https" => true,
            "http" => false,
            _ => {
                return Err(RemoteError::InvalidAddress(
                    "the scheme must be https".into(),
                ));
            }
        };
        let Some(host) = url.host() else {
            return Err(RemoteError::InvalidAddress("no host".into()));
        };
        if !url.username().is_empty() || url.password().is_some() {
            return Err(RemoteError::InvalidAddress(
                "the address must not contain a user name or password".into(),
            ));
        }
        if url.query().is_some() {
            return Err(RemoteError::InvalidAddress(
                "the address must not contain a query".into(),
            ));
        }
        if url.fragment().is_some() {
            return Err(RemoteError::InvalidAddress(
                "the address must not contain a fragment".into(),
            ));
        }
        if !secure && !is_loopback(&host) {
            return Err(RemoteError::InsecureTransport);
        }
        Ok(Self::Base(url.as_str().trim_end_matches('/').to_owned()))
    }

    /// The address of a target's origin, as the store keeps it.
    ///
    /// # Errors
    ///
    /// [`RemoteError::InvalidAddress`] for [`TargetOrigin::Local`] (this device has no REST API) or an invalid
    /// URL; [`RemoteError::InsecureTransport`] as in [`ServerAddress::parse`].
    pub fn from_origin(origin: &TargetOrigin) -> Result<Self, RemoteError> {
        match origin {
            TargetOrigin::Local => Err(RemoteError::InvalidAddress(
                "this device is not a server".into(),
            )),
            TargetOrigin::SameOrigin => Ok(Self::SameOrigin),
            TargetOrigin::Url(url) => Self::parse(url),
            TargetOrigin::Loopback { port } => Self::parse(&format!("http://127.0.0.1:{port}")),
        }
    }

    /// The URL of `path` (starting with `/`, below `/api/v1`) on this server.
    pub fn url(&self, path: &str) -> String {
        match self {
            Self::SameOrigin => format!("{API_PREFIX}{path}"),
            Self::Base(base) => format!("{base}{API_PREFIX}{path}"),
        }
    }

    /// What to show for it: the base URL, or `None` for the serving origin (the page names it).
    pub fn base(&self) -> Option<&str> {
        match self {
            Self::SameOrigin => None,
            Self::Base(base) => Some(base),
        }
    }
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SameOrigin => f.write_str("this server"),
            Self::Base(base) => f.write_str(base),
        }
    }
}

/// Whether a host is this machine: `localhost` or a loopback address.
fn is_loopback(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(ip) => ip.is_loopback(),
        Host::Ipv6(ip) => ip.is_loopback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_is_required_except_on_loopback() {
        assert_eq!(
            ServerAddress::parse("https://et.example.com/").unwrap(),
            ServerAddress::Base("https://et.example.com".into())
        );
        for ok in [
            "http://127.0.0.1:11211",
            "http://localhost:11211/",
            "http://[::1]:11211",
            "http://LOCALHOST",
        ] {
            assert!(ServerAddress::parse(ok).is_ok(), "{ok}");
        }
        for insecure in [
            "http://et.example.com",
            "http://192.0.2.7:11211",
            "http://localhost.example.com",
        ] {
            assert_eq!(
                ServerAddress::parse(insecure),
                Err(RemoteError::InsecureTransport),
                "{insecure}"
            );
        }
    }

    #[test]
    fn trailing_slashes_and_default_ports_are_normalized() {
        assert_eq!(
            ServerAddress::parse("  https://et.example.com:443/panel//  ").unwrap(),
            ServerAddress::Base("https://et.example.com/panel".into())
        );
        let address = ServerAddress::parse("http://127.0.0.1:11211/").unwrap();
        assert_eq!(
            address.url("/auth/login"),
            "http://127.0.0.1:11211/api/v1/auth/login"
        );
    }

    #[test]
    fn credentials_query_fragment_and_odd_schemes_are_rejected() {
        for bad in [
            "https://admin:admin@et.example.com",
            "https://admin@et.example.com",
            "https://et.example.com?x=1",
            "https://et.example.com/#top",
            "ftp://et.example.com",
            "et.example.com",
            "127.0.0.1:11211",
            "https://",
            "",
        ] {
            assert!(
                matches!(
                    ServerAddress::parse(bad),
                    Err(RemoteError::InvalidAddress(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn same_origin_uses_relative_urls() {
        assert_eq!(ServerAddress::SameOrigin.url("/summary"), "/api/v1/summary");
        assert_eq!(ServerAddress::SameOrigin.base(), None);
    }

    #[test]
    fn target_origins_map_to_addresses() {
        assert_eq!(
            ServerAddress::from_origin(&TargetOrigin::Loopback { port: 11211 }).unwrap(),
            ServerAddress::Base("http://127.0.0.1:11211".into())
        );
        assert_eq!(
            ServerAddress::from_origin(&TargetOrigin::SameOrigin).unwrap(),
            ServerAddress::SameOrigin
        );
        assert!(ServerAddress::from_origin(&TargetOrigin::Local).is_err());
    }
}
