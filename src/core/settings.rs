//! Network settings that local profiles and console configs share: the core's behavior switches, port forwards, the
//! WireGuard VPN portal and mapped listeners. Each side renders them in its own contract (the TOML config file,
//! easytier-web's JSON); the domain types and their defaults live here once.

use std::{fmt, net::SocketAddr};

use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};

use crate::core::net::{self, NetError, PeerUrl};

/// The port EasyTier's own GUI suggests for the VPN portal.
pub const DEFAULT_VPN_PORTAL_PORT: u16 = 22022;

/// The core's behavior switches that change how it connects and forwards, not what it exposes. Exposure switches
/// (exit node, private mode, relaying every peer's RPC, a TUN device, encryption) are separate fields with their
/// warnings.
///
/// Field names are a storage contract: a profile stores them flattened beside its other fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeFlags {
    /// Prefer the lowest-latency route over the fewest hops.
    pub latency_first: bool,
    /// Use a user-space TCP/IP stack for subnet proxy and KCP / QUIC proxy (avoids OS firewall problems).
    pub use_smoltcp: bool,
    /// Do not use IPv6 at all.
    pub disable_ipv6: bool,
    /// Take a public IPv6 address from a peer that shares an IPv6 subnet.
    pub ipv6_public_addr_auto: bool,
    /// Carry TCP over KCP between peers.
    pub enable_kcp_proxy: bool,
    /// Refuse KCP from peers (they fall back to TCP).
    pub disable_kcp_input: bool,
    /// Carry TCP over QUIC between peers.
    pub enable_quic_proxy: bool,
    /// Refuse QUIC from peers (they fall back to TCP).
    pub disable_quic_input: bool,
    /// No automatic direct links (peers asking for P2P still get one).
    pub disable_p2p: bool,
    /// Talk only to peers with a direct link, never through relays.
    pub p2p_only: bool,
    /// Try a direct link only when traffic needs that peer.
    pub lazy_p2p: bool,
    /// Ask peers for a direct link even when they are lazy about it.
    pub need_p2p: bool,
    /// Make connections only through physical network interfaces, not other VPNs' virtual ones.
    pub bind_device: bool,
    /// Run the core on several threads.
    pub multi_thread: bool,
    /// Forward subnet-proxy traffic through the OS kernel instead of the core's own NAT.
    pub proxy_forward_by_system: bool,
    /// No TCP hole punching.
    pub disable_tcp_hole_punching: bool,
    /// No UDP hole punching.
    pub disable_udp_hole_punching: bool,
    /// Treat symmetric NATs as cone NATs (no birthday-attack punching).
    pub disable_sym_hole_punching: bool,
    /// Relay this device's UDP broadcasts to peers (Windows only; LAN game discovery).
    pub enable_udp_broadcast_relay: bool,
    /// No UPnP / NAT-PMP port mapping.
    pub disable_upnp: bool,
    /// Resolve peers' hostnames through EasyTier's built-in DNS (magic DNS).
    pub accept_dns: bool,
}

impl Default for NodeFlags {
    /// EasyTier's defaults: multi-threaded, bound to physical interfaces, everything else off.
    fn default() -> Self {
        Self {
            latency_first: false,
            use_smoltcp: false,
            disable_ipv6: false,
            ipv6_public_addr_auto: false,
            enable_kcp_proxy: false,
            disable_kcp_input: false,
            enable_quic_proxy: false,
            disable_quic_input: false,
            disable_p2p: false,
            p2p_only: false,
            lazy_p2p: false,
            need_p2p: false,
            bind_device: true,
            multi_thread: true,
            proxy_forward_by_system: false,
            disable_tcp_hole_punching: false,
            disable_udp_hole_punching: false,
            disable_sym_hole_punching: false,
            enable_udp_broadcast_relay: false,
            disable_upnp: false,
            accept_dns: false,
        }
    }
}

/// How the editors group the switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlagGroup {
    /// Routes and direct links.
    Routing,
    /// NAT traversal.
    Traversal,
    /// TCP over KCP / QUIC.
    Transport,
    /// The network stack and system integration.
    System,
}

impl FlagGroup {
    /// Every group, in the order the editors show them.
    pub const ALL: [Self; 4] = [
        Self::Routing,
        Self::Traversal,
        Self::Transport,
        Self::System,
    ];
}

/// One switch of [`NodeFlags`], so the editors can list them in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Flag {
    /// [`NodeFlags::latency_first`].
    LatencyFirst,
    /// [`NodeFlags::disable_p2p`].
    DisableP2p,
    /// [`NodeFlags::p2p_only`].
    P2pOnly,
    /// [`NodeFlags::lazy_p2p`].
    LazyP2p,
    /// [`NodeFlags::need_p2p`].
    NeedP2p,
    /// [`NodeFlags::bind_device`].
    BindDevice,
    /// [`NodeFlags::disable_tcp_hole_punching`].
    DisableTcpHolePunching,
    /// [`NodeFlags::disable_udp_hole_punching`].
    DisableUdpHolePunching,
    /// [`NodeFlags::disable_sym_hole_punching`].
    DisableSymHolePunching,
    /// [`NodeFlags::disable_upnp`].
    DisableUpnp,
    /// [`NodeFlags::enable_kcp_proxy`].
    EnableKcpProxy,
    /// [`NodeFlags::disable_kcp_input`].
    DisableKcpInput,
    /// [`NodeFlags::enable_quic_proxy`].
    EnableQuicProxy,
    /// [`NodeFlags::disable_quic_input`].
    DisableQuicInput,
    /// [`NodeFlags::multi_thread`].
    MultiThread,
    /// [`NodeFlags::use_smoltcp`].
    UseSmoltcp,
    /// [`NodeFlags::disable_ipv6`].
    DisableIpv6,
    /// [`NodeFlags::ipv6_public_addr_auto`].
    Ipv6PublicAddrAuto,
    /// [`NodeFlags::accept_dns`].
    MagicDns,
    /// [`NodeFlags::proxy_forward_by_system`].
    ProxyForwardBySystem,
    /// [`NodeFlags::enable_udp_broadcast_relay`].
    UdpBroadcastRelay,
}

impl Flag {
    /// Every switch, grouped in [`FlagGroup::ALL`] order.
    pub const ALL: [Self; 21] = [
        Self::LatencyFirst,
        Self::DisableP2p,
        Self::P2pOnly,
        Self::LazyP2p,
        Self::NeedP2p,
        Self::BindDevice,
        Self::DisableTcpHolePunching,
        Self::DisableUdpHolePunching,
        Self::DisableSymHolePunching,
        Self::DisableUpnp,
        Self::EnableKcpProxy,
        Self::DisableKcpInput,
        Self::EnableQuicProxy,
        Self::DisableQuicInput,
        Self::MultiThread,
        Self::UseSmoltcp,
        Self::DisableIpv6,
        Self::Ipv6PublicAddrAuto,
        Self::MagicDns,
        Self::ProxyForwardBySystem,
        Self::UdpBroadcastRelay,
    ];

    /// The switches of `group`, in order.
    pub fn of(group: FlagGroup) -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(move |flag| flag.group() == group)
    }

    /// The group it is shown in.
    pub const fn group(self) -> FlagGroup {
        match self {
            Self::LatencyFirst
            | Self::DisableP2p
            | Self::P2pOnly
            | Self::LazyP2p
            | Self::NeedP2p
            | Self::BindDevice => FlagGroup::Routing,
            Self::DisableTcpHolePunching
            | Self::DisableUdpHolePunching
            | Self::DisableSymHolePunching
            | Self::DisableUpnp => FlagGroup::Traversal,
            Self::EnableKcpProxy
            | Self::DisableKcpInput
            | Self::EnableQuicProxy
            | Self::DisableQuicInput => FlagGroup::Transport,
            Self::MultiThread
            | Self::UseSmoltcp
            | Self::DisableIpv6
            | Self::Ipv6PublicAddrAuto
            | Self::MagicDns
            | Self::ProxyForwardBySystem
            | Self::UdpBroadcastRelay => FlagGroup::System,
        }
    }

    /// Whether turning it on changes what peers can reach (shown with an exposure note): kernel forwarding of
    /// proxied subnets bypasses the core's NAT and its ACL forward chain.
    pub const fn widens_exposure(self) -> bool {
        matches!(self, Self::ProxyForwardBySystem)
    }

    /// A stable id for test ids and option lists (the field name).
    pub const fn id(self) -> &'static str {
        match self {
            Self::LatencyFirst => "latency_first",
            Self::DisableP2p => "disable_p2p",
            Self::P2pOnly => "p2p_only",
            Self::LazyP2p => "lazy_p2p",
            Self::NeedP2p => "need_p2p",
            Self::BindDevice => "bind_device",
            Self::DisableTcpHolePunching => "disable_tcp_hole_punching",
            Self::DisableUdpHolePunching => "disable_udp_hole_punching",
            Self::DisableSymHolePunching => "disable_sym_hole_punching",
            Self::DisableUpnp => "disable_upnp",
            Self::EnableKcpProxy => "enable_kcp_proxy",
            Self::DisableKcpInput => "disable_kcp_input",
            Self::EnableQuicProxy => "enable_quic_proxy",
            Self::DisableQuicInput => "disable_quic_input",
            Self::MultiThread => "multi_thread",
            Self::UseSmoltcp => "use_smoltcp",
            Self::DisableIpv6 => "disable_ipv6",
            Self::Ipv6PublicAddrAuto => "ipv6_public_addr_auto",
            Self::MagicDns => "accept_dns",
            Self::ProxyForwardBySystem => "proxy_forward_by_system",
            Self::UdpBroadcastRelay => "enable_udp_broadcast_relay",
        }
    }

    /// Its value in `flags`.
    pub const fn get(self, flags: &NodeFlags) -> bool {
        match self {
            Self::LatencyFirst => flags.latency_first,
            Self::DisableP2p => flags.disable_p2p,
            Self::P2pOnly => flags.p2p_only,
            Self::LazyP2p => flags.lazy_p2p,
            Self::NeedP2p => flags.need_p2p,
            Self::BindDevice => flags.bind_device,
            Self::DisableTcpHolePunching => flags.disable_tcp_hole_punching,
            Self::DisableUdpHolePunching => flags.disable_udp_hole_punching,
            Self::DisableSymHolePunching => flags.disable_sym_hole_punching,
            Self::DisableUpnp => flags.disable_upnp,
            Self::EnableKcpProxy => flags.enable_kcp_proxy,
            Self::DisableKcpInput => flags.disable_kcp_input,
            Self::EnableQuicProxy => flags.enable_quic_proxy,
            Self::DisableQuicInput => flags.disable_quic_input,
            Self::MultiThread => flags.multi_thread,
            Self::UseSmoltcp => flags.use_smoltcp,
            Self::DisableIpv6 => flags.disable_ipv6,
            Self::Ipv6PublicAddrAuto => flags.ipv6_public_addr_auto,
            Self::MagicDns => flags.accept_dns,
            Self::ProxyForwardBySystem => flags.proxy_forward_by_system,
            Self::UdpBroadcastRelay => flags.enable_udp_broadcast_relay,
        }
    }

    /// Sets its value in `flags`.
    pub const fn set(self, flags: &mut NodeFlags, on: bool) {
        let field = match self {
            Self::LatencyFirst => &mut flags.latency_first,
            Self::DisableP2p => &mut flags.disable_p2p,
            Self::P2pOnly => &mut flags.p2p_only,
            Self::LazyP2p => &mut flags.lazy_p2p,
            Self::NeedP2p => &mut flags.need_p2p,
            Self::BindDevice => &mut flags.bind_device,
            Self::DisableTcpHolePunching => &mut flags.disable_tcp_hole_punching,
            Self::DisableUdpHolePunching => &mut flags.disable_udp_hole_punching,
            Self::DisableSymHolePunching => &mut flags.disable_sym_hole_punching,
            Self::DisableUpnp => &mut flags.disable_upnp,
            Self::EnableKcpProxy => &mut flags.enable_kcp_proxy,
            Self::DisableKcpInput => &mut flags.disable_kcp_input,
            Self::EnableQuicProxy => &mut flags.enable_quic_proxy,
            Self::DisableQuicInput => &mut flags.disable_quic_input,
            Self::MultiThread => &mut flags.multi_thread,
            Self::UseSmoltcp => &mut flags.use_smoltcp,
            Self::DisableIpv6 => &mut flags.disable_ipv6,
            Self::Ipv6PublicAddrAuto => &mut flags.ipv6_public_addr_auto,
            Self::MagicDns => &mut flags.accept_dns,
            Self::ProxyForwardBySystem => &mut flags.proxy_forward_by_system,
            Self::UdpBroadcastRelay => &mut flags.enable_udp_broadcast_relay,
        };
        *field = on;
    }
}

/// Transport of a forwarded port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Proto {
    /// TCP.
    Tcp,
    /// UDP.
    Udp,
}

impl Proto {
    /// Both, in the order the editors offer them.
    pub const ALL: [Self; 2] = [Self::Tcp, Self::Udp];

    /// The name in the config file and in easytier-web's JSON.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }

    /// The protocol named `text` (case-insensitive).
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|proto| proto.name().eq_ignore_ascii_case(text.trim()))
    }
}

/// A local port forwarded to an address in the virtual network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PortForward {
    /// TCP or UDP.
    pub proto: Proto,
    /// Where this device listens (`127.0.0.1:8080` keeps it on this device; `0.0.0.0` opens it to the LAN).
    pub bind: SocketAddr,
    /// The virtual-network address and port it is forwarded to.
    pub dst: SocketAddr,
}

impl PortForward {
    /// Whether it listens beyond this device.
    pub fn is_public(&self) -> bool {
        !self.bind.ip().is_loopback()
    }

    /// Reads one line of the editors' forward list, `tcp 127.0.0.1:8080 10.144.144.2:80` (the protocol, where this
    /// device listens, where it goes); `field` names the entry in errors (`{field}.bind`, `{field}.dst`).
    ///
    /// # Errors
    ///
    /// [`ForwardLineError::Shape`] unless the line is a protocol and two addresses; the address errors of
    /// [`net::parse_socket_addr`].
    pub fn parse_line(field: &str, line: &str) -> Result<Self, ForwardLineError> {
        let mut parts = line.split_whitespace();
        let (Some(proto), Some(bind), Some(dst), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(ForwardLineError::Shape);
        };
        let proto = Proto::parse(proto).ok_or(ForwardLineError::Shape)?;
        let bind = net::parse_socket_addr(&format!("{field}.bind"), bind)
            .map_err(ForwardLineError::Address)?;
        let dst = net::parse_socket_addr(&format!("{field}.dst"), dst)
            .map_err(ForwardLineError::Address)?;
        Ok(Self { proto, bind, dst })
    }
}

impl fmt::Display for PortForward {
    /// The editors' line: `tcp 127.0.0.1:8080 10.144.144.2:80`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.proto.name(), self.bind, self.dst)
    }
}

/// Why a forward line was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ForwardLineError {
    /// Not `proto bind dst` with `tcp` or `udp`.
    #[error("expected a protocol and two addresses, such as tcp 127.0.0.1:8080 10.144.144.2:80")]
    Shape,
    /// An address is not `ip:port`.
    #[error(transparent)]
    Address(NetError),
}

/// The WireGuard VPN portal: plain WireGuard clients join the network through this node. It listens on every
/// interface (EasyTier binds `0.0.0.0:<port>`), so it is an exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VpnPortal {
    /// The addresses handed to WireGuard clients (`10.14.14.0/24`).
    pub client_cidr: Ipv4Net,
    /// The UDP port WireGuard listens on.
    pub listen_port: u16,
}

/// A public address of one of this node's listeners, announced to peers so they can connect to it
/// (`tcp://203.0.113.5:11223`): useful behind port forwarding the core cannot discover.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MappedListener(PeerUrl);

impl MappedListener {
    /// Parses `text` as a peer URL whose scheme can listen (not `http`, `https`, `txt` or `srv`).
    ///
    /// # Errors
    ///
    /// What [`PeerUrl::parse`] reports, and [`NetError::ConnectorOnly`] for a discovery-only scheme.
    pub fn parse(field: &str, text: &str) -> Result<Self, NetError> {
        let url = PeerUrl::parse(field, text)?;
        if url.scheme().is_connector_only() {
            return Err(NetError::ConnectorOnly {
                field: field.to_owned(),
                scheme: url.scheme(),
            });
        }
        Ok(Self(url))
    }

    /// The normalized URL text, as it is written to the config.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for MappedListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<String> for MappedListener {
    type Error = NetError;

    fn try_from(text: String) -> Result<Self, NetError> {
        Self::parse("mapped_listeners", &text)
    }
}

impl From<MappedListener> for String {
    fn from(url: MappedListener) -> Self {
        url.0.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_flag_reads_and_writes_its_own_field() {
        for flag in Flag::ALL {
            let mut flags = NodeFlags::default();
            let before = flag.get(&flags);
            flag.set(&mut flags, !before);
            assert_eq!(flag.get(&flags), !before, "{}", flag.id());
            let changed = Flag::ALL
                .into_iter()
                .filter(|other| other.get(&flags) != other.get(&NodeFlags::default()))
                .collect::<Vec<_>>();
            assert_eq!(changed, vec![flag]);
        }
        let groups: usize = FlagGroup::ALL
            .iter()
            .map(|group| Flag::of(*group).count())
            .sum();
        assert_eq!(groups, Flag::ALL.len());
    }

    #[test]
    fn stored_flags_keep_easytier_defaults_for_missing_fields() {
        let flags: NodeFlags = serde_json::from_str(r#"{"latency_first":true}"#).unwrap();
        assert!(flags.latency_first && flags.bind_device && flags.multi_thread);
    }

    #[test]
    fn mapped_listeners_refuse_discovery_schemes() {
        assert_eq!(
            MappedListener::parse("m", "tcp://203.0.113.5:11223")
                .unwrap()
                .as_str(),
            "tcp://203.0.113.5:11223"
        );
        assert!(matches!(
            MappedListener::parse("m", "txt://example.com"),
            Err(NetError::ConnectorOnly { .. })
        ));
    }
}
