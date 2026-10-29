//! easytier-web's JSON replies, read tolerantly and mapped to the app's types: the one adapter that names the
//! server's fields.
//!
//! Every struct is `#[serde(default)]` without `deny_unknown_fields`: a newer server adds fields, an older one
//! lacks some, and a list must never vanish because one row is odd.

mod rpc;

use std::{collections::BTreeMap, net::Ipv4Addr};

use ipnet::Ipv4Net;
pub(crate) use rpc::*;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{
    api::decode,
    error::RemoteError,
    ids::UuidQuad,
    model::{
        ConfigSource, Credential, Machine, MachineNetworks, NetworkMeta, NetworkRuntime,
        NewCredential, PeerLink, PeerLinks, ServerSession, ServerSummary,
    },
    timestamp::parse_rfc3339_ms,
};
use crate::core::{
    secret::Secret,
    state::{NatType, NodeInfo, NodeSnapshot, PeerSummary, RouteKind},
};

// --- Machines -----------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SummaryWire {
    device_count: u32,
}

/// Reads `GET /summary`.
pub(crate) fn parse_summary(body: &[u8]) -> Result<ServerSummary, RemoteError> {
    let wire: SummaryWire = decode(body)?;
    Ok(ServerSummary {
        device_count: wire.device_count,
    })
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MachinesWire {
    machines: Vec<MachineWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MachineWire {
    client_url: Option<String>,
    info: Option<HeartbeatWire>,
    location: Option<LocationWire>,
}

/// The heartbeat. `user_token` (the enrollment account name) is deliberately not read: it is a credential.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct HeartbeatWire {
    machine_id: Option<UuidQuad>,
    easytier_version: String,
    report_time: Option<String>,
    hostname: String,
    running_network_instances: Vec<UuidQuad>,
    device_os: Option<DeviceOsWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct DeviceOsWire {
    os_type: String,
    version: String,
    distribution: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct LocationWire {
    country: Option<String>,
    city: Option<String>,
    region: Option<String>,
}

/// Reads `GET /machines`. A machine that has not sent its first heartbeat has no id the console could address,
/// so it is left out until it has.
pub(crate) fn parse_machines(body: &[u8]) -> Result<Vec<Machine>, RemoteError> {
    let wire: MachinesWire = decode(body)?;
    Ok(wire
        .machines
        .into_iter()
        .filter_map(|item| {
            let info = item.info?;
            let id = Uuid::from(info.machine_id?);
            let os = info.device_os.map(os_label).unwrap_or_default();
            Some(Machine {
                id,
                hostname: info.hostname,
                os,
                version: info.easytier_version,
                report_time_ms: info.report_time.as_deref().and_then(parse_rfc3339_ms),
                running_networks: info
                    .running_network_instances
                    .into_iter()
                    .map(Uuid::from)
                    .collect(),
                location: item.location.and_then(location_label),
                client_url: item.client_url.unwrap_or_default(),
            })
        })
        .collect())
}

/// `macOS 27.2`: the distribution (or OS type) and the version.
fn os_label(os: DeviceOsWire) -> String {
    let name = if os.distribution.trim().is_empty() {
        os.os_type
    } else {
        os.distribution
    };
    [name.trim(), os.version.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `Germany, Berlin`: the non-empty parts; `None` for no place at all.
fn location_label(location: LocationWire) -> Option<String> {
    let parts: Vec<String> = [location.country, location.region, location.city]
        .into_iter()
        .flatten()
        .map(|part| part.trim().to_owned())
        .filter(|part| !part.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SessionWire {
    machine_id: String,
    client_url: String,
    user_id: i64,
    /// The enrollment account name; a credential, so a `Secret`.
    token: Secret,
}

/// Reads `GET /sessions`; a row whose machine id is not a UUID is skipped.
pub(crate) fn parse_sessions(body: &[u8]) -> Result<Vec<ServerSession>, RemoteError> {
    let wire: Vec<SessionWire> = decode(body)?;
    Ok(wire
        .into_iter()
        .filter_map(|row| {
            Some(ServerSession {
                machine_id: row.machine_id.parse().ok()?,
                client_url: row.client_url,
                user_id: row.user_id,
                account: row.token,
            })
        })
        .collect())
}

// --- Networks of a machine ----------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MachineNetworksWire {
    running_inst_ids: Vec<UuidQuad>,
    disabled_inst_ids: Vec<UuidQuad>,
}

/// Reads `GET /machines/{mid}/networks`.
pub(crate) fn parse_machine_networks(body: &[u8]) -> Result<MachineNetworks, RemoteError> {
    let wire: MachineNetworksWire = decode(body)?;
    Ok(MachineNetworks {
        running: wire.running_inst_ids.into_iter().map(Uuid::from).collect(),
        disabled: wire.disabled_inst_ids.into_iter().map(Uuid::from).collect(),
    })
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MetasWire {
    metas: BTreeMap<String, MetaWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MetaWire {
    inst_id: Option<UuidQuad>,
    network_name: String,
    config_permission: u32,
    instance_name: String,
    source: i64,
}

/// Reads `POST /machines/{mid}/networks/metas`. The map key is the canonical id; the `inst_id` quarters are the
/// fallback.
pub(crate) fn parse_metas(body: &[u8]) -> Result<Vec<NetworkMeta>, RemoteError> {
    let wire: MetasWire = decode(body)?;
    Ok(wire
        .metas
        .into_iter()
        .filter_map(|(key, meta)| {
            let instance_id = key
                .parse::<Uuid>()
                .ok()
                .or_else(|| meta.inst_id.map(Uuid::from))?;
            Some(NetworkMeta {
                instance_id,
                network_name: meta.network_name,
                instance_name: meta.instance_name,
                source: ConfigSource::from_code(meta.source),
                config_permission: meta.config_permission,
            })
        })
        .collect())
}

// --- Running info -------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct InfoWire {
    info: Option<InfoMapWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct InfoMapWire {
    map: BTreeMap<String, RunningInfoWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RunningInfoWire {
    my_node_info: Option<MyNodeWire>,
    routes: Vec<RouteWire>,
    peers: Vec<PeerInfoWire>,
    running: bool,
    error_msg: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MyNodeWire {
    virtual_ipv4: Option<InetWire>,
    hostname: String,
    version: String,
    stun_info: Option<StunWire>,
    listeners: Vec<UrlWire>,
    peer_id: u32,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct InetWire {
    address: Option<AddrWire>,
    network_length: u32,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct AddrWire {
    /// Host-order integer: `177246465` is 10.144.145.1.
    addr: u32,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct UrlWire {
    url: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct StunWire {
    udp_nat_type: i64,
    public_ip: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RouteWire {
    peer_id: u32,
    ipv4_addr: Option<InetWire>,
    cost: i64,
    path_latency: i64,
    hostname: String,
    stun_info: Option<StunWire>,
    version: String,
    path_latency_latency_first: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct PeerInfoWire {
    peer_id: u32,
    conns: Vec<ConnWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ConnWire {
    tunnel: Option<TunnelWire>,
    stats: Option<ConnStatsWire>,
    loss_rate: f64,
    is_client: bool,
    is_closed: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct TunnelWire {
    tunnel_type: String,
    remote_addr: Option<UrlWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ConnStatsWire {
    rx_bytes: u64,
    tx_bytes: u64,
    latency_us: u64,
}

/// An address with its prefix; `None` for the unspecified address (not assigned yet).
fn inet(wire: Option<&InetWire>) -> Option<Ipv4Net> {
    let wire = wire?;
    let addr = Ipv4Addr::from(wire.address.as_ref()?.addr);
    if addr.is_unspecified() {
        return None;
    }
    let prefix = u8::try_from(wire.network_length).ok()?;
    Ipv4Net::new(addr, prefix).ok()
}

/// Reads one instance's running info into the app's snapshot.
fn runtime_from_wire(wire: RunningInfoWire, read_at_ms: f64) -> NetworkRuntime {
    let node = wire.my_node_info.unwrap_or_default();
    let node_info = NodeInfo {
        peer_id: node.peer_id,
        hostname: node.hostname,
        virtual_ipv4: inet(node.virtual_ipv4.as_ref()),
        // `ring://` is the in-process loopback listener, not a socket a peer can dial.
        listeners: node
            .listeners
            .into_iter()
            .map(|listener| listener.url)
            .filter(|url| !url.starts_with("ring://"))
            .collect(),
        public_ips: node
            .stun_info
            .as_ref()
            .map(|stun| stun.public_ip.clone())
            .unwrap_or_default(),
        nat_type: node.stun_info.as_ref().map_or(NatType::Unknown, |stun| {
            NatType::from_code(stun.udp_nat_type)
        }),
        proxy_cidrs: Vec::new(),
        version: node.version,
    };
    let peers = wire
        .routes
        .into_iter()
        .map(|route| {
            let conns = wire
                .peers
                .iter()
                .find(|peer| peer.peer_id == route.peer_id)
                .map(|peer| peer.conns.as_slice())
                .unwrap_or_default();
            peer_summary(route, conns)
        })
        .collect();
    NetworkRuntime {
        running: wire.running,
        error: wire.error_msg.filter(|message| !message.trim().is_empty()),
        snapshot: NodeSnapshot {
            read_at_ms,
            node: node_info,
            peers,
        },
    }
}

/// One route plus the links to that peer. A route of cost 1 is a direct link (what the CLI calls `p2p`); a higher
/// cost is the number of hops through other nodes.
fn peer_summary(route: RouteWire, conns: &[ConnWire]) -> PeerSummary {
    let live: Vec<&ConnWire> = conns.iter().filter(|conn| !conn.is_closed).collect();
    let kind = match route.cost {
        1 => RouteKind::Direct,
        cost if cost >= 2 => {
            u32::try_from(cost).map_or(RouteKind::Unknown, |hops| RouteKind::Relayed { hops })
        }
        _ => RouteKind::Unknown,
    };
    let link_latency = live
        .iter()
        .filter_map(|conn| conn.stats.as_ref())
        .map(|stats| stats.latency_us)
        .filter(|latency| *latency > 0)
        .min()
        .map(|micros| micros as f64 / 1000.0);
    let latency_ms = match kind {
        RouteKind::Direct => link_latency,
        RouteKind::Relayed { .. } | RouteKind::Unknown => {
            let path = route
                .path_latency_latency_first
                .unwrap_or(route.path_latency);
            (path > 0).then_some(path as f64)
        }
    };
    let loss = (!live.is_empty()).then(|| {
        let total: f64 = live.iter().map(|conn| conn.loss_rate).sum();
        (total / live.len() as f64).clamp(0.0, 1.0)
    });
    let counters = live.iter().filter_map(|conn| conn.stats.as_ref());
    let (rx_bytes, tx_bytes) = counters.fold(
        (None, None),
        |(rx, tx): (Option<u64>, Option<u64>), stats| {
            (
                Some(rx.unwrap_or(0).saturating_add(stats.rx_bytes)),
                Some(tx.unwrap_or(0).saturating_add(stats.tx_bytes)),
            )
        },
    );
    let mut tunnels: Vec<&str> = Vec::new();
    for conn in &live {
        if let Some(tunnel) = conn.tunnel.as_ref().map(|t| t.tunnel_type.as_str())
            && !tunnel.is_empty()
            && !tunnels.contains(&tunnel)
        {
            tunnels.push(tunnel);
        }
    }
    PeerSummary {
        peer_id: route.peer_id,
        hostname: route.hostname,
        virtual_ipv4: inet(route.ipv4_addr.as_ref()),
        route: kind,
        latency_ms,
        loss,
        rx_bytes,
        tx_bytes,
        tunnel: (!tunnels.is_empty()).then(|| tunnels.join(",")),
        nat_type: route.stun_info.as_ref().map_or(NatType::Unknown, |stun| {
            NatType::from_code(stun.udp_nat_type)
        }),
        version: (!route.version.trim().is_empty()).then_some(route.version),
    }
}

/// Reads `GET /machines/{mid}/networks/info/{inst}` for `instance`: `None` when the machine reports no such
/// instance. `read_at_ms` stamps the snapshot (`nsh_core::time::unix_ms()` at the time the reply arrived).
pub(crate) fn parse_network_info(
    body: &[u8],
    instance: Uuid,
    read_at_ms: f64,
) -> Result<Option<NetworkRuntime>, RemoteError> {
    let wire: InfoWire = decode(body)?;
    let Some(info) = wire.info else {
        return Ok(None);
    };
    let wanted = instance.to_string();
    Ok(info
        .map
        .into_iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(&wanted))
        .map(|(_, running)| runtime_from_wire(running, read_at_ms)))
}

// --- Peers and credentials over proxy-rpc -------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ListPeerWire {
    peer_infos: Vec<PeerInfoWire>,
}

/// One peer with its links.
fn peer_links(peer: PeerInfoWire) -> PeerLinks {
    PeerLinks {
        peer_id: peer.peer_id,
        links: peer.conns.into_iter().map(peer_link).collect(),
    }
}

/// Reads the reply of `PeerManageRpcService/ListPeer`.
pub(crate) fn parse_list_peers(body: &[u8]) -> Result<Vec<PeerLinks>, RemoteError> {
    let wire: ListPeerWire = decode(body)?;
    Ok(wire.peer_infos.into_iter().map(peer_links).collect())
}

fn peer_link(conn: ConnWire) -> PeerLink {
    let (rx_bytes, tx_bytes, latency_us) = conn.stats.map_or((0, 0, 0), |stats| {
        (stats.rx_bytes, stats.tx_bytes, stats.latency_us)
    });
    let (tunnel, remote) = conn.tunnel.map_or((String::new(), None), |tunnel| {
        (
            tunnel.tunnel_type,
            tunnel
                .remote_addr
                .map(|url| url.url)
                .filter(|url| !url.is_empty()),
        )
    });
    PeerLink {
        tunnel,
        remote,
        latency_ms: (latency_us > 0).then(|| latency_us as f64 / 1000.0),
        loss: conn.loss_rate.clamp(0.0, 1.0),
        rx_bytes,
        tx_bytes,
        is_client: conn.is_client,
        closed: conn.is_closed,
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ListCredentialsWire {
    credentials: Vec<CredentialWire>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct CredentialWire {
    credential_id: String,
    groups: Vec<String>,
    allow_relay: bool,
    /// Unix seconds.
    expiry_unix: i64,
    allowed_proxy_cidrs: Vec<String>,
    reusable: Option<bool>,
}

/// Reads the reply of `CredentialManageRpcService/ListCredentials`.
pub(crate) fn parse_credentials(body: &[u8]) -> Result<Vec<Credential>, RemoteError> {
    let wire: ListCredentialsWire = decode(body)?;
    Ok(wire
        .credentials
        .into_iter()
        .map(|credential| Credential {
            id: credential.credential_id,
            groups: credential.groups,
            allow_relay: credential.allow_relay,
            expires_at_ms: (credential.expiry_unix > 0)
                .then(|| credential.expiry_unix.checked_mul(1000))
                .flatten(),
            allowed_proxy_cidrs: credential.allowed_proxy_cidrs,
            reusable: credential.reusable,
        })
        .collect())
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct GeneratedCredentialWire {
    credential_id: String,
    credential_secret: Secret,
}

/// Reads the reply of `CredentialManageRpcService/GenerateCredential`.
pub(crate) fn parse_new_credential(body: &[u8]) -> Result<NewCredential, RemoteError> {
    let wire: GeneratedCredentialWire = decode(body)?;
    if wire.credential_id.is_empty() || wire.credential_secret.is_empty() {
        return Err(RemoteError::Decode(
            "unexpected shape: no credential in the reply".into(),
        ));
    }
    Ok(NewCredential {
        id: wire.credential_id,
        secret: wire.credential_secret,
    })
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RevokedWire {
    success: bool,
}

/// Reads the reply of `CredentialManageRpcService/RevokeCredential`: whether a credential was revoked.
pub(crate) fn parse_revoked(body: &[u8]) -> Result<bool, RemoteError> {
    let wire: RevokedWire = decode(body)?;
    Ok(wire.success)
}

// --- Config conversion --------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct TomlWire {
    toml_config: Option<Secret>,
    error: Option<String>,
}

/// Reads `validate-config` / `generate-config`: the TOML, which holds the network secret in clear.
///
/// A body with `error` (or without a TOML) is the server refusing the config.
pub(crate) fn parse_toml_reply(body: &[u8]) -> Result<Secret, RemoteError> {
    let wire: TomlWire = decode(body)?;
    if let Some(message) = wire.error.filter(|message| !message.trim().is_empty()) {
        return Err(RemoteError::Rejected {
            status: 200,
            message: message.chars().take(300).collect(),
        });
    }
    wire.toml_config
        .ok_or_else(|| RemoteError::Decode("unexpected shape: no TOML config in the reply".into()))
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ParseConfigWire {
    config: Option<Value>,
    error: Option<String>,
}

/// Reads `parse-config`: the `NetworkConfig` JSON of the TOML. The error is a Debug string of the TOML failure
/// (it can quote the offending line), shown as the server's message.
pub(crate) fn parse_config_reply(body: &[u8]) -> Result<Value, RemoteError> {
    let wire: ParseConfigWire = decode(body)?;
    if let Some(message) = wire.error.filter(|message| !message.trim().is_empty()) {
        return Err(RemoteError::Rejected {
            status: 200,
            message: message.chars().take(300).collect(),
        });
    }
    wire.config
        .ok_or_else(|| RemoteError::Decode("unexpected shape: no config in the reply".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::state::RouteKind;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/easytier-2.6.4/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn id(text: &str) -> Uuid {
        text.parse().unwrap()
    }

    const MACHINE: &str = "00000000-0000-0000-0000-000000000001";
    const INSTANCE: &str = "00000000-0000-0000-0000-0000000000aa";

    #[test]
    fn summary_and_sessions() {
        assert_eq!(
            parse_summary(&fixture("web-summary.json"))
                .unwrap()
                .device_count,
            1
        );
        let sessions = parse_sessions(&fixture("web-sessions.json")).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].machine_id, id(MACHINE));
        assert_eq!(sessions[0].user_id, 2);
        assert_eq!(sessions[0].account.expose(), "admin");
        assert!(!format!("{:?}", sessions[0]).contains("admin"));
        assert!(sessions[0].client_url.starts_with("udp://127.0.0.1:"));
    }

    #[test]
    fn machines_fixture() {
        let machines = parse_machines(&fixture("web-machines.json")).unwrap();
        assert_eq!(machines.len(), 1);
        let machine = &machines[0];
        assert_eq!(machine.id, id(MACHINE));
        assert_eq!(machine.hostname, "example-host-m");
        assert_eq!(machine.version, "2.6.4-8428a89d");
        assert_eq!(machine.os, "macOS 27.2");
        assert_eq!(machine.location.as_deref(), Some("本地网络"));
        assert!(machine.running_networks.is_empty());
        let reported = machine.report_time_ms.unwrap();
        assert_eq!(reported, 1_793_229_820_084.0);
        assert!(machine.is_online(reported + 5_000.0));
        assert!(!machine.is_online(reported + 3_600_000.0));
    }

    #[test]
    fn machines_without_a_first_heartbeat_are_left_out() {
        let body =
            br#"{"machines":[{"client_url":"udp://127.0.0.1:1","info":null,"location":null}]}"#;
        assert!(parse_machines(body).unwrap().is_empty());
        assert!(parse_machines(b"{}").unwrap().is_empty());
    }

    #[test]
    fn machine_networks_fixtures() {
        let running = parse_machine_networks(&fixture("web-list-networks.json")).unwrap();
        assert_eq!(running.running, vec![id(INSTANCE)]);
        assert!(running.disabled.is_empty());
        let disabled = parse_machine_networks(&fixture("web-list-after-disable.json")).unwrap();
        assert!(disabled.running.is_empty());
        assert_eq!(disabled.disabled, vec![id(INSTANCE)]);
        let none = parse_machine_networks(&fixture("web-list-after-delete.json")).unwrap();
        assert_eq!(none, MachineNetworks::default());
    }

    #[test]
    fn metas_fixture() {
        let metas = parse_metas(&fixture("web-metas.json")).unwrap();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].instance_id, id(INSTANCE));
        assert_eq!(metas[0].network_name, "example-net");
        assert_eq!(metas[0].source, ConfigSource::User);
        assert_eq!(metas[0].config_permission, 0);
    }

    #[test]
    fn network_info_without_peers() {
        let runtime = parse_network_info(&fixture("web-info-one.json"), id(INSTANCE), 42.0)
            .unwrap()
            .unwrap();
        assert!(runtime.running);
        assert_eq!(runtime.error, None);
        assert_eq!(runtime.snapshot.read_at_ms, 42.0);
        let node = &runtime.snapshot.node;
        assert_eq!(node.hostname, "example-host-m");
        assert_eq!(node.virtual_ipv4, "10.144.145.1/24".parse().ok());
        assert_eq!(node.nat_type, NatType::PortRestricted);
        assert_eq!(node.version, "2.6.4-8428a89d");
        assert_eq!(node.listeners, vec!["tcp://127.0.0.1:41012".to_owned()]);
        assert_eq!(
            node.public_ips,
            vec!["203.0.113.11".to_owned(), "2001:db8::1".to_owned()]
        );
        assert!(runtime.snapshot.peers.is_empty());
    }

    #[test]
    fn network_info_all_form_is_read_the_same_way() {
        let runtime = parse_network_info(&fixture("web-info-all.json"), id(INSTANCE), 0.0)
            .unwrap()
            .unwrap();
        assert!(runtime.running);
        // Another instance is not in the map.
        assert_eq!(
            parse_network_info(&fixture("web-info-all.json"), id(MACHINE), 0.0).unwrap(),
            None
        );
        assert_eq!(parse_network_info(b"{}", id(INSTANCE), 0.0).unwrap(), None);
    }

    #[test]
    fn network_info_with_a_direct_peer() {
        let runtime = parse_network_info(
            &fixture("web-info-peers.json"),
            id("00000000-0000-0000-0000-0000000000cc"),
            1.0,
        )
        .unwrap()
        .unwrap();
        let peers = &runtime.snapshot.peers;
        assert_eq!(peers.len(), 1);
        let peer = &peers[0];
        assert_eq!(peer.peer_id, 365_717_954);
        assert_eq!(peer.hostname, "example-host-peer");
        assert_eq!(peer.route, RouteKind::Direct);
        assert_eq!(peer.virtual_ipv4, "10.144.150.2/24".parse().ok());
        // 143 microseconds on the one link.
        assert_eq!(peer.latency_ms, Some(0.143));
        assert_eq!(peer.rx_bytes, Some(1574));
        assert_eq!(peer.tx_bytes, Some(1609));
        assert_eq!(peer.tunnel.as_deref(), Some("tcp"));
        assert_eq!(peer.nat_type, NatType::PortRestricted);
        assert_eq!(peer.version.as_deref(), Some("2.6.4-8428a89d"));
        assert_eq!(peer.loss, Some(0.0));
        // The `ring://` listener is not shown.
        assert!(
            runtime
                .snapshot
                .node
                .listeners
                .iter()
                .all(|listener| !listener.starts_with("ring://"))
        );
    }

    #[test]
    fn a_route_without_links_is_relayed_with_the_path_latency() {
        let body = br#"{"info":{"map":{"00000000-0000-0000-0000-0000000000aa":{
            "running":true,
            "routes":[{"peer_id":7,"cost":3,"path_latency":40,"hostname":"far","ipv4_addr":{"address":{"addr":167772161},"network_length":24}}],
            "peers":[]}}}}"#;
        let runtime = parse_network_info(body, id(INSTANCE), 0.0)
            .unwrap()
            .unwrap();
        let peer = &runtime.snapshot.peers[0];
        assert_eq!(peer.route, RouteKind::Relayed { hops: 3 });
        assert_eq!(peer.latency_ms, Some(40.0));
        assert_eq!(peer.rx_bytes, None);
        assert_eq!(peer.tunnel, None);
        assert_eq!(peer.virtual_ipv4, "10.0.0.1/24".parse().ok());
    }

    #[test]
    fn list_peer_fixtures() {
        // No peers yet: the reply of the original capture.
        assert!(
            parse_list_peers(&fixture("web-proxy-rpc.json"))
                .unwrap()
                .is_empty()
        );
        let peers = parse_list_peers(&fixture("web-list-peer.json")).unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].peer_id, 365_717_954);
        let link = &peers[0].links[0];
        assert_eq!(link.tunnel, "tcp");
        assert_eq!(link.remote.as_deref(), Some("tcp://192.0.2.5:60388"));
        assert_eq!(link.latency_ms, Some(0.143));
        assert_eq!((link.rx_bytes, link.tx_bytes), (1574, 1609));
        assert!(!link.is_client);
        assert!(!link.closed);
    }

    #[test]
    fn credential_fixtures() {
        assert!(
            parse_credentials(&fixture("cli-credential-list.json"))
                .unwrap()
                .is_empty()
        );
        let list = parse_credentials(&fixture("web-credential-list.json")).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].groups, vec!["g1".to_owned()]);
        assert!(list[0].allow_relay);
        assert_eq!(
            list[0].allowed_proxy_cidrs,
            vec!["192.168.9.0/24".to_owned()]
        );
        assert_eq!(list[0].reusable, Some(false));
        assert_eq!(list[0].expires_at_ms, Some(1_793_238_303_000));
        let created = parse_new_credential(&fixture("web-credential-generate.json")).unwrap();
        assert_eq!(created.id, "0c2dcf86-37c3-4db6-b1ef-7167739c2d3b");
        assert_eq!(created.secret, Secret::new("example-secret-do-not-use"));
        assert!(!format!("{created:?}").contains("example-secret"));
        assert!(parse_new_credential(b"{}").is_err());
        assert!(!parse_revoked(br#"{"success":false}"#).unwrap());
        assert!(parse_revoked(br#"{"success":true}"#).unwrap());
    }

    #[test]
    fn toml_replies() {
        let toml = parse_toml_reply(&fixture("web-validate.json")).unwrap();
        assert!(toml.expose().contains("instance_name = \"example-net\""));
        assert!(!format!("{toml:?}").contains("example-net"));
        assert_eq!(
            parse_toml_reply(br#"{"error":"bad listener","toml_config":null}"#),
            Err(RemoteError::Rejected {
                status: 200,
                message: "bad listener".into()
            })
        );
        assert!(matches!(
            parse_toml_reply(b"{}"),
            Err(RemoteError::Decode(_))
        ));
    }

    #[test]
    fn parse_config_replies() {
        let config = parse_config_reply(&fixture("web-parse-config.json")).unwrap();
        assert_eq!(config["network_name"], "example-net");
        let Err(RemoteError::Rejected {
            status: 200,
            message,
        }) = parse_config_reply(&fixture("web-parse-bad.json"))
        else {
            panic!("a TOML error is a rejection");
        };
        assert!(message.starts_with("failed to parse config file"));
    }

    #[test]
    fn unreadable_replies_are_decode_errors_not_panics() {
        assert!(matches!(
            parse_machines(b"not json"),
            Err(RemoteError::Decode(_))
        ));
        assert!(matches!(
            parse_machines(br#""text""#),
            Err(RemoteError::Decode(_))
        ));
        assert!(matches!(parse_summary(b""), Err(RemoteError::Decode(_))));
    }
}
