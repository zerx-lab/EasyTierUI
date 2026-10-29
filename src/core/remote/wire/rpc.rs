//! The replies of the other `proxy-rpc` services, read tolerantly (the module is a child of [`wire`](super) to share
//! its address, url and link shapes).
//!
//! Three replies carry secrets and are read field by field so the secret never gets a field to land in: the
//! node info's rendered `config` (the network secret), the foreign networks' `network_secret_digest`, and the VPN
//! portal's `client_config` (a WireGuard private key, kept as a [`Secret`]).

use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use ipnet::Ipv6Net;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{AddrWire, InetWire, PeerInfoWire, UrlWire, inet, peer_links};
use crate::core::{
    remote::{
        api::decode,
        error::RemoteError,
        form::NetworkConfigForm,
        rpc_model::{
            AclAction, AclConnection, AclProtocol, AclRuleStats, AclStats, ConnTrackState,
            Connector, ConnectorStatus, ForeignNetwork, ForeignNetworkHost, ForeignNetworkSummary,
            GlobalForeignNetwork, LogLevel, Metric, NodeFacts, PeerFeatures, PeerMapLink,
            PeerMapNode, PortForwardEntry, PublicIpv6Info, PublicIpv6Lease, RouteEntry, StunFacts,
            TransportProtocol, VpnPortalInfo, Whitelist,
        },
        timestamp::parse_rfc3339_ms,
    },
    secret::Secret,
    state::NatType,
};

// --- Shared shapes ------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct Ipv6Wire {
    part1: u32,
    part2: u32,
    part3: u32,
    part4: u32,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct Inet6Wire {
    address: Option<Ipv6Wire>,
    network_length: u32,
}

/// `common.SocketAddr`: the oneof is written as the variant's name.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SocketWire {
    ip: Option<SocketIpWire>,
    port: u32,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SocketIpWire {
    #[serde(rename = "Ipv4")]
    ipv4: Option<AddrWire>,
    #[serde(rename = "Ipv6")]
    ipv6: Option<Ipv6Wire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct FeatureWire {
    is_public_server: bool,
    avoid_relay_data: bool,
    kcp_input: bool,
    no_relay_kcp: bool,
    support_conn_list_sync: bool,
    quic_input: bool,
    no_relay_quic: bool,
    is_credential_peer: bool,
    need_p2p: bool,
    disable_p2p: bool,
    ipv6_public_addr_provider: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct StunInfoWire {
    udp_nat_type: i64,
    tcp_nat_type: i64,
    last_update_time: i64,
    public_ip: Vec<String>,
    min_port: u32,
    max_port: u32,
}

fn ipv6(wire: &Ipv6Wire) -> Ipv6Addr {
    let [a0, a1, a2, a3] = wire.part1.to_be_bytes();
    let [b0, b1, b2, b3] = wire.part2.to_be_bytes();
    let [c0, c1, c2, c3] = wire.part3.to_be_bytes();
    let [d0, d1, d2, d3] = wire.part4.to_be_bytes();
    Ipv6Addr::from([
        a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3, d0, d1, d2, d3,
    ])
}

/// An IPv6 address with its prefix length; `None` for the unspecified address (not assigned).
fn inet6(wire: Option<&Inet6Wire>) -> Option<Ipv6Net> {
    let wire = wire?;
    let addr = ipv6(wire.address.as_ref()?);
    if addr.is_unspecified() {
        return None;
    }
    Ipv6Net::new(addr, u8::try_from(wire.network_length).ok()?).ok()
}

/// An IPv4 address; `None` for the unspecified one.
fn ipv4(wire: &AddrWire) -> Option<Ipv4Addr> {
    let addr = Ipv4Addr::from(wire.addr);
    (!addr.is_unspecified()).then_some(addr)
}

fn socket(wire: Option<&SocketWire>) -> Option<SocketAddr> {
    let wire = wire?;
    let ip = wire.ip.as_ref()?;
    let ip = match (&ip.ipv4, &ip.ipv6) {
        (Some(v4), _) => IpAddr::V4(Ipv4Addr::from(v4.addr)),
        (None, Some(v6)) => IpAddr::V6(ipv6(v6)),
        (None, None) => return None,
    };
    Some(SocketAddr::new(ip, u16::try_from(wire.port).ok()?))
}

fn features(wire: Option<FeatureWire>) -> PeerFeatures {
    let wire = wire.unwrap_or_default();
    PeerFeatures {
        is_public_server: wire.is_public_server,
        avoid_relay_data: wire.avoid_relay_data,
        kcp_input: wire.kcp_input,
        no_relay_kcp: wire.no_relay_kcp,
        support_conn_list_sync: wire.support_conn_list_sync,
        quic_input: wire.quic_input,
        no_relay_quic: wire.no_relay_quic,
        is_credential_peer: wire.is_credential_peer,
        need_p2p: wire.need_p2p,
        disable_p2p: wire.disable_p2p,
        ipv6_public_addr_provider: wire.ipv6_public_addr_provider,
    }
}

fn stun(wire: Option<StunInfoWire>) -> StunFacts {
    let wire = wire.unwrap_or_default();
    let port_range = match (u16::try_from(wire.min_port), u16::try_from(wire.max_port)) {
        (Ok(min), Ok(max)) if max > 0 => Some((min, max)),
        _ => None,
    };
    StunFacts {
        udp_nat_type: NatType::from_code(wire.udp_nat_type),
        tcp_nat_type: NatType::from_code(wire.tcp_nat_type),
        public_ips: wire
            .public_ip
            .iter()
            .filter_map(|text| text.trim().parse().ok())
            .collect(),
        port_range,
        updated_at_ms: seconds_to_ms(wire.last_update_time),
    }
}

/// Unix seconds as milliseconds; `None` for 0 (never) and for a value that does not fit.
fn seconds_to_ms(seconds: i64) -> Option<i64> {
    if seconds <= 0 {
        return None;
    }
    seconds.checked_mul(1000)
}

fn instance_id(text: &str) -> Option<Uuid> {
    text.trim().parse().ok()
}

// --- Node info ----------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ShowNodeInfoWire {
    node_info: Option<NodeInfoWire>,
}

/// `config` (the node's whole config, network secret included) has no field here on purpose.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct NodeInfoWire {
    peer_id: u32,
    ipv4_addr: String,
    proxy_cidrs: Vec<String>,
    hostname: String,
    stun_info: Option<StunInfoWire>,
    inst_id: String,
    listeners: Vec<String>,
    version: String,
    feature_flag: Option<FeatureWire>,
    ip_list: Option<IpListWire>,
    public_ipv6_addr: Option<Inet6Wire>,
    ipv6_public_addr_prefix: Option<Inet6Wire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct IpListWire {
    public_ipv4: Option<AddrWire>,
    interface_ipv4s: Vec<AddrWire>,
    public_ipv6: Option<Ipv6Wire>,
    interface_ipv6s: Vec<Ipv6Wire>,
}

/// Reads the reply of `PeerManageRpcService/ShowNodeInfo`.
pub(crate) fn parse_node_facts(body: &[u8]) -> Result<NodeFacts, RemoteError> {
    let wire: ShowNodeInfoWire = decode(body)?;
    let node = wire
        .node_info
        .ok_or_else(|| RemoteError::Decode("unexpected shape: no node info in the reply".into()))?;
    let ip_list = node.ip_list.unwrap_or_default();
    let interface_ips = ip_list
        .interface_ipv4s
        .iter()
        .filter_map(ipv4)
        .map(IpAddr::V4)
        .chain(
            ip_list
                .interface_ipv6s
                .iter()
                .map(ipv6)
                .filter(|addr| !addr.is_unspecified())
                .map(IpAddr::V6),
        )
        .collect();
    Ok(NodeFacts {
        peer_id: node.peer_id,
        hostname: node.hostname,
        version: node.version,
        instance_id: instance_id(&node.inst_id),
        virtual_ipv4: node
            .ipv4_addr
            .trim()
            .parse()
            .ok()
            .filter(|net: &ipnet::Ipv4Net| !net.addr().is_unspecified()),
        listeners: node.listeners,
        proxy_cidrs: node.proxy_cidrs,
        stun: stun(node.stun_info),
        interface_ips,
        public_ipv4: ip_list.public_ipv4.as_ref().and_then(ipv4),
        public_ipv6: ip_list
            .public_ipv6
            .as_ref()
            .map(ipv6)
            .filter(|addr| !addr.is_unspecified()),
        public_ipv6_addr: inet6(node.public_ipv6_addr.as_ref()),
        public_ipv6_prefix: inet6(node.ipv6_public_addr_prefix.as_ref()),
        features: features(node.feature_flag),
    })
}

// --- Routes -------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ListRouteWire {
    routes: Vec<RouteInfoWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RouteInfoWire {
    peer_id: u32,
    ipv4_addr: Option<InetWire>,
    ipv6_addr: Option<Inet6Wire>,
    next_hop_peer_id: u32,
    cost: i64,
    path_latency: i64,
    proxy_cidrs: Vec<String>,
    hostname: String,
    stun_info: Option<StunInfoWire>,
    inst_id: String,
    version: String,
    feature_flag: Option<FeatureWire>,
}

/// Reads the reply of `PeerManageRpcService/ListRoute`.
pub(crate) fn parse_routes(body: &[u8]) -> Result<Vec<RouteEntry>, RemoteError> {
    let wire: ListRouteWire = decode(body)?;
    Ok(wire
        .routes
        .into_iter()
        .map(|route| RouteEntry {
            peer_id: route.peer_id,
            hostname: route.hostname,
            instance_id: instance_id(&route.inst_id),
            virtual_ipv4: inet(route.ipv4_addr.as_ref()),
            virtual_ipv6: inet6(route.ipv6_addr.as_ref()),
            next_hop_peer_id: route.next_hop_peer_id,
            cost: u32::try_from(route.cost).unwrap_or_default(),
            latency_ms: u32::try_from(route.path_latency)
                .ok()
                .filter(|latency| *latency > 0),
            version: route.version,
            proxy_cidrs: route.proxy_cidrs,
            stun: stun(route.stun_info),
            features: features(route.feature_flag),
        })
        .collect())
}

// --- Foreign networks ---------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ListForeignWire {
    foreign_networks: BTreeMap<String, ForeignEntryWire>,
}

/// `network_secret_digest` and `trusted_keys` have no field here on purpose.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ForeignEntryWire {
    peers: Vec<PeerInfoWire>,
    my_peer_id_for_this_network: u32,
}

/// Reads the reply of `PeerManageRpcService/ListForeignNetwork`, in the order of the networks' names.
pub(crate) fn parse_foreign_networks(body: &[u8]) -> Result<Vec<ForeignNetwork>, RemoteError> {
    let wire: ListForeignWire = decode(body)?;
    Ok(wire
        .foreign_networks
        .into_iter()
        .map(|(name, entry)| ForeignNetwork {
            name,
            my_peer_id: entry.my_peer_id_for_this_network,
            peers: entry.peers.into_iter().map(peer_links).collect(),
        })
        .collect())
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ListGlobalForeignWire {
    foreign_networks: BTreeMap<String, GlobalForeignListWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct GlobalForeignListWire {
    foreign_networks: Vec<GlobalForeignWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct GlobalForeignWire {
    network_name: String,
    peer_ids: Vec<u32>,
    last_updated: String,
    version: u32,
}

/// Reads the reply of `PeerManageRpcService/ListGlobalForeignNetwork`, ordered by the relaying node's id. A key
/// that is not a peer id is skipped.
pub(crate) fn parse_global_foreign_networks(
    body: &[u8],
) -> Result<Vec<ForeignNetworkHost>, RemoteError> {
    let wire: ListGlobalForeignWire = decode(body)?;
    let mut hosts: Vec<ForeignNetworkHost> = wire
        .foreign_networks
        .into_iter()
        .filter_map(|(key, list)| {
            Some(ForeignNetworkHost {
                peer_id: key.parse().ok()?,
                networks: list
                    .foreign_networks
                    .into_iter()
                    .map(|network| GlobalForeignNetwork {
                        name: network.network_name,
                        peer_ids: network.peer_ids,
                        updated_at_ms: parse_rfc3339_ms(&network.last_updated),
                        version: network.version,
                    })
                    .collect(),
            })
        })
        .collect();
    hosts.sort_by_key(|host| host.peer_id);
    Ok(hosts)
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ForeignSummaryReplyWire {
    summary: Option<ForeignSummaryWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ForeignSummaryWire {
    info_map: BTreeMap<String, ForeignSummaryInfoWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ForeignSummaryInfoWire {
    network_count: u32,
    peer_count: u32,
}

/// Reads the reply of `PeerManageRpcService/GetForeignNetworkSummary`, ordered by the relaying node's id. The
/// node is the map's key: the `peer_id` inside the entry is 0 in 2.6.4.
pub(crate) fn parse_foreign_network_summary(
    body: &[u8],
) -> Result<Vec<ForeignNetworkSummary>, RemoteError> {
    let wire: ForeignSummaryReplyWire = decode(body)?;
    let mut rows: Vec<ForeignNetworkSummary> = wire
        .summary
        .unwrap_or_default()
        .info_map
        .into_iter()
        .filter_map(|(key, info)| {
            Some(ForeignNetworkSummary {
                peer_id: key.parse().ok()?,
                network_count: info.network_count,
                peer_count: info.peer_count,
            })
        })
        .collect();
    rows.sort_by_key(|row| row.peer_id);
    Ok(rows)
}

// --- Public IPv6 --------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct PublicIpv6Wire {
    provider_prefix: Option<Inet6Wire>,
    provider_leases: Vec<LeaseWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct LeaseWire {
    peer_id: u32,
    inst_id: String,
    leased_addr: Option<Inet6Wire>,
    valid_until_unix_seconds: i64,
    reused: bool,
}

/// Reads the reply of `PeerManageRpcService/ListPublicIpv6Info`.
pub(crate) fn parse_public_ipv6(body: &[u8]) -> Result<PublicIpv6Info, RemoteError> {
    let wire: PublicIpv6Wire = decode(body)?;
    Ok(PublicIpv6Info {
        provider_prefix: inet6(wire.provider_prefix.as_ref()),
        leases: wire
            .provider_leases
            .into_iter()
            .map(|lease| PublicIpv6Lease {
                peer_id: lease.peer_id,
                instance_id: instance_id(&lease.inst_id),
                addr: inet6(lease.leased_addr.as_ref()),
                valid_until_ms: seconds_to_ms(lease.valid_until_unix_seconds),
                reused: lease.reused,
            })
            .collect(),
    })
}

// --- Peer center --------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct GlobalPeerMapWire {
    global_peer_map: BTreeMap<String, PeerMapWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct PeerMapWire {
    direct_peers: BTreeMap<String, DirectPeerWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct DirectPeerWire {
    latency_ms: i64,
}

/// Reads the reply of `PeerCenterManageRpcService/GetGlobalPeerMap`, ordered by peer id. A key that is not a peer
/// id is skipped.
pub(crate) fn parse_global_peer_map(body: &[u8]) -> Result<Vec<PeerMapNode>, RemoteError> {
    let wire: GlobalPeerMapWire = decode(body)?;
    let mut nodes: Vec<PeerMapNode> = wire
        .global_peer_map
        .into_iter()
        .filter_map(|(key, node)| {
            let mut direct_peers: Vec<PeerMapLink> = node
                .direct_peers
                .into_iter()
                .filter_map(|(peer, link)| {
                    Some(PeerMapLink {
                        peer_id: peer.parse().ok()?,
                        latency_ms: u32::try_from(link.latency_ms).unwrap_or_default(),
                    })
                })
                .collect();
            direct_peers.sort_by_key(|link| link.peer_id);
            Some(PeerMapNode {
                peer_id: key.parse().ok()?,
                direct_peers,
            })
        })
        .collect();
    nodes.sort_by_key(|node| node.peer_id);
    Ok(nodes)
}

// --- Connectors, mapped listeners, VPN portal ---------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ListConnectorWire {
    connectors: Vec<ConnectorWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ConnectorWire {
    url: Option<UrlWire>,
    status: i64,
}

/// Reads the reply of `ConnectorManageRpcService/ListConnector`. A connector without a url is skipped.
pub(crate) fn parse_connectors(body: &[u8]) -> Result<Vec<Connector>, RemoteError> {
    let wire: ListConnectorWire = decode(body)?;
    Ok(wire
        .connectors
        .into_iter()
        .filter_map(|connector| {
            let url = connector.url?.url;
            (!url.is_empty()).then(|| Connector {
                url,
                status: ConnectorStatus::from_code(connector.status),
            })
        })
        .collect())
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ListMappedListenerWire {
    mappedlisteners: Vec<MappedListenerWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MappedListenerWire {
    url: Option<UrlWire>,
}

/// Reads the reply of `MappedListenerManageRpcService/ListMappedListener`: the urls.
pub(crate) fn parse_mapped_listeners(body: &[u8]) -> Result<Vec<String>, RemoteError> {
    let wire: ListMappedListenerWire = decode(body)?;
    Ok(wire
        .mappedlisteners
        .into_iter()
        .filter_map(|listener| listener.url)
        .map(|url| url.url)
        .filter(|url| !url.is_empty())
        .collect())
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct VpnPortalReplyWire {
    vpn_portal_info: Option<VpnPortalWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct VpnPortalWire {
    vpn_type: String,
    client_config: Secret,
    connected_clients: Vec<String>,
}

/// Reads the reply of `VpnPortalRpcService/GetVpnPortalInfo`. The client config holds a private key and stays a
/// [`Secret`].
pub(crate) fn parse_vpn_portal(body: &[u8]) -> Result<VpnPortalInfo, RemoteError> {
    let wire: VpnPortalReplyWire = decode(body)?;
    let portal = wire.vpn_portal_info.unwrap_or_default();
    Ok(VpnPortalInfo {
        vpn_type: portal.vpn_type,
        client_config: portal.client_config,
        connected_clients: portal.connected_clients,
    })
}

// --- ACL ----------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct AclStatsReplyWire {
    acl_stats: Option<AclStatsWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct AclStatsWire {
    rules: Vec<RuleStatsWire>,
    conn_track: Vec<ConnTrackWire>,
    global: BTreeMap<String, u64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RuleStatsWire {
    rule: Option<RuleWire>,
    stat: Option<StatItemWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RuleWire {
    name: String,
    description: String,
    priority: u32,
    enabled: bool,
    protocol: i64,
    ports: Vec<String>,
    source_ips: Vec<String>,
    destination_ips: Vec<String>,
    source_ports: Vec<String>,
    action: i64,
    rate_limit: u32,
    stateful: bool,
    source_groups: Vec<String>,
    destination_groups: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct StatItemWire {
    packet_count: u64,
    byte_count: u64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ConnTrackWire {
    src_addr: Option<SocketWire>,
    dst_addr: Option<SocketWire>,
    protocol: i64,
    state: i64,
    created_at: u64,
    last_seen: u64,
    packet_count: u64,
    byte_count: u64,
}

fn unix_seconds_to_ms(seconds: u64) -> Option<i64> {
    seconds_to_ms(i64::try_from(seconds).ok()?)
}

/// Reads the reply of `AclManageRpcService/GetAclStats`. Rule statistics only: the rules' group secrets are not
/// part of this reply.
pub(crate) fn parse_acl_stats(body: &[u8]) -> Result<AclStats, RemoteError> {
    let wire: AclStatsReplyWire = decode(body)?;
    let stats = wire.acl_stats.unwrap_or_default();
    Ok(AclStats {
        rules: stats
            .rules
            .into_iter()
            .map(|entry| {
                let rule = entry.rule.unwrap_or_default();
                let stat = entry.stat.unwrap_or_default();
                AclRuleStats {
                    name: rule.name,
                    description: rule.description,
                    priority: rule.priority,
                    enabled: rule.enabled,
                    protocol: AclProtocol::from_code(rule.protocol),
                    ports: rule.ports,
                    source_ports: rule.source_ports,
                    source_ips: rule.source_ips,
                    destination_ips: rule.destination_ips,
                    source_groups: rule.source_groups,
                    destination_groups: rule.destination_groups,
                    action: AclAction::from_code(rule.action),
                    rate_limit: rule.rate_limit,
                    stateful: rule.stateful,
                    packets: stat.packet_count,
                    bytes: stat.byte_count,
                }
            })
            .collect(),
        connections: stats
            .conn_track
            .into_iter()
            .map(|entry| AclConnection {
                src: socket(entry.src_addr.as_ref()),
                dst: socket(entry.dst_addr.as_ref()),
                protocol: AclProtocol::from_code(entry.protocol),
                state: ConnTrackState::from_code(entry.state),
                created_at_ms: unix_seconds_to_ms(entry.created_at),
                last_seen_ms: unix_seconds_to_ms(entry.last_seen),
                packets: entry.packet_count,
                bytes: entry.byte_count,
            })
            .collect(),
        global: stats.global,
    })
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct WhitelistWire {
    tcp_ports: Vec<String>,
    udp_ports: Vec<String>,
}

/// Reads the reply of `AclManageRpcService/GetWhitelist`.
pub(crate) fn parse_whitelist(body: &[u8]) -> Result<Whitelist, RemoteError> {
    let wire: WhitelistWire = decode(body)?;
    Ok(Whitelist {
        tcp_ports: wire.tcp_ports,
        udp_ports: wire.udp_ports,
    })
}

// --- Port forwards ------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ListPortForwardWire {
    cfgs: Vec<PortForwardWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct PortForwardWire {
    bind_addr: Option<SocketWire>,
    dst_addr: Option<SocketWire>,
    socket_type: i64,
}

/// Reads the reply of `PortForwardManageRpcService/ListPortForward`. A row whose addresses cannot be read is
/// skipped.
pub(crate) fn parse_port_forwards(body: &[u8]) -> Result<Vec<PortForwardEntry>, RemoteError> {
    let wire: ListPortForwardWire = decode(body)?;
    Ok(wire
        .cfgs
        .into_iter()
        .filter_map(|cfg| {
            Some(PortForwardEntry {
                protocol: if cfg.socket_type == 1 {
                    TransportProtocol::Udp
                } else {
                    TransportProtocol::Tcp
                },
                bind: socket(cfg.bind_addr.as_ref())?,
                dst: socket(cfg.dst_addr.as_ref())?,
            })
        })
        .collect())
}

// --- Stats --------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct StatsWire {
    metrics: Vec<MetricWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MetricWire {
    name: String,
    value: u64,
    labels: BTreeMap<String, String>,
}

/// Reads the reply of `StatsRpcService/GetStats`.
pub(crate) fn parse_stats(body: &[u8]) -> Result<Vec<Metric>, RemoteError> {
    let wire: StatsWire = decode(body)?;
    Ok(wire
        .metrics
        .into_iter()
        .map(|metric| Metric {
            name: metric.name,
            value: metric.value,
            labels: metric.labels,
        })
        .collect())
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct PrometheusWire {
    prometheus_text: String,
}

/// Reads the reply of `StatsRpcService/GetPrometheusStats`: the exposition text.
pub(crate) fn parse_prometheus(body: &[u8]) -> Result<String, RemoteError> {
    let wire: PrometheusWire = decode(body)?;
    Ok(wire.prometheus_text)
}

// --- Logger and config --------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct LoggerWire {
    level: i64,
}

/// Reads the reply of `LoggerRpcService/GetLoggerConfig`.
pub(crate) fn parse_log_level(body: &[u8]) -> Result<LogLevel, RemoteError> {
    let wire: LoggerWire = decode(body)?;
    LogLevel::from_code(wire.level)
        .ok_or_else(|| RemoteError::Decode("unexpected shape: unknown log level".into()))
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct GetConfigWire {
    config: Option<Value>,
}

/// Reads the reply of `ConfigRpcService/GetConfig`: the instance's running config, which holds the network secret
/// ([`NetworkConfigForm`] keeps it as a [`Secret`]).
pub(crate) fn parse_running_config(body: &[u8]) -> Result<NetworkConfigForm, RemoteError> {
    let wire: GetConfigWire = decode(body)?;
    let config = wire
        .config
        .ok_or_else(|| RemoteError::Decode("unexpected shape: no config in the reply".into()))?;
    NetworkConfigForm::from_json(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/easytier-2.6.4/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn addr(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    #[test]
    fn node_info_gives_the_facts_and_never_the_config() {
        let body = fixture("web-rpc-show-node-info.json");
        let node = parse_node_facts(&body).unwrap();
        assert_eq!(node.peer_id, 2_708_317_465);
        assert_eq!(node.hostname, "example-host-m");
        assert_eq!(node.version, "2.6.4-8428a89d");
        assert_eq!(
            node.instance_id,
            Some("00000000-0000-0000-0000-0000000000cc".parse().unwrap())
        );
        assert_eq!(node.virtual_ipv4, Some("10.144.145.1/24".parse().unwrap()));
        assert_eq!(node.proxy_cidrs, vec!["10.250.0.0/24"]);
        assert!(node.listeners.contains(&"tcp://127.0.0.1:43012".to_owned()));
        assert_eq!(node.stun.udp_nat_type, NatType::PortRestricted);
        assert_eq!(node.stun.tcp_nat_type, NatType::Symmetric);
        assert_eq!(
            node.stun.public_ips,
            vec![
                "203.0.113.10".parse::<IpAddr>().unwrap(),
                "2001:db8::1".parse().unwrap(),
                "203.0.113.11".parse().unwrap()
            ]
        );
        assert_eq!(node.stun.port_range, Some((8934, 8934)));
        assert_eq!(node.stun.updated_at_ms, Some(1_793_252_029_000));
        // u32 and four-quarter forms are decoded to addresses, IPv4 first.
        assert_eq!(
            node.interface_ips,
            vec![
                "192.0.2.1".parse::<IpAddr>().unwrap(),
                "192.0.2.2".parse().unwrap(),
                "2001:db8::1".parse().unwrap(),
                "2001:db8::2".parse().unwrap(),
                "2001:db8::3".parse().unwrap(),
                "2001:db8::1".parse().unwrap(),
            ]
        );
        assert_eq!(node.public_ipv4, None);
        assert!(node.features.kcp_input && node.features.quic_input);
        assert!(!node.features.is_public_server);
        // The reply held the network secret; nothing of it is in the result.
        assert!(String::from_utf8_lossy(&body).contains("example-secret-do-not-use"));
        assert!(!format!("{node:?}").contains("example-secret"));
    }

    #[test]
    fn a_reply_without_node_info_is_a_decode_error() {
        assert!(matches!(
            parse_node_facts(b"{}"),
            Err(RemoteError::Decode(_))
        ));
    }

    #[test]
    fn routes_carry_hops_latency_and_next_hop() {
        let routes = parse_routes(&fixture("web-rpc-list-route.json")).unwrap();
        assert_eq!(routes.len(), 1);
        let route = &routes[0];
        assert_eq!(route.peer_id, 696_501_495);
        assert_eq!(route.next_hop_peer_id, 696_501_495);
        assert_eq!(route.hostname, "example-host-p");
        assert_eq!(route.virtual_ipv4, Some("10.144.145.2/24".parse().unwrap()));
        assert_eq!(route.virtual_ipv6, None);
        assert_eq!(route.cost, 1);
        assert_eq!(route.latency_ms, Some(2));
        assert_eq!(route.stun.udp_nat_type, NatType::PortRestricted);
        assert!(route.proxy_cidrs.is_empty());
        assert!(route.features.support_conn_list_sync);
        assert!(route.instance_id.is_some());
    }

    #[test]
    fn foreign_network_lists_the_relayed_network_without_its_digest() {
        let body = fixture("web-rpc-list-foreign-network.json");
        let networks = parse_foreign_networks(&body).unwrap();
        assert_eq!(networks.len(), 1);
        let network = &networks[0];
        assert_eq!(network.name, "example-other");
        assert_eq!(network.my_peer_id, 1_653_060_257);
        assert_eq!(network.peers.len(), 1);
        let peer = &network.peers[0];
        assert_eq!(peer.peer_id, 1_299_294_143);
        assert_eq!(peer.links.len(), 1);
        let link = &peer.links[0];
        assert_eq!(link.tunnel, "tcp");
        assert_eq!(link.rx_bytes, 2053);
        assert_eq!(link.tx_bytes, 2043);
        assert_eq!(link.latency_ms, Some(0.155));
        assert!(!link.is_client && !link.closed);
        assert!(!format!("{networks:?}").contains("digest"));
    }

    #[test]
    fn global_foreign_networks_and_summary() {
        let hosts =
            parse_global_foreign_networks(&fixture("web-rpc-list-global-foreign-network.json"))
                .unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].peer_id, 2_708_317_465);
        let network = &hosts[0].networks[0];
        assert_eq!(network.name, "example-other");
        assert_eq!(network.peer_ids, vec![1_299_294_143]);
        assert_eq!(network.version, 1_793_252_036);
        assert!(network.updated_at_ms.is_some());

        let summary =
            parse_foreign_network_summary(&fixture("web-rpc-foreign-network-summary.json"))
                .unwrap();
        assert_eq!(
            summary,
            vec![ForeignNetworkSummary {
                peer_id: 2_708_317_465,
                network_count: 1,
                peer_count: 1,
            }]
        );
    }

    #[test]
    fn public_ipv6_without_a_provider_is_empty() {
        let info = parse_public_ipv6(&fixture("web-rpc-list-public-ipv6.json")).unwrap();
        assert_eq!(info, PublicIpv6Info::default());
        let body = br#"{"provider_prefix":{"address":{"part1":536939960,"part2":1,"part3":0,"part4":0},"network_length":64},
            "provider_leases":[{"peer_id":7,"inst_id":"","leased_addr":{"address":{"part1":536939960,"part2":1,"part3":0,"part4":5},"network_length":128},"valid_until_unix_seconds":100,"reused":true}]}"#;
        let info = parse_public_ipv6(body).unwrap();
        assert_eq!(
            info.provider_prefix,
            Some("2001:db8:0:1::/64".parse().unwrap())
        );
        assert_eq!(info.leases[0].peer_id, 7);
        assert_eq!(
            info.leases[0].addr,
            Some("2001:db8:0:1::5/128".parse().unwrap())
        );
        assert_eq!(info.leases[0].valid_until_ms, Some(100_000));
        assert!(info.leases[0].reused);
    }

    #[test]
    fn global_peer_map_links_both_ways() {
        let nodes = parse_global_peer_map(&fixture("web-rpc-global-peer-map.json")).unwrap();
        assert_eq!(
            nodes,
            vec![
                PeerMapNode {
                    peer_id: 696_501_495,
                    direct_peers: vec![PeerMapLink {
                        peer_id: 2_708_317_465,
                        latency_ms: 2
                    }],
                },
                PeerMapNode {
                    peer_id: 2_708_317_465,
                    direct_peers: vec![PeerMapLink {
                        peer_id: 696_501_495,
                        latency_ms: 2
                    }],
                },
            ]
        );
    }

    #[test]
    fn connectors_and_mapped_listeners() {
        assert!(
            parse_connectors(&fixture("web-rpc-list-connector.json"))
                .unwrap()
                .is_empty()
        );
        let connectors = parse_connectors(&fixture("web-rpc-list-connector-patched.json")).unwrap();
        assert_eq!(
            connectors,
            vec![Connector {
                url: "tcp://127.0.0.1:43015".into(),
                status: ConnectorStatus::Connected,
            }]
        );
        // An unknown status code is a state of its own, not an error that hides the list.
        let odd = parse_connectors(br#"{"connectors":[{"url":{"url":"tcp://h:1"},"status":9}]}"#)
            .unwrap();
        assert_eq!(odd[0].status, ConnectorStatus::Unknown);

        assert_eq!(
            parse_mapped_listeners(&fixture("web-rpc-list-mapped-listener.json")).unwrap(),
            vec!["tcp://127.0.0.1:43013"]
        );
        assert_eq!(
            parse_mapped_listeners(&fixture("web-rpc-list-mapped-listener-patched.json")).unwrap(),
            vec!["tcp://127.0.0.1:43013", "tcp://127.0.0.1:43016"]
        );
    }

    #[test]
    fn vpn_portal_keeps_its_private_key_secret() {
        let portal = parse_vpn_portal(&fixture("web-rpc-vpn-portal.json")).unwrap();
        assert_eq!(portal.vpn_type, "wireguard");
        assert!(portal.is_enabled());
        assert!(portal.connected_clients.is_empty());
        assert!(portal.client_config.expose().contains("[Interface]"));
        assert!(
            portal
                .client_config
                .expose()
                .contains("example-private-key-do-not-use")
        );
        assert!(!format!("{portal:?}").contains("example-private-key"));
        assert!(!parse_vpn_portal(b"{}").unwrap().is_enabled());
    }

    #[test]
    fn acl_stats_and_whitelists() {
        let stats = parse_acl_stats(&fixture("web-rpc-acl-stats.json")).unwrap();
        assert!(stats.rules.is_empty() && stats.connections.is_empty());
        assert_eq!(stats.global.get("CacheMaxSize"), Some(&1024));
        assert_eq!(stats.global.get("CacheSize"), Some(&0));

        let body = br#"{"acl_stats":{"rules":[{"rule":{"name":"ssh","priority":10,"enabled":true,"protocol":1,"ports":["22"],"action":2,"source_ips":["10.0.0.0/8"]},"stat":{"packet_count":5,"byte_count":300}}],
            "conn_track":[{"src_addr":{"ip":{"Ipv4":{"addr":167772161}},"port":5000},"dst_addr":{"ip":{"Ipv4":{"addr":167772162}},"port":22},"protocol":1,"state":1,"created_at":10,"last_seen":20,"packet_count":3,"byte_count":99}],"global":{}}}"#;
        let stats = parse_acl_stats(body).unwrap();
        let rule = &stats.rules[0];
        assert_eq!(rule.name, "ssh");
        assert_eq!(rule.protocol, AclProtocol::Tcp);
        assert_eq!(rule.action, AclAction::Drop);
        assert_eq!((rule.packets, rule.bytes), (5, 300));
        assert_eq!(rule.ports, vec!["22"]);
        let connection = &stats.connections[0];
        assert_eq!(connection.src, Some(addr("10.0.0.1:5000")));
        assert_eq!(connection.dst, Some(addr("10.0.0.2:22")));
        assert_eq!(connection.state, ConnTrackState::Established);
        assert_eq!(connection.created_at_ms, Some(10_000));

        let empty = parse_whitelist(&fixture("web-rpc-whitelist.json")).unwrap();
        assert_eq!(empty, Whitelist::default());
        let patched = parse_whitelist(&fixture("web-rpc-whitelist-patched.json")).unwrap();
        assert_eq!(patched.tcp_ports, vec!["80"]);
        assert_eq!(patched.udp_ports, vec!["53"]);
    }

    #[test]
    fn port_forwards_decode_the_socket_address_oneof() {
        let forwards = parse_port_forwards(&fixture("web-rpc-list-port-forward.json")).unwrap();
        assert_eq!(
            forwards,
            vec![PortForwardEntry {
                protocol: TransportProtocol::Tcp,
                bind: addr("127.0.0.1:43050"),
                dst: addr("10.144.145.2:22"),
            }]
        );
        let patched =
            parse_port_forwards(&fixture("web-rpc-list-port-forward-patched.json")).unwrap();
        assert_eq!(patched.len(), 2);
        assert_eq!(patched[1].protocol, TransportProtocol::Udp);
        assert_eq!(patched[1].bind, addr("127.0.0.1:43051"));
        assert_eq!(patched[1].dst, addr("10.144.145.2:53"));

        let v6 = br#"{"cfgs":[{"bind_addr":{"ip":{"Ipv6":{"part1":0,"part2":0,"part3":0,"part4":1}},"port":80},"dst_addr":{"ip":{"Ipv4":{"addr":1}},"port":70000},"socket_type":0},
            {"bind_addr":{"ip":{"Ipv6":{"part1":0,"part2":0,"part3":0,"part4":1}},"port":80},"dst_addr":{"ip":{"Ipv4":{"addr":16909060}},"port":81},"socket_type":1}]}"#;
        let parsed = parse_port_forwards(v6).unwrap();
        // The row with an impossible port is skipped; the other survives.
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].bind, addr("[::1]:80"));
        assert_eq!(parsed[0].dst, addr("1.2.3.4:81"));
    }

    #[test]
    fn stats_and_prometheus_text() {
        let metrics = parse_stats(&fixture("web-rpc-stats.json")).unwrap();
        let rx = metrics
            .iter()
            .find(|metric| metric.name == "compression_bytes_rx_after")
            .unwrap();
        assert_eq!(rx.value, 1596);
        assert_eq!(
            rx.labels.get("network_name").map(String::as_str),
            Some("example-net")
        );
        assert!(
            metrics
                .iter()
                .any(|metric| metric.name == "peer_rpc_client_rx"
                    && metric.labels.get("status").map(String::as_str) == Some("success"))
        );

        let text = parse_prometheus(&fixture("web-rpc-prometheus-stats.json")).unwrap();
        assert!(text.starts_with("# TYPE compression_bytes_rx_after counter\n"));
        assert!(text.contains("compression_bytes_rx_after{network_name=\"example-net\"} 1596"));
    }

    #[test]
    fn logger_level_is_typed() {
        assert_eq!(
            parse_log_level(&fixture("web-rpc-logger-get.json")).unwrap(),
            LogLevel::Disabled
        );
        assert_eq!(parse_log_level(br#"{"level":4}"#).unwrap(), LogLevel::Debug);
        assert!(matches!(
            parse_log_level(br#"{"level":99}"#),
            Err(RemoteError::Decode(_))
        ));
        for level in LogLevel::ALL {
            assert_eq!(LogLevel::from_code(i64::from(level.code())), Some(level));
        }
    }

    #[test]
    fn running_config_is_a_form_whose_secret_stays_secret() {
        let form = parse_running_config(&fixture("web-rpc-get-config.json")).unwrap();
        assert_eq!(form.network_name, "example-net");
        assert_eq!(form.hostname.as_deref(), Some("example-host-m"));
        assert_eq!(form.network_secret.expose(), "example-secret-do-not-use");
        assert!(!format!("{form:?}").contains("example-secret"));
        assert_eq!(
            form.instance_id,
            "00000000-0000-0000-0000-0000000000cc"
                .parse::<Uuid>()
                .unwrap()
        );
        assert!(matches!(
            parse_running_config(b"{}"),
            Err(RemoteError::Decode(_))
        ));
    }

    #[test]
    fn replies_with_odd_shapes_fail_without_quoting_values() {
        let error = parse_routes(br#"{"routes":"example-secret-do-not-use"}"#).unwrap_err();
        let RemoteError::Decode(text) = &error else {
            panic!("a decode error");
        };
        assert!(!text.contains("example-secret"), "{text}");
    }
}
