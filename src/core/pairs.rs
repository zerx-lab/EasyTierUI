//! The adapter from EasyTier's `PeerRoutePair` JSON to the app's [`PeerSummary`]: one pair per node of the network,
//! its route (how it is reached) and its peer (the direct connections, with their exact byte counters and latency in
//! microseconds). The core prints it in three places, all mapped here: `easytier-cli -v peer`, the
//! `peer_route_pairs` of a running-info document (the mobile library, easytier-web's `networks/info`).
//!
//! Pure functions over text: no EasyTier crate, so it compiles and is tested on every platform. Parsing is tolerant
//! (`#[serde(default)]`, unknown fields ignored); a renamed upstream field is fixed here only.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;
use serde::Deserialize;
use uuid::Uuid;

use crate::core::state::{NatType, PeerSummary, RouteKind};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct InetDoc {
    address: Option<Ipv4Doc>,
    network_length: u32,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Ipv4Doc {
    /// Host-order integer (`177246465` is `10.144.145.1`).
    addr: u32,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct StunDoc {
    udp_nat_type: i64,
    pub(crate) public_ip: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PairDoc {
    route: Option<RouteDoc>,
    peer: Option<PeerDoc>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RouteDoc {
    peer_id: u32,
    ipv4_addr: Option<InetDoc>,
    cost: i32,
    path_latency: i32,
    path_latency_latency_first: Option<i32>,
    hostname: String,
    stun_info: Option<StunDoc>,
    version: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PeerDoc {
    conns: Vec<ConnDoc>,
    default_conn_id: Option<UuidDoc>,
}

/// A protobuf UUID: four big-endian `u32` quarters.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UuidDoc {
    part1: u32,
    part2: u32,
    part3: u32,
    part4: u32,
}

impl UuidDoc {
    fn to_uuid(&self) -> Uuid {
        let wide = |hi: u32, lo: u32| (u64::from(hi) << 32) | u64::from(lo);
        let value = (u128::from(wide(self.part1, self.part2)) << 64)
            | u128::from(wide(self.part3, self.part4));
        Uuid::from_u128(value)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ConnDoc {
    conn_id: String,
    tunnel: Option<TunnelDoc>,
    stats: Option<StatsDoc>,
    loss_rate: f32,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TunnelDoc {
    tunnel_type: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StatsDoc {
    rx_bytes: u64,
    tx_bytes: u64,
    latency_us: u64,
}

/// An address with its prefix; `None` for the all-zero address (no IP assigned) or an impossible prefix.
pub(crate) fn inet(doc: &InetDoc) -> Option<Ipv4Net> {
    let addr = doc.address.as_ref()?.addr;
    if addr == 0 {
        return None;
    }
    let prefix = u8::try_from(doc.network_length).ok()?;
    Ipv4Net::new(Ipv4Addr::from(addr), prefix).ok()
}

pub(crate) fn nat(stun: Option<&StunDoc>) -> NatType {
    stun.map_or(NatType::Unknown, |stun| {
        NatType::from_code(stun.udp_nat_type)
    })
}

/// The connection the core prefers to a peer, else the first.
fn default_conn(peer: &PeerDoc) -> Option<&ConnDoc> {
    let default = peer.default_conn_id.as_ref().map(UuidDoc::to_uuid);
    peer.conns
        .iter()
        .find(|conn| default.is_some_and(|id| Uuid::parse_str(&conn.conn_id) == Ok(id)))
        .or_else(|| peer.conns.first())
}

/// Round-trip time of a direct link in milliseconds (what `easytier-cli peer` shows): the preferred connection's,
/// else the lowest.
fn direct_latency_ms(peer: &PeerDoc) -> Option<f64> {
    let micros = |conn: &ConnDoc| conn.stats.as_ref().map(|stats| stats.latency_us);
    let chosen = default_conn(peer)
        .and_then(micros)
        .or_else(|| peer.conns.iter().filter_map(micros).min())?;
    // The core reports 32-bit microseconds; clamp rather than wrap.
    Some(f64::from(u32::try_from(chosen).unwrap_or(u32::MAX)) / 1000.0)
}

fn peer_summary(pair: &PairDoc, route: &RouteDoc) -> PeerSummary {
    let peer = pair.peer.as_ref();
    let route_kind = match u32::try_from(route.cost) {
        Ok(1) => RouteKind::Direct,
        Ok(hops) if hops >= 2 => RouteKind::Relayed { hops },
        _ => RouteKind::Unknown,
    };
    let latency_ms = match route_kind {
        RouteKind::Direct => peer.and_then(direct_latency_ms),
        RouteKind::Relayed { .. } => {
            let path = route
                .path_latency_latency_first
                .unwrap_or(route.path_latency);
            (path > 0).then(|| f64::from(path))
        }
        RouteKind::Unknown => None,
    };
    let stats: Vec<&StatsDoc> = peer
        .into_iter()
        .flat_map(|peer| peer.conns.iter())
        .filter_map(|conn| conn.stats.as_ref())
        .collect();
    let sum = |field: fn(&StatsDoc) -> u64| {
        (!stats.is_empty()).then(|| {
            stats
                .iter()
                .fold(0_u64, |total, stats| total.saturating_add(field(stats)))
        })
    };
    let mut tunnels: Vec<&str> = Vec::new();
    for tunnel in peer
        .into_iter()
        .flat_map(|peer| peer.conns.iter())
        .filter_map(|conn| conn.tunnel.as_ref())
        .map(|tunnel| tunnel.tunnel_type.as_str())
        .filter(|tunnel| !tunnel.is_empty())
    {
        if !tunnels.contains(&tunnel) {
            tunnels.push(tunnel);
        }
    }
    PeerSummary {
        peer_id: route.peer_id,
        hostname: route.hostname.clone(),
        virtual_ipv4: route.ipv4_addr.as_ref().and_then(inet),
        route: route_kind,
        latency_ms,
        loss: peer
            .and_then(default_conn)
            .map(|conn| f64::from(conn.loss_rate)),
        rx_bytes: sum(|stats| stats.rx_bytes),
        tx_bytes: sum(|stats| stats.tx_bytes),
        tunnel: (!tunnels.is_empty()).then(|| tunnels.join(",")),
        nat_type: nat(route.stun_info.as_ref()),
        version: (!route.version.is_empty()).then(|| route.version.clone()),
    }
}

/// The peers among `pairs`: every pair with a route, except the node `local_peer_id` itself.
pub(crate) fn peers(pairs: &[PairDoc], local_peer_id: u32) -> Vec<PeerSummary> {
    pairs
        .iter()
        .filter_map(|pair| pair.route.as_ref().map(|route| (pair, route)))
        .filter(|(_, route)| route.peer_id != local_peer_id)
        .map(|(pair, route)| peer_summary(pair, route))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `easytier-cli -v -o json peer` of node A of two `--no-tun` nodes on loopback (2.6.4), addresses redacted.
    const CLI_PEERS: &str =
        include_str!("../../tests/fixtures/easytier-2.6.4/cli-peer-verbose.json");

    #[test]
    fn the_cli_list_maps_with_exact_counters() {
        let pairs: Vec<PairDoc> = serde_json::from_str(CLI_PEERS).unwrap();
        let [peer] = peers(&pairs, 1_236_704_359).try_into().unwrap();
        assert_eq!(peer.peer_id, 641_069_609);
        assert_eq!(peer.hostname, "example-host-b");
        assert_eq!(peer.virtual_ipv4, "10.144.144.2/24".parse().ok());
        assert_eq!(peer.route, RouteKind::Direct);
        assert_eq!(peer.latency_ms, Some(0.18));
        assert_eq!(peer.loss, Some(0.0));
        // To the byte, where `easytier-cli -o json peer` prints "1.47 kB" / "1.85 kB".
        assert_eq!(peer.rx_bytes, Some(1474));
        assert_eq!(peer.tx_bytes, Some(1846));
        assert_eq!(peer.tunnel.as_deref(), Some("tcp"));
        assert_eq!(peer.nat_type, NatType::PortRestricted);
        assert_eq!(peer.version.as_deref(), Some("2.6.4-8428a89d"));
    }

    #[test]
    fn every_connection_of_a_peer_counts() {
        let json = r#"[{"route": {"peer_id": 5, "cost": 1},
                        "peer": {"conns": [
                            {"conn_id": "a", "tunnel": {"tunnel_type": "udp"},
                             "stats": {"rx_bytes": 100, "tx_bytes": 7, "latency_us": 24000}},
                            {"conn_id": "b", "tunnel": {"tunnel_type": "tcp"},
                             "stats": {"rx_bytes": 50, "tx_bytes": 3, "latency_us": 25000}}]}}]"#;
        let pairs: Vec<PairDoc> = serde_json::from_str(json).unwrap();
        let [peer] = peers(&pairs, 1).try_into().unwrap();
        assert_eq!(peer.rx_bytes, Some(150));
        assert_eq!(peer.tx_bytes, Some(10));
        assert_eq!(peer.tunnel.as_deref(), Some("udp,tcp"));
        // No preferred connection named: the first one's.
        assert_eq!(peer.latency_ms, Some(24.0));
    }

    #[test]
    fn protobuf_uuid_quarters_match_the_canonical_text() {
        let doc = UuidDoc {
            part1: 1_210_454,
            part2: 1_709_591_257,
            part3: 2_790_571_653,
            part4: 2_326_456_765,
        };
        assert_eq!(
            doc.to_uuid().to_string(),
            "00127856-65e6-4ad9-a654-be858aaae9bd"
        );
    }
}
