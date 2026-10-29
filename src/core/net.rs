//! Peer and listener URLs, CIDRs, addresses and ports from untrusted input.
//!
//! Everything a person types or an imported file carries goes through here before it reaches a profile: the scheme
//! table is in one place ([`Scheme::info`]), URLs are parsed with `url::Url` and wrapped in [`PeerUrl`] /
//! [`ListenerUrl`] (valid by construction), and every error names the field it was found in. Errors never echo the
//! rejected text (it may be a secret pasted into the wrong field), only the field and what was wrong.

use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
};

use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use url::Url;

/// The port a bare listener `11010` expands from: tcp and udp use it, the other schemes add an offset.
pub const BASE_PORT: u16 = 11010;

/// The longest URL text accepted: far above any real peer address, far below anything that could hurt.
pub const MAX_URL_LEN: usize = 512;

/// Prefix lengths accepted for a node's own virtual address (`10.144.144.1/24`): wide enough for any private
/// range, narrow enough to leave room for hosts.
pub const STATIC_PREFIX_RANGE: (u8, u8) = (8, 30);

/// Prefix lengths accepted for a proxied subnet: a `/32` is one host, anything wider than `/8` would swallow
/// most of the address space.
pub const SUBNET_PREFIX_RANGE: (u8, u8) = (8, 32);

/// A URL scheme EasyTier understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scheme {
    /// `tcp://`
    Tcp,
    /// `udp://`
    Udp,
    /// `ws://`
    Ws,
    /// `wss://`
    Wss,
    /// `quic://`
    Quic,
    /// `wg://` (WireGuard transport)
    Wg,
    /// `faketcp://`
    Faketcp,
    /// `http://`: connector-only, a page that lists peers
    Http,
    /// `https://`: connector-only
    Https,
    /// `txt://`: connector-only, a DNS TXT record that lists peers
    Txt,
    /// `srv://`: connector-only, a DNS SRV record
    Srv,
}

/// One row of the scheme table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemeInfo {
    /// The scheme as written in a URL.
    pub name: &'static str,
    /// Whether it only makes sense as a peer (peer discovery), never as a listener.
    pub connector_only: bool,
    /// The port a listener without one binds; `None` for connector-only schemes.
    pub listener_port: Option<u16>,
    /// The port a peer URL without one dials; `None` where the name alone is enough (`txt`, `srv`).
    pub peer_port: Option<u16>,
}

impl Scheme {
    /// Every scheme, listener-capable ones first.
    pub const ALL: [Self; 11] = [
        Self::Tcp,
        Self::Udp,
        Self::Ws,
        Self::Wss,
        Self::Quic,
        Self::Wg,
        Self::Faketcp,
        Self::Http,
        Self::Https,
        Self::Txt,
        Self::Srv,
    ];

    /// The one scheme table. Listener ports are [`BASE_PORT`] plus the scheme's offset (tcp / udp 11010, ws / wg
    /// 11011, quic / wss 11012, faketcp 11013); a peer URL without a port uses the same, except ws (80) and wss (443).
    pub const fn info(self) -> SchemeInfo {
        const fn row(
            name: &'static str,
            listener_port: Option<u16>,
            peer_port: Option<u16>,
            connector_only: bool,
        ) -> SchemeInfo {
            SchemeInfo {
                name,
                connector_only,
                listener_port,
                peer_port,
            }
        }
        match self {
            Self::Tcp => row("tcp", Some(11010), Some(11010), false),
            Self::Udp => row("udp", Some(11010), Some(11010), false),
            Self::Ws => row("ws", Some(11011), Some(80), false),
            Self::Wss => row("wss", Some(11012), Some(443), false),
            Self::Quic => row("quic", Some(11012), Some(11012), false),
            Self::Wg => row("wg", Some(11011), Some(11011), false),
            Self::Faketcp => row("faketcp", Some(11013), Some(11013), false),
            Self::Http => row("http", None, Some(80), true),
            Self::Https => row("https", None, Some(443), true),
            Self::Txt => row("txt", None, None, true),
            Self::Srv => row("srv", None, None, true),
        }
    }

    /// The scheme as written in a URL.
    pub const fn name(self) -> &'static str {
        self.info().name
    }

    /// Whether it is only for peer discovery (`http`, `https`, `txt`, `srv`).
    pub const fn is_connector_only(self) -> bool {
        self.info().connector_only
    }

    /// The scheme named `text` (case-insensitive).
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|scheme| scheme.name().eq_ignore_ascii_case(text))
    }

    /// The schemes a listener may use.
    pub fn listener_schemes() -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(|scheme| !scheme.is_connector_only())
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why text was rejected. Each variant names the field; none repeats the rejected text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetError {
    /// Nothing was entered.
    #[error("{field}: this is empty")]
    Empty {
        /// The field.
        field: String,
    },
    /// The text is longer than any real value.
    #[error("{field}: longer than {max} characters")]
    TooLong {
        /// The field.
        field: String,
        /// The limit, in characters.
        max: usize,
    },
    /// Not a URL, or it contains whitespace or control characters.
    #[error("{field}: not a valid address")]
    Malformed {
        /// The field.
        field: String,
    },
    /// A scheme EasyTier does not know.
    #[error("{field}: unknown scheme \"{scheme}\"")]
    UnknownScheme {
        /// The field.
        field: String,
        /// The scheme, shortened to 16 characters.
        scheme: String,
    },
    /// `http`, `https`, `txt` and `srv` find peers; they cannot listen.
    #[error("{field}: \"{scheme}\" can only be used to find peers, not to listen")]
    ConnectorOnly {
        /// The field.
        field: String,
        /// The scheme.
        scheme: Scheme,
    },
    /// The URL has no host.
    #[error("{field}: the host is missing")]
    MissingHost {
        /// The field.
        field: String,
    },
    /// A listener binds an address, so its host must be an IP address.
    #[error("{field}: the host must be an IP address")]
    HostNotIp {
        /// The field.
        field: String,
    },
    /// Not a number from 1 to 65535.
    #[error("{field}: the port must be a number from 1 to 65535")]
    BadPort {
        /// The field.
        field: String,
    },
    /// A `ws` listener on port 80 or `wss` on 443 (or one without a port, which the core binds there): a URL cannot
    /// keep that port written, so the listener would silently change. Pick another port.
    #[error(
        "{field}: a \"{scheme}\" listener needs an explicit port other than the well-known one"
    )]
    WellKnownPort {
        /// The field.
        field: String,
        /// The scheme.
        scheme: Scheme,
    },
    /// The URL carries something this field cannot use.
    #[error("{field}: the address must not contain a {part}")]
    Unexpected {
        /// The field.
        field: String,
        /// `credentials`, `query`, `fragment` or `path`.
        part: &'static str,
    },
    /// Not an IP address, or not `ip:port`.
    #[error("{field}: not a valid IP address")]
    BadAddress {
        /// The field.
        field: String,
    },
    /// Not `a.b.c.d/len`.
    #[error("{field}: expected an IPv4 address with a prefix such as 10.144.144.1/24")]
    BadCidr {
        /// The field.
        field: String,
    },
    /// The prefix length is not a number in the allowed range.
    #[error("{field}: the prefix length must be from {min} to {max}")]
    BadPrefix {
        /// The field.
        field: String,
        /// Shortest prefix allowed.
        min: u8,
        /// Longest prefix allowed.
        max: u8,
    },
    /// A subnet must be written as its network address (`192.168.50.0/24`, not `192.168.50.7/24`).
    #[error("{field}: the address has bits set beyond the prefix; write the network address")]
    HostBits {
        /// The field.
        field: String,
    },
    /// An address that cannot be used here (loopback, multicast, unspecified, the network or broadcast address).
    #[error("{field}: this address cannot be used here")]
    Reserved {
        /// The field.
        field: String,
    },
}

impl NetError {
    /// The field the problem was found in.
    pub fn field(&self) -> &str {
        match self {
            Self::Empty { field }
            | Self::TooLong { field, .. }
            | Self::Malformed { field }
            | Self::UnknownScheme { field, .. }
            | Self::ConnectorOnly { field, .. }
            | Self::MissingHost { field }
            | Self::HostNotIp { field }
            | Self::BadPort { field }
            | Self::WellKnownPort { field, .. }
            | Self::Unexpected { field, .. }
            | Self::BadAddress { field }
            | Self::BadCidr { field }
            | Self::BadPrefix { field, .. }
            | Self::HostBits { field }
            | Self::Reserved { field } => field,
        }
    }

    /// The same problem reported under another field name (a row of a list, a nested key).
    #[must_use]
    pub fn in_field(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        match &mut self {
            Self::Empty { field }
            | Self::TooLong { field, .. }
            | Self::Malformed { field }
            | Self::UnknownScheme { field, .. }
            | Self::ConnectorOnly { field, .. }
            | Self::MissingHost { field }
            | Self::HostNotIp { field }
            | Self::BadPort { field }
            | Self::WellKnownPort { field, .. }
            | Self::Unexpected { field, .. }
            | Self::BadAddress { field }
            | Self::BadCidr { field }
            | Self::BadPrefix { field, .. }
            | Self::HostBits { field }
            | Self::Reserved { field } => *field = name,
        }
        self
    }
}

/// Which side of a connection a URL describes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Peer,
    Listener,
}

/// A parsed, checked endpoint URL.
struct Endpoint {
    scheme: Scheme,
    url: Url,
}

fn parse_endpoint(field: &str, text: &str, role: Role) -> Result<Endpoint, NetError> {
    let fail = |build: fn(String) -> NetError| build(field.to_owned());
    let text = text.trim();
    if text.is_empty() {
        return Err(NetError::Empty {
            field: field.to_owned(),
        });
    }
    if text.chars().count() > MAX_URL_LEN {
        return Err(NetError::TooLong {
            field: field.to_owned(),
            max: MAX_URL_LEN,
        });
    }
    // `Url::parse` silently drops tabs and newlines; an address with any is not one a person typed on purpose.
    if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(fail(|field| NetError::Malformed { field }));
    }
    let url = Url::parse(text).map_err(|error| match error {
        url::ParseError::EmptyHost => fail(|field| NetError::MissingHost { field }),
        url::ParseError::InvalidPort => fail(|field| NetError::BadPort { field }),
        _ => fail(|field| NetError::Malformed { field }),
    })?;
    let Some(scheme) = Scheme::parse(url.scheme()) else {
        return Err(NetError::UnknownScheme {
            field: field.to_owned(),
            scheme: url.scheme().chars().take(16).collect(),
        });
    };
    if role == Role::Listener && scheme.is_connector_only() {
        return Err(NetError::ConnectorOnly {
            field: field.to_owned(),
            scheme,
        });
    }
    if url.cannot_be_a_base() || url.host_str().is_none_or(str::is_empty) {
        return Err(fail(|field| NetError::MissingHost { field }));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(NetError::Unexpected {
            field: field.to_owned(),
            part: "credentials",
        });
    }
    if url.query().is_some() {
        return Err(NetError::Unexpected {
            field: field.to_owned(),
            part: "query",
        });
    }
    if url.fragment().is_some() {
        return Err(NetError::Unexpected {
            field: field.to_owned(),
            part: "fragment",
        });
    }
    if url.port() == Some(0) {
        return Err(fail(|field| NetError::BadPort { field }));
    }
    // Only the web schemes carry a path; elsewhere a path means the person typed something else.
    let path_allowed = matches!(
        scheme,
        Scheme::Ws | Scheme::Wss | Scheme::Http | Scheme::Https
    );
    if !path_allowed && !matches!(url.path(), "" | "/") {
        return Err(NetError::Unexpected {
            field: field.to_owned(),
            part: "path",
        });
    }
    Ok(Endpoint { scheme, url })
}

/// The host of `url` as an IP address, if it is a literal one (`[::]` brackets included).
fn host_ip(url: &Url) -> Option<IpAddr> {
    let host = url.host_str()?;
    let host = host
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(host);
    host.parse().ok()
}

/// An address EasyTier dials: `tcp://host:11010`, `wss://relay.example.com`, `txt://peers.example.com`.
///
/// Valid by construction: a known scheme, a host, a port in range, nothing it cannot use.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PeerUrl {
    scheme: Scheme,
    url: Url,
}

impl PeerUrl {
    /// Parses `text`, reporting problems under `field`.
    ///
    /// # Errors
    ///
    /// [`NetError`] naming `field`: empty, over-long, malformed, unknown scheme, missing host, bad port, or a
    /// credential / query / fragment / path the scheme cannot carry.
    pub fn parse(field: &str, text: &str) -> Result<Self, NetError> {
        let Endpoint { scheme, url } = parse_endpoint(field, text, Role::Peer)?;
        Ok(Self { scheme, url })
    }

    /// The scheme.
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// The host as written (IPv6 in brackets).
    pub fn host(&self) -> &str {
        self.url.host_str().unwrap_or_default()
    }

    /// The port the URL names, if it does.
    pub fn port(&self) -> Option<u16> {
        self.url.port()
    }

    /// The port that will be dialed: the URL's, else the scheme's default (tcp / udp 11010, ws 80, wss 443, ...);
    /// `None` for `txt` and `srv`, which name a DNS record.
    pub fn effective_port(&self) -> Option<u16> {
        self.url.port().or(self.scheme.info().peer_port)
    }

    /// The normalized URL text, as it is written to the config file.
    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    /// The parsed URL.
    pub fn as_url(&self) -> &Url {
        &self.url
    }
}

impl fmt::Display for PeerUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PeerUrl {
    type Err = NetError;

    fn from_str(text: &str) -> Result<Self, NetError> {
        Self::parse("url", text)
    }
}

impl TryFrom<&str> for PeerUrl {
    type Error = NetError;

    fn try_from(text: &str) -> Result<Self, NetError> {
        Self::parse("url", text)
    }
}

impl TryFrom<String> for PeerUrl {
    type Error = NetError;

    fn try_from(text: String) -> Result<Self, NetError> {
        Self::parse("url", &text)
    }
}

impl From<PeerUrl> for String {
    fn from(url: PeerUrl) -> Self {
        url.url.into()
    }
}

/// An address EasyTier binds: `tcp://0.0.0.0:11010`, `udp://[::]:11010`, `ws://127.0.0.1:11011`.
///
/// Valid by construction: a listener-capable scheme, a literal IP address, a port in range. The port is always
/// written out: a URL without one is completed with the scheme's default (see [`Scheme::info`]), because
/// EasyTier itself binds a different default for a port-less `ws://` (80) or `wss://` (443) than for `-l ws:`
/// (11011, 11012), and the person must see the port that is bound.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ListenerUrl {
    scheme: Scheme,
    ip: IpAddr,
    url: Url,
}

impl ListenerUrl {
    /// Parses `text`, reporting problems under `field`. A missing port becomes the scheme's default
    /// (tcp / udp 11010, ws / wg 11011, quic / wss 11012, faketcp 11013).
    ///
    /// # Errors
    ///
    /// [`NetError`] naming `field`: as for [`PeerUrl::parse`], plus [`NetError::ConnectorOnly`] for `http`, `https`,
    /// `txt` and `srv`, [`NetError::HostNotIp`] for a host that is not an IP address, and
    /// [`NetError::WellKnownPort`] for `ws` on port 80 or `wss` on 443.
    pub fn parse(field: &str, text: &str) -> Result<Self, NetError> {
        Self::parse_with(field, text, false)
    }

    /// Parses a listener written in a config file. As [`ListenerUrl::parse`], except that a `ws://` or `wss://`
    /// without a port is refused: the core binds 80 / 443 for it, which a URL cannot spell out.
    ///
    /// # Errors
    ///
    /// As [`ListenerUrl::parse`], and [`NetError::WellKnownPort`] for a port-less `ws` or `wss`.
    pub fn parse_config(field: &str, text: &str) -> Result<Self, NetError> {
        Self::parse_with(field, text, true)
    }

    fn parse_with(field: &str, text: &str, from_config: bool) -> Result<Self, NetError> {
        let Endpoint { scheme, mut url } = parse_endpoint(field, text, Role::Listener)?;
        let Some(ip) = host_ip(&url) else {
            return Err(NetError::HostNotIp {
                field: field.to_owned(),
            });
        };
        if url.port().is_none() {
            // The URL standard drops a port equal to the scheme's well-known one (`ws://h:80` becomes `ws://h/`),
            // so the written port cannot be told from an absent one afterwards; refuse what cannot be kept.
            let written = matches!(scheme, Scheme::Ws | Scheme::Wss)
                && Url::parse(&format!("x{}", text.trim())).is_ok_and(|raw| raw.port().is_some());
            if written || (from_config && matches!(scheme, Scheme::Ws | Scheme::Wss)) {
                return Err(NetError::WellKnownPort {
                    field: field.to_owned(),
                    scheme,
                });
            }
            let port = scheme.info().listener_port.unwrap_or(BASE_PORT);
            url.set_port(Some(port)).map_err(|()| NetError::Malformed {
                field: field.to_owned(),
            })?;
        }
        Ok(Self { scheme, ip, url })
    }

    /// A listener of `scheme` on `ip`; without `port`, the scheme's default.
    ///
    /// # Errors
    ///
    /// [`NetError::ConnectorOnly`] for `http`, `https`, `txt` and `srv`; [`NetError::BadPort`] for port 0;
    /// [`NetError::WellKnownPort`] for `ws` on 80 or `wss` on 443.
    pub fn build(scheme: Scheme, ip: IpAddr, port: Option<u16>) -> Result<Self, NetError> {
        let host = match ip {
            IpAddr::V4(v4) => v4.to_string(),
            IpAddr::V6(v6) => format!("[{v6}]"),
        };
        let text = match port {
            Some(port) => format!("{scheme}://{host}:{port}"),
            None => format!("{scheme}://{host}"),
        };
        Self::parse("listener", &text)
    }

    /// The scheme.
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// The address it binds.
    pub fn ip(&self) -> IpAddr {
        self.ip
    }

    /// The port that is bound.
    pub fn effective_port(&self) -> u16 {
        self.url.port().unwrap_or(BASE_PORT)
    }

    /// Whether it binds every interface (`0.0.0.0` or `[::]`): reachable from the whole network the device is on,
    /// not only from this machine.
    pub fn exposes_all_interfaces(&self) -> bool {
        self.ip.is_unspecified()
    }

    /// The normalized URL text, as it is written to the config file.
    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    /// The parsed URL.
    pub fn as_url(&self) -> &Url {
        &self.url
    }
}

impl fmt::Display for ListenerUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ListenerUrl {
    type Err = NetError;

    fn from_str(text: &str) -> Result<Self, NetError> {
        Self::parse("url", text)
    }
}

impl TryFrom<&str> for ListenerUrl {
    type Error = NetError;

    fn try_from(text: &str) -> Result<Self, NetError> {
        Self::parse("url", text)
    }
}

impl TryFrom<String> for ListenerUrl {
    type Error = NetError;

    fn try_from(text: String) -> Result<Self, NetError> {
        Self::parse("url", &text)
    }
}

impl From<ListenerUrl> for String {
    fn from(url: ListenerUrl) -> Self {
        url.url.into()
    }
}

fn err_cidr(field: &str) -> NetError {
    NetError::BadCidr {
        field: field.to_owned(),
    }
}

/// Splits `a.b.c.d/len` and checks the prefix is within `range`.
fn split_cidr(field: &str, text: &str, range: (u8, u8)) -> Result<(Ipv4Addr, u8), NetError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(NetError::Empty {
            field: field.to_owned(),
        });
    }
    let (addr, prefix) = text.split_once('/').ok_or_else(|| err_cidr(field))?;
    let addr: Ipv4Addr = addr.parse().map_err(|_| err_cidr(field))?;
    let bad_prefix = || NetError::BadPrefix {
        field: field.to_owned(),
        min: range.0,
        max: range.1,
    };
    let prefix: u8 = prefix.parse().map_err(|_| bad_prefix())?;
    if !(range.0..=range.1).contains(&prefix) {
        return Err(bad_prefix());
    }
    Ok((addr, prefix))
}

/// Whether `addr` cannot be a node's address or a proxied network.
fn is_reserved(addr: Ipv4Addr) -> bool {
    addr.is_unspecified() || addr.is_loopback() || addr.is_multicast() || addr.is_broadcast()
}

/// Checks a node's own virtual address with its prefix (`10.144.144.1/24`): prefix within
/// [`STATIC_PREFIX_RANGE`], a usable host address (not loopback, multicast, unspecified, nor the network or
/// broadcast address of its own network).
///
/// # Errors
///
/// [`NetError::BadPrefix`] or [`NetError::Reserved`] under `field`.
pub fn check_ipv4_inet(field: &str, net: Ipv4Net) -> Result<Ipv4Net, NetError> {
    let (min, max) = STATIC_PREFIX_RANGE;
    if !(min..=max).contains(&net.prefix_len()) {
        return Err(NetError::BadPrefix {
            field: field.to_owned(),
            min,
            max,
        });
    }
    let addr = net.addr();
    if is_reserved(addr) || addr == net.network() || addr == net.broadcast() {
        return Err(NetError::Reserved {
            field: field.to_owned(),
        });
    }
    Ok(net)
}

/// Parses a node's own virtual address with its prefix (`10.144.144.1/24`); see [`check_ipv4_inet`].
///
/// # Errors
///
/// [`NetError`] under `field`: empty, not `a.b.c.d/len`, a prefix out of range, an unusable address.
pub fn parse_ipv4_inet(field: &str, text: &str) -> Result<Ipv4Net, NetError> {
    let (addr, prefix) = split_cidr(field, text, STATIC_PREFIX_RANGE)?;
    let net = Ipv4Net::new(addr, prefix).map_err(|_| err_cidr(field))?;
    check_ipv4_inet(field, net)
}

/// Checks a subnet to proxy (`192.168.50.0/24`): prefix within [`SUBNET_PREFIX_RANGE`], the network address
/// (no host bits), not loopback, multicast or unspecified.
///
/// # Errors
///
/// [`NetError::BadPrefix`], [`NetError::HostBits`] or [`NetError::Reserved`] under `field`.
pub fn check_ipv4_subnet(field: &str, net: Ipv4Net) -> Result<Ipv4Net, NetError> {
    let (min, max) = SUBNET_PREFIX_RANGE;
    if !(min..=max).contains(&net.prefix_len()) {
        return Err(NetError::BadPrefix {
            field: field.to_owned(),
            min,
            max,
        });
    }
    if net.addr() != net.network() {
        return Err(NetError::HostBits {
            field: field.to_owned(),
        });
    }
    if net.addr().is_unspecified() || net.addr().is_loopback() || net.addr().is_multicast() {
        return Err(NetError::Reserved {
            field: field.to_owned(),
        });
    }
    Ok(net)
}

/// Parses a subnet to proxy (`192.168.50.0/24`); see [`check_ipv4_subnet`].
///
/// # Errors
///
/// [`NetError`] under `field`: empty, not `a.b.c.d/len`, a prefix out of range, host bits set, an unusable network.
pub fn parse_ipv4_subnet(field: &str, text: &str) -> Result<Ipv4Net, NetError> {
    let (addr, prefix) = split_cidr(field, text, SUBNET_PREFIX_RANGE)?;
    let net = Ipv4Net::new(addr, prefix).map_err(|_| err_cidr(field))?;
    check_ipv4_subnet(field, net)
}

/// Parses an IP address (IPv4 or IPv6, brackets allowed).
///
/// # Errors
///
/// [`NetError::Empty`] or [`NetError::BadAddress`] under `field`.
pub fn parse_ip_addr(field: &str, text: &str) -> Result<IpAddr, NetError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(NetError::Empty {
            field: field.to_owned(),
        });
    }
    let text = text
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(text);
    text.parse().map_err(|_| NetError::BadAddress {
        field: field.to_owned(),
    })
}

/// Parses a TCP / UDP port number (1 to 65535).
///
/// # Errors
///
/// [`NetError::Empty`] or [`NetError::BadPort`] under `field`.
pub fn parse_port(field: &str, text: &str) -> Result<u16, NetError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(NetError::Empty {
            field: field.to_owned(),
        });
    }
    match text.parse::<u16>() {
        Ok(port) if port != 0 => Ok(port),
        _ => Err(NetError::BadPort {
            field: field.to_owned(),
        }),
    }
}

/// Parses `ip:port` (`127.0.0.1:8080`, `[::1]:8080`) with a port from 1 to 65535.
///
/// # Errors
///
/// [`NetError::Empty`], [`NetError::BadAddress`] or [`NetError::BadPort`] under `field`.
pub fn parse_socket_addr(field: &str, text: &str) -> Result<SocketAddr, NetError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(NetError::Empty {
            field: field.to_owned(),
        });
    }
    let addr: SocketAddr = text.parse().map_err(|_| NetError::BadAddress {
        field: field.to_owned(),
    })?;
    if addr.port() == 0 {
        return Err(NetError::BadPort {
            field: field.to_owned(),
        });
    }
    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_table_has_the_documented_ports() {
        let ports = |scheme: Scheme| (scheme.info().listener_port, scheme.info().peer_port);
        assert_eq!(ports(Scheme::Tcp), (Some(11010), Some(11010)));
        assert_eq!(ports(Scheme::Udp), (Some(11010), Some(11010)));
        assert_eq!(ports(Scheme::Ws), (Some(11011), Some(80)));
        assert_eq!(ports(Scheme::Wg), (Some(11011), Some(11011)));
        assert_eq!(ports(Scheme::Quic), (Some(11012), Some(11012)));
        assert_eq!(ports(Scheme::Wss), (Some(11012), Some(443)));
        assert_eq!(ports(Scheme::Faketcp), (Some(11013), Some(11013)));
        assert_eq!(ports(Scheme::Txt), (None, None));
        assert_eq!(Scheme::listener_schemes().count(), 7);
        assert_eq!(Scheme::parse("QUIC"), Some(Scheme::Quic));
        assert_eq!(Scheme::parse("ftp"), None);
    }

    #[test]
    fn peers_parse_and_show_the_port_they_will_dial() {
        let peer = PeerUrl::parse("peers[0]", "tcp://203.0.113.7").unwrap();
        assert_eq!(peer.port(), None);
        assert_eq!(peer.effective_port(), Some(11010));
        assert_eq!(peer.as_str(), "tcp://203.0.113.7");

        let ws: PeerUrl = "ws://relay.example.com".parse().unwrap();
        assert_eq!(ws.effective_port(), Some(80));
        let wss: PeerUrl = "wss://relay.example.com".parse().unwrap();
        assert_eq!(wss.effective_port(), Some(443));
        let quic: PeerUrl = "quic://relay.example.com".parse().unwrap();
        assert_eq!(quic.effective_port(), Some(11012));
        let explicit: PeerUrl = "udp://[2001:db8::1]:4000".parse().unwrap();
        assert_eq!(explicit.effective_port(), Some(4000));
        assert_eq!(explicit.host(), "[2001:db8::1]");
        let txt: PeerUrl = "txt://peers.example.com".parse().unwrap();
        assert_eq!(txt.effective_port(), None);
        assert!(txt.scheme().is_connector_only());
        let https = PeerUrl::try_from("https://example.com/peers").unwrap();
        assert_eq!(https.effective_port(), Some(443));
    }

    #[test]
    fn peers_reject_what_they_cannot_dial() {
        let reject = |text: &str| PeerUrl::parse("peers[2]", text).unwrap_err();
        assert!(matches!(
            reject("ftp://example.com"),
            NetError::UnknownScheme { .. }
        ));
        assert!(matches!(
            reject("example.com:11010"),
            NetError::UnknownScheme { .. }
        ));
        assert!(matches!(reject("tcp://"), NetError::MissingHost { .. }));
        assert!(matches!(
            reject("tcp:example.com"),
            NetError::MissingHost { .. }
        ));
        assert!(matches!(
            reject("tcp://example.com:0"),
            NetError::BadPort { .. }
        ));
        assert!(matches!(
            reject("tcp://example.com:70000"),
            NetError::BadPort { .. }
        ));
        assert!(matches!(
            reject("tcp://example.com:http"),
            NetError::BadPort { .. }
        ));
        assert!(matches!(reject(""), NetError::Empty { .. }));
        assert!(matches!(
            reject("tcp://exa mple.com"),
            NetError::Malformed { .. }
        ));
        assert!(matches!(
            reject("tcp://user:pw@example.com"),
            NetError::Unexpected {
                part: "credentials",
                ..
            }
        ));
        assert!(matches!(
            reject("tcp://example.com?x=1"),
            NetError::Unexpected { part: "query", .. }
        ));
        assert!(matches!(
            reject("tcp://example.com/x"),
            NetError::Unexpected { part: "path", .. }
        ));
        assert!(matches!(
            reject(&format!("tcp://{}", "a".repeat(600))),
            NetError::TooLong { .. }
        ));
        assert_eq!(reject("ftp://example.com").field(), "peers[2]");
    }

    #[test]
    fn errors_never_echo_the_rejected_text() {
        let error = PeerUrl::parse("peers[0]", "tcp://user:hunter2@example.com").unwrap_err();
        assert!(!error.to_string().contains("hunter2"));
        let error = PeerUrl::parse("peers[0]", "hunter2-not-a-url").unwrap_err();
        assert!(!error.to_string().contains("hunter2"));
    }

    #[test]
    fn listeners_show_the_port_and_flag_every_interface() {
        let all = ListenerUrl::parse("listeners[0]", "tcp://0.0.0.0:11010").unwrap();
        assert!(all.exposes_all_interfaces());
        assert_eq!(all.effective_port(), 11010);
        let v6: ListenerUrl = "udp://[::]:11010".parse().unwrap();
        assert!(v6.exposes_all_interfaces());
        let local: ListenerUrl = "ws://127.0.0.1".parse().unwrap();
        assert!(!local.exposes_all_interfaces());
        assert_eq!(local.effective_port(), 11011);
        assert_eq!(local.as_str(), "ws://127.0.0.1:11011/");
        let faketcp: ListenerUrl = "faketcp://10.0.0.5".parse().unwrap();
        assert_eq!(faketcp.effective_port(), 11013);
        let quic: ListenerUrl = "quic://192.168.1.2".parse().unwrap();
        assert_eq!(quic.effective_port(), 11012);
        assert_eq!(
            ListenerUrl::build(
                Scheme::Tcp,
                IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
                Some(4000)
            )
            .unwrap()
            .as_str(),
            "tcp://[::]:4000"
        );
    }

    #[test]
    fn listeners_reject_peer_discovery_schemes_names_and_swallowed_ports() {
        let reject = |text: &str| ListenerUrl::parse("listeners[1]", text).unwrap_err();
        assert!(matches!(
            reject("txt://example.com"),
            NetError::ConnectorOnly {
                scheme: Scheme::Txt,
                ..
            }
        ));
        assert!(matches!(
            reject("https://0.0.0.0"),
            NetError::ConnectorOnly { .. }
        ));
        assert!(matches!(
            reject("tcp://example.com:11010"),
            NetError::HostNotIp { .. }
        ));
        assert!(matches!(
            reject("gopher://0.0.0.0"),
            NetError::UnknownScheme { .. }
        ));
        assert!(matches!(
            reject("tcp://0.0.0.0:0"),
            NetError::BadPort { .. }
        ));
        assert!(matches!(
            reject("ws://0.0.0.0:80"),
            NetError::WellKnownPort {
                scheme: Scheme::Ws,
                ..
            }
        ));
        assert!(matches!(
            reject("wss://0.0.0.0:443"),
            NetError::WellKnownPort {
                scheme: Scheme::Wss,
                ..
            }
        ));
        // A config's port-less ws binds 80 in the core, which a URL cannot keep.
        assert!(matches!(
            ListenerUrl::parse_config("listeners[1]", "ws://0.0.0.0").unwrap_err(),
            NetError::WellKnownPort { .. }
        ));
        assert_eq!(
            ListenerUrl::parse_config("l", "tcp://0.0.0.0")
                .unwrap()
                .as_str(),
            "tcp://0.0.0.0:11010"
        );
        assert_eq!(reject("tcp://").field(), "listeners[1]");
    }

    #[test]
    fn urls_round_trip_through_json_and_validate_on_the_way_in() {
        let peer: PeerUrl = "tcp://203.0.113.7:11010".parse().unwrap();
        let json = serde_json::to_string(&peer).unwrap();
        assert_eq!(json, "\"tcp://203.0.113.7:11010\"");
        assert_eq!(serde_json::from_str::<PeerUrl>(&json).unwrap(), peer);
        assert!(serde_json::from_str::<PeerUrl>("\"nope://x\"").is_err());
        let listener: ListenerUrl = "udp://0.0.0.0:11010".parse().unwrap();
        let json = serde_json::to_string(&listener).unwrap();
        assert_eq!(
            serde_json::from_str::<ListenerUrl>(&json).unwrap(),
            listener
        );
    }

    #[test]
    fn static_addresses_need_a_sane_prefix_and_a_host_address() {
        let net = parse_ipv4_inet("ip", "10.144.144.1/24").unwrap();
        assert_eq!(net.to_string(), "10.144.144.1/24");
        assert!(matches!(
            parse_ipv4_inet("ip", "10.144.144.1/31"),
            Err(NetError::BadPrefix {
                min: 8,
                max: 30,
                ..
            })
        ));
        assert!(matches!(
            parse_ipv4_inet("ip", "10.144.144.1/4"),
            Err(NetError::BadPrefix { .. })
        ));
        assert!(matches!(
            parse_ipv4_inet("ip", "10.144.144.1/33"),
            Err(NetError::BadPrefix { .. })
        ));
        assert!(matches!(
            parse_ipv4_inet("ip", "10.144.144.1/x"),
            Err(NetError::BadPrefix { .. })
        ));
        assert!(matches!(
            parse_ipv4_inet("ip", "10.144.144.1"),
            Err(NetError::BadCidr { .. })
        ));
        assert!(matches!(
            parse_ipv4_inet("ip", "10.144.144/24"),
            Err(NetError::BadCidr { .. })
        ));
        assert!(matches!(
            parse_ipv4_inet("ip", "10.144.144.0/24"),
            Err(NetError::Reserved { .. })
        ));
        assert!(matches!(
            parse_ipv4_inet("ip", "10.144.144.255/24"),
            Err(NetError::Reserved { .. })
        ));
        assert!(matches!(
            parse_ipv4_inet("ip", "127.0.0.1/8"),
            Err(NetError::Reserved { .. })
        ));
        assert!(matches!(
            parse_ipv4_inet("ip", ""),
            Err(NetError::Empty { .. })
        ));
    }

    #[test]
    fn subnets_are_network_addresses() {
        assert_eq!(
            parse_ipv4_subnet("s", "192.168.50.0/24")
                .unwrap()
                .to_string(),
            "192.168.50.0/24"
        );
        assert_eq!(
            parse_ipv4_subnet("s", "192.168.50.7/32")
                .unwrap()
                .prefix_len(),
            32
        );
        assert!(matches!(
            parse_ipv4_subnet("s", "192.168.50.7/24"),
            Err(NetError::HostBits { .. })
        ));
        assert!(matches!(
            parse_ipv4_subnet("s", "0.0.0.0/0"),
            Err(NetError::BadPrefix { .. })
        ));
        assert!(matches!(
            parse_ipv4_subnet("s", "10.0.0.0/7"),
            Err(NetError::BadPrefix { .. })
        ));
        assert!(matches!(
            parse_ipv4_subnet("s", "127.0.0.0/8"),
            Err(NetError::Reserved { .. })
        ));
        assert!(matches!(
            parse_ipv4_subnet("s", "192.168.50.0"),
            Err(NetError::BadCidr { .. })
        ));
    }

    #[test]
    fn addresses_and_ports_parse_strictly() {
        assert_eq!(
            parse_ip_addr("a", "10.144.144.1").unwrap().to_string(),
            "10.144.144.1"
        );
        assert_eq!(
            parse_ip_addr("a", "[fd00::1]").unwrap().to_string(),
            "fd00::1"
        );
        assert!(matches!(
            parse_ip_addr("a", "example.com"),
            Err(NetError::BadAddress { .. })
        ));
        assert_eq!(parse_port("p", " 1080 ").unwrap(), 1080);
        assert!(matches!(
            parse_port("p", "0"),
            Err(NetError::BadPort { .. })
        ));
        assert!(matches!(
            parse_port("p", "65536"),
            Err(NetError::BadPort { .. })
        ));
        assert_eq!(
            parse_socket_addr("a", "127.0.0.1:8080").unwrap().port(),
            8080
        );
        assert_eq!(parse_socket_addr("a", "[::1]:8080").unwrap().port(), 8080);
        assert!(matches!(
            parse_socket_addr("a", "127.0.0.1:0"),
            Err(NetError::BadPort { .. })
        ));
        assert!(matches!(
            parse_socket_addr("a", "127.0.0.1"),
            Err(NetError::BadAddress { .. })
        ));
    }
}
