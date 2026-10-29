//! What the console pages show about a machine's core beyond its peer list: node facts, routes, foreign networks,
//! connectors, the VPN portal, ACL statistics, port forwards, metrics, the log level and the running config.
//!
//! Adapters in [`wire`](super::wire) fill them from `proxy-rpc` replies; nothing here names an EasyTier field.
//! Secrets the core carries in the same replies are either dropped by the adapter (the node's rendered config,
//! the foreign networks' secret digests) or kept as [`Secret`] (the VPN portal's client config).

use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use ipnet::{Ipv4Net, Ipv6Net};
use uuid::Uuid;

use super::model::PeerLinks;
use crate::core::{secret::Secret, state::NatType};

/// What a node advertises it can and wants to do (EasyTier's peer feature flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeerFeatures {
    /// A public server: it relays for others and needs no hole punching to be reached.
    pub is_public_server: bool,
    /// It refuses to relay other nodes' data.
    pub avoid_relay_data: bool,
    /// It accepts KCP-proxied TCP.
    pub kcp_input: bool,
    /// It refuses to relay KCP-proxied TCP.
    pub no_relay_kcp: bool,
    /// It can sync its connection list to other nodes.
    pub support_conn_list_sync: bool,
    /// It accepts QUIC-proxied TCP.
    pub quic_input: bool,
    /// It refuses to relay QUIC-proxied TCP.
    pub no_relay_quic: bool,
    /// It joined with a credential rather than the network secret.
    pub is_credential_peer: bool,
    /// It only wants direct connections.
    pub need_p2p: bool,
    /// It does not try to hole punch.
    pub disable_p2p: bool,
    /// It hands out public IPv6 addresses.
    pub ipv6_public_addr_provider: bool,
}

/// What STUN found out about a node's connectivity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StunFacts {
    /// The UDP NAT type; [`NatType::Unknown`] before STUN finished or for a code this app does not know.
    pub udp_nat_type: NatType,
    /// The TCP NAT type.
    pub tcp_nat_type: NatType,
    /// The public addresses STUN saw, IPv4 and IPv6.
    pub public_ips: Vec<IpAddr>,
    /// The lowest and highest public port seen; `None` before STUN ran.
    pub port_range: Option<(u16, u16)>,
    /// When STUN last ran, in Unix milliseconds; `None` when it never did.
    pub updated_at_ms: Option<i64>,
}

/// The node's own facts (`PeerManageRpcService/ShowNodeInfo`). The node's rendered config, which holds the network
/// secret, is never read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeFacts {
    /// The node's numeric peer id.
    pub peer_id: u32,
    /// The hostname it announces.
    pub hostname: String,
    /// The core's version (`2.6.4-8428a89d`).
    pub version: String,
    /// The network instance's id.
    pub instance_id: Option<Uuid>,
    /// The virtual IPv4 address with its prefix; `None` before one is assigned.
    pub virtual_ipv4: Option<Ipv4Net>,
    /// Where the node listens, as the core prints the URLs (including the in-process `ring://` one).
    pub listeners: Vec<String>,
    /// The subnets it proxies.
    pub proxy_cidrs: Vec<String>,
    /// What STUN found out.
    pub stun: StunFacts,
    /// The addresses of the machine's network interfaces, IPv4 first.
    pub interface_ips: Vec<IpAddr>,
    /// The public IPv4 address the core knows of.
    pub public_ipv4: Option<Ipv4Addr>,
    /// The public IPv6 address the core knows of.
    pub public_ipv6: Option<Ipv6Addr>,
    /// The public IPv6 address leased to this node, with its prefix length.
    pub public_ipv6_addr: Option<Ipv6Net>,
    /// The prefix this node leases public IPv6 addresses from.
    pub public_ipv6_prefix: Option<Ipv6Net>,
    /// Its feature flags.
    pub features: PeerFeatures,
}

/// One node in the routing table (`PeerManageRpcService/ListRoute`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RouteEntry {
    /// The node's peer id.
    pub peer_id: u32,
    /// Its hostname.
    pub hostname: String,
    /// Its network instance id.
    pub instance_id: Option<Uuid>,
    /// Its virtual IPv4 address with the prefix.
    pub virtual_ipv4: Option<Ipv4Net>,
    /// Its virtual IPv6 address with the prefix.
    pub virtual_ipv6: Option<Ipv6Net>,
    /// The peer id of the first hop towards it (itself for a direct link).
    pub next_hop_peer_id: u32,
    /// Hops to it: 1 is a direct link.
    pub cost: u32,
    /// The path latency in milliseconds; `None` when the core has no measurement.
    pub latency_ms: Option<u32>,
    /// Its core version.
    pub version: String,
    /// The subnets it proxies.
    pub proxy_cidrs: Vec<String>,
    /// What STUN found out about it.
    pub stun: StunFacts,
    /// Its feature flags.
    pub features: PeerFeatures,
}

/// A network this node relays for although it is not a member (`PeerManageRpcService/ListForeignNetwork`). The
/// network's secret digest is never read.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignNetwork {
    /// The foreign network's name.
    pub name: String,
    /// The peer id this node has inside that network.
    pub my_peer_id: u32,
    /// The foreign network's peers connected here, with their links.
    pub peers: Vec<PeerLinks>,
}

/// One foreign network seen somewhere in the mesh (`ListGlobalForeignNetwork`).
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalForeignNetwork {
    /// Its name.
    pub name: String,
    /// The ids of its peers connected to the relaying node.
    pub peer_ids: Vec<u32>,
    /// When the relaying node last reported it, in Unix milliseconds.
    pub updated_at_ms: Option<f64>,
    /// The report's version counter.
    pub version: u32,
}

/// The foreign networks one node of the mesh relays (`ListGlobalForeignNetwork`).
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignNetworkHost {
    /// The relaying node's peer id.
    pub peer_id: u32,
    /// The foreign networks it relays.
    pub networks: Vec<GlobalForeignNetwork>,
}

/// How many foreign networks and peers one node of the mesh relays (`GetForeignNetworkSummary`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignNetworkSummary {
    /// The relaying node's peer id.
    pub peer_id: u32,
    /// Foreign networks it relays.
    pub network_count: u32,
    /// Foreign peers connected to it.
    pub peer_count: u32,
}

/// A public IPv6 address a provider node leased to a peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicIpv6Lease {
    /// The peer it was leased to.
    pub peer_id: u32,
    /// That peer's instance id.
    pub instance_id: Option<Uuid>,
    /// The leased address with its prefix length.
    pub addr: Option<Ipv6Net>,
    /// When the lease ends, in Unix milliseconds; `None` when the core gave none.
    pub valid_until_ms: Option<i64>,
    /// Whether the peer got the address it had before.
    pub reused: bool,
}

/// This node as a public IPv6 provider (`PeerManageRpcService/ListPublicIpv6Info`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublicIpv6Info {
    /// The prefix it leases from; `None` when this node is no provider.
    pub provider_prefix: Option<Ipv6Net>,
    /// The leases it handed out.
    pub leases: Vec<PublicIpv6Lease>,
}

/// A direct link between two nodes as the peer center knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerMapLink {
    /// The node at the other end.
    pub peer_id: u32,
    /// The round-trip time in milliseconds.
    pub latency_ms: u32,
}

/// A node of the mesh and its direct links (`PeerCenterManageRpcService/GetGlobalPeerMap`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerMapNode {
    /// The node's peer id.
    pub peer_id: u32,
    /// The nodes it is directly connected to.
    pub direct_peers: Vec<PeerMapLink>,
}

/// Whether a manual connector's link is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectorStatus {
    /// Connected.
    Connected,
    /// Not connected; the core retries.
    Disconnected,
    /// Dialing.
    Connecting,
    /// A status this app does not know.
    #[default]
    Unknown,
}

impl ConnectorStatus {
    /// From EasyTier's numeric code.
    pub fn from_code(code: i64) -> Self {
        match code {
            0 => Self::Connected,
            1 => Self::Disconnected,
            2 => Self::Connecting,
            _ => Self::Unknown,
        }
    }
}

/// A peer address the node dials by itself (`ConnectorManageRpcService/ListConnector`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    /// The address, as the core prints it.
    pub url: String,
    /// Whether the link is up.
    pub status: ConnectorStatus,
}

/// The VPN portal (WireGuard) of an instance (`VpnPortalRpcService/GetVpnPortalInfo`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VpnPortalInfo {
    /// The portal's kind (`wireguard`); empty when the instance has no portal.
    pub vpn_type: String,
    /// A ready-made client config. **It holds the client's WireGuard private key**: show it only on request.
    pub client_config: Secret,
    /// The clients connected right now, as the core prints them.
    pub connected_clients: Vec<String>,
}

impl VpnPortalInfo {
    /// Whether the instance runs a portal.
    pub fn is_enabled(&self) -> bool {
        !self.vpn_type.trim().is_empty()
    }
}

/// An IP protocol an ACL rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AclProtocol {
    /// Not set.
    Unspecified,
    /// TCP.
    Tcp,
    /// UDP.
    Udp,
    /// ICMP.
    Icmp,
    /// ICMPv6.
    Icmpv6,
    /// Any protocol.
    Any,
    /// A protocol this app does not know.
    #[default]
    Unknown,
}

impl AclProtocol {
    /// From EasyTier's numeric code.
    pub fn from_code(code: i64) -> Self {
        match code {
            0 => Self::Unspecified,
            1 => Self::Tcp,
            2 => Self::Udp,
            3 => Self::Icmp,
            4 => Self::Icmpv6,
            5 => Self::Any,
            _ => Self::Unknown,
        }
    }
}

/// What an ACL rule does to a matching packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AclAction {
    /// Nothing (only counts).
    Noop,
    /// Lets the packet pass.
    Allow,
    /// Drops it silently.
    Drop,
    /// An action this app does not know.
    #[default]
    Unknown,
}

impl AclAction {
    /// From EasyTier's numeric code.
    pub fn from_code(code: i64) -> Self {
        match code {
            0 => Self::Noop,
            1 => Self::Allow,
            2 => Self::Drop,
            _ => Self::Unknown,
        }
    }
}

/// One ACL rule with how often it matched.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AclRuleStats {
    /// The rule's name.
    pub name: String,
    /// Its description.
    pub description: String,
    /// Higher wins.
    pub priority: u32,
    /// Whether the rule is active.
    pub enabled: bool,
    /// The protocol it matches.
    pub protocol: AclProtocol,
    /// The destination ports or ranges it matches; empty for any.
    pub ports: Vec<String>,
    /// The source ports or ranges it matches.
    pub source_ports: Vec<String>,
    /// The source address ranges it matches.
    pub source_ips: Vec<String>,
    /// The destination address ranges it matches.
    pub destination_ips: Vec<String>,
    /// The source groups it matches.
    pub source_groups: Vec<String>,
    /// The destination groups it matches.
    pub destination_groups: Vec<String>,
    /// What it does.
    pub action: AclAction,
    /// Packets per second allowed; 0 for no limit.
    pub rate_limit: u32,
    /// Whether it tracks connections.
    pub stateful: bool,
    /// Packets that matched it.
    pub packets: u64,
    /// Bytes that matched it.
    pub bytes: u64,
}

/// Where a tracked connection stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnTrackState {
    /// First packet seen.
    New,
    /// Both directions seen.
    Established,
    /// Related to an established connection.
    Related,
    /// Not valid.
    Invalid,
    /// A state this app does not know.
    #[default]
    Unknown,
}

impl ConnTrackState {
    /// From EasyTier's numeric code.
    pub fn from_code(code: i64) -> Self {
        match code {
            0 => Self::New,
            1 => Self::Established,
            2 => Self::Related,
            3 => Self::Invalid,
            _ => Self::Unknown,
        }
    }
}

/// A connection the stateful ACL tracks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclConnection {
    /// The source address.
    pub src: Option<SocketAddr>,
    /// The destination address.
    pub dst: Option<SocketAddr>,
    /// The protocol.
    pub protocol: AclProtocol,
    /// Its state.
    pub state: ConnTrackState,
    /// When it started, in Unix milliseconds.
    pub created_at_ms: Option<i64>,
    /// When a packet was last seen, in Unix milliseconds.
    pub last_seen_ms: Option<i64>,
    /// Packets seen.
    pub packets: u64,
    /// Bytes seen.
    pub bytes: u64,
}

/// The ACL's counters (`AclManageRpcService/GetAclStats`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AclStats {
    /// Rules with their hit counts.
    pub rules: Vec<AclRuleStats>,
    /// Tracked connections.
    pub connections: Vec<AclConnection>,
    /// Global counters by the core's name (`CacheSize`, `CacheMaxSize`, ...).
    pub global: BTreeMap<String, u64>,
}

/// The ports the node accepts traffic on from the mesh (`AclManageRpcService/GetWhitelist`). Entries are a port
/// (`80`) or a range (`1000-2000`), as the core stores them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Whitelist {
    /// TCP ports and ranges.
    pub tcp_ports: Vec<String>,
    /// UDP ports and ranges.
    pub udp_ports: Vec<String>,
}

/// A transport protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportProtocol {
    /// TCP.
    Tcp,
    /// UDP.
    Udp,
}

/// A port forward of a running instance (`PortForwardManageRpcService/ListPortForward`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortForwardEntry {
    /// The protocol.
    pub protocol: TransportProtocol,
    /// Where the node listens.
    pub bind: SocketAddr,
    /// Where the traffic goes.
    pub dst: SocketAddr,
}

/// One metric of the core (`StatsRpcService/GetStats`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metric {
    /// The metric's name.
    pub name: String,
    /// Its value (a counter or a gauge).
    pub value: u64,
    /// Its labels (`network_name`, `peer_id`, ...).
    pub labels: BTreeMap<String, String>,
}

/// The core process's log level (`LoggerRpcService`). The core sets it for the whole process, not per instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    /// No logging.
    #[default]
    Disabled,
    /// Errors only.
    Error,
    /// Warnings and errors.
    Warning,
    /// Informational messages and above.
    Info,
    /// Debugging output.
    Debug,
    /// Everything.
    Trace,
}

impl LogLevel {
    /// Every level, quietest first.
    pub const ALL: [Self; 6] = [
        Self::Disabled,
        Self::Error,
        Self::Warning,
        Self::Info,
        Self::Debug,
        Self::Trace,
    ];

    /// From EasyTier's numeric code; `None` for a level this app does not know.
    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(Self::Disabled),
            1 => Some(Self::Error),
            2 => Some(Self::Warning),
            3 => Some(Self::Info),
            4 => Some(Self::Debug),
            5 => Some(Self::Trace),
            _ => None,
        }
    }

    /// EasyTier's numeric code.
    pub fn code(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Error => 1,
            Self::Warning => 2,
            Self::Info => 3,
            Self::Debug => 4,
            Self::Trace => 5,
        }
    }
}
