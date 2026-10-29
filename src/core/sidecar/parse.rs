//! The adapter from `easytier-cli -o json` (2.6.x) to the app's [`NodeSnapshot`]: the one place that names the CLI's
//! JSON fields. Parsing is tolerant (`#[serde(default)]`, unknown fields ignored, numbers or strings accepted) so a
//! point release that adds a field, or prints a number as text, does not blank the peer list.
//!
//! `easytier-cli peer` prints *humanized* strings: latency `"0.10"`, loss `"0.0%"`, bytes `"1.59 kB"` (decimal
//! units), `"-"` for unknown, cost `Local` / `p2p` / `relay(<hops>)`. The first row is this node.

use serde::{Deserialize, Deserializer};

use crate::core::state::{NatType, NodeInfo, NodeSnapshot, PeerSummary, RouteKind, parse_ipv4_net};

/// Why the CLI's output could not be mapped.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// `easytier-cli node` printed something that is not a JSON object.
    #[error("the node report is not valid JSON")]
    Node,
    /// `easytier-cli peer` printed something that is not a JSON array.
    #[error("the peer list is not valid JSON")]
    Peers,
}

/// A JSON string or number, read as text.
#[derive(Deserialize)]
#[serde(untagged)]
enum Flex {
    Text(String),
    Unsigned(u64),
    Signed(i64),
    Float(f64),
}

fn flex<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(match Option::<Flex>::deserialize(deserializer)? {
        Some(Flex::Text(text)) => text,
        Some(Flex::Unsigned(n)) => n.to_string(),
        Some(Flex::Signed(n)) => n.to_string(),
        Some(Flex::Float(n)) => n.to_string(),
        None => String::new(),
    })
}

fn flex_int<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    Ok(flex(deserializer)?.trim().parse().unwrap_or(0))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CliNode {
    #[serde(deserialize_with = "flex")]
    peer_id: String,
    ipv4_addr: Option<String>,
    proxy_cidrs: Vec<String>,
    hostname: String,
    stun_info: Option<CliStun>,
    listeners: Vec<String>,
    version: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CliStun {
    #[serde(deserialize_with = "flex_int")]
    udp_nat_type: i64,
    public_ip: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CliPeer {
    cidr: Option<String>,
    ipv4: Option<String>,
    hostname: String,
    cost: String,
    #[serde(deserialize_with = "flex")]
    lat_ms: String,
    #[serde(deserialize_with = "flex")]
    loss_rate: String,
    #[serde(deserialize_with = "flex")]
    rx_bytes: String,
    #[serde(deserialize_with = "flex")]
    tx_bytes: String,
    tunnel_proto: String,
    nat_type: String,
    #[serde(deserialize_with = "flex")]
    id: String,
    version: String,
}

/// Listener URLs that exist only inside the process (`ring://`), not for a person to dial.
fn is_internal_listener(url: &str) -> bool {
    url.starts_with("ring://")
}

/// This node, from `easytier-cli -o json node`.
///
/// # Errors
///
/// [`ParseError::Node`] when the text is not a JSON object.
pub fn node_info(json: &str) -> Result<NodeInfo, ParseError> {
    // serde accepts a JSON array for a struct whose fields all default; the CLI's node reply is always an object.
    let value: serde_json::Value = serde_json::from_str(json).map_err(|_| ParseError::Node)?;
    if !value.is_object() {
        return Err(ParseError::Node);
    }
    let node: CliNode = serde_json::from_value(value).map_err(|_| ParseError::Node)?;
    let (nat_type, public_ips) = node.stun_info.map_or_else(
        || (NatType::Unknown, Vec::new()),
        |stun| (NatType::from_code(stun.udp_nat_type), stun.public_ip),
    );
    Ok(NodeInfo {
        peer_id: node.peer_id.trim().parse().unwrap_or_default(),
        hostname: node.hostname,
        virtual_ipv4: node.ipv4_addr.as_deref().and_then(parse_ipv4_net),
        listeners: node
            .listeners
            .into_iter()
            .filter(|url| !is_internal_listener(url))
            .collect(),
        public_ips,
        nat_type,
        proxy_cidrs: node
            .proxy_cidrs
            .iter()
            .filter_map(|cidr| parse_ipv4_net(cidr))
            .collect(),
        version: node.version,
    })
}

/// The peers of `local`, from `easytier-cli -o json peer`: the local row (`cost` is `Local`, or its id is the node's)
/// and rows without a usable id are left out.
///
/// # Errors
///
/// [`ParseError::Peers`] when the text is not a JSON array.
pub fn peers(json: &str, local: &NodeInfo) -> Result<Vec<PeerSummary>, ParseError> {
    let rows: Vec<CliPeer> = serde_json::from_str(json).map_err(|_| ParseError::Peers)?;
    Ok(rows
        .into_iter()
        .filter(|row| row.cost.trim() != "Local")
        .filter_map(peer_summary)
        .filter(|peer| peer.peer_id != local.peer_id)
        .collect())
}

fn peer_summary(row: CliPeer) -> Option<PeerSummary> {
    let peer_id = row.id.trim().parse().ok()?;
    let address = row
        .cidr
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .or(row.ipv4.as_deref())
        .and_then(parse_ipv4_net);
    Some(PeerSummary {
        peer_id,
        hostname: row.hostname,
        virtual_ipv4: address,
        route: route_kind(&row.cost),
        latency_ms: parse_latency_ms(&row.lat_ms),
        loss: parse_loss(&row.loss_rate),
        rx_bytes: parse_bytes(&row.rx_bytes),
        tx_bytes: parse_bytes(&row.tx_bytes),
        tunnel: known_text(&row.tunnel_proto),
        nat_type: NatType::from_name(row.nat_type.trim()),
        version: known_text(&row.version).filter(|v| v != "unknown"),
    })
}

fn known_text(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty() && text != "-").then(|| text.to_owned())
}

/// `p2p` is a direct link, `relay(<n>)` goes through `n - 1` other nodes (`n` is the path length, at least 2).
pub fn route_kind(cost: &str) -> RouteKind {
    let cost = cost.trim();
    if cost == "p2p" {
        return RouteKind::Direct;
    }
    cost.strip_prefix("relay(")
        .and_then(|rest| rest.strip_suffix(')'))
        .and_then(|hops| hops.trim().parse::<u32>().ok())
        .map_or(RouteKind::Unknown, |hops| RouteKind::Relayed {
            hops: hops.max(2),
        })
}

/// `"0.10"` → 0.10 ms; `"-"` and garbage → `None`.
pub fn parse_latency_ms(text: &str) -> Option<f64> {
    let value: f64 = text.trim().parse().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

/// `"12.5%"` → 0.125; clamped to `0.0..=1.0`; `"-"` and garbage → `None`.
pub fn parse_loss(text: &str) -> Option<f64> {
    let percent: f64 = text.trim().strip_suffix('%')?.trim().parse().ok()?;
    percent
        .is_finite()
        .then(|| (percent / 100.0).clamp(0.0, 1.0))
}

/// `"1.59 kB"` → 1590 (decimal units, as the CLI prints them); `"512 B"`; `"-"` and garbage → `None`.
pub fn parse_bytes(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let (number, unit) = text.split_at_checked(split)?;
    let value: f64 = number.parse().ok()?;
    let multiplier: f64 = match unit.trim() {
        "B" => 1.0,
        "kB" | "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        "PB" => 1e15,
        "EB" => 1e18,
        _ => return None,
    };
    let bytes = (value * multiplier).round();
    // Saturate instead of wrapping for absurd values.
    if !bytes.is_finite() || bytes < 0.0 {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "bytes is finite and non-negative here, and `as` saturates at u64::MAX"
    )]
    let bytes = bytes as u64;
    Some(bytes)
}

/// A snapshot from the two CLI reads.
///
/// # Errors
///
/// [`ParseError`] naming the read that was not JSON of the expected shape.
pub fn snapshot(
    node_json: &str,
    peer_json: &str,
    read_at_ms: f64,
) -> Result<NodeSnapshot, ParseError> {
    let node = node_info(node_json)?;
    let peers = peers(peer_json, &node)?;
    Ok(NodeSnapshot {
        read_at_ms,
        node,
        peers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODE: &str = include_str!("../../../tests/fixtures/easytier-2.6.4/cli-node.json");
    const PEER: &str = include_str!("../../../tests/fixtures/easytier-2.6.4/cli-peer.json");
    const ROUTE: &str = include_str!("../../../tests/fixtures/easytier-2.6.4/cli-route.json");

    #[test]
    fn the_node_fixture_maps_to_node_info() {
        let node = node_info(NODE).unwrap();
        assert_eq!(node.peer_id, 4_088_092_882);
        assert_eq!(node.hostname, "example-host-a");
        assert_eq!(node.virtual_ipv4, parse_ipv4_net("10.144.144.1/24"));
        assert_eq!(node.version, "2.6.4-8428a89d");
        assert_eq!(node.nat_type, NatType::PortRestricted);
        assert_eq!(
            node.public_ips,
            ["203.0.113.10", "203.0.113.11", "2001:db8::1"]
        );
        assert_eq!(node.listeners, ["tcp://127.0.0.1:41010"]);
        assert_eq!(
            node.proxy_cidrs,
            [parse_ipv4_net("192.168.50.0/24").unwrap()]
        );
    }

    #[test]
    fn the_peer_fixture_maps_to_peers_without_the_local_row() {
        let snapshot = snapshot(NODE, PEER, 1234.0).unwrap();
        assert_eq!(snapshot.read_at_ms, 1234.0);
        assert_eq!(snapshot.peers.len(), 1);
        let peer = snapshot.peers.first().unwrap();
        assert_eq!(peer.peer_id, 979_611_292);
        assert_eq!(peer.hostname, "example-host-b");
        assert_eq!(peer.virtual_ipv4, parse_ipv4_net("10.144.144.2/24"));
        assert_eq!(peer.route, RouteKind::Direct);
        assert_eq!(peer.latency_ms, Some(0.10));
        assert_eq!(peer.loss, Some(0.0));
        assert_eq!(peer.rx_bytes, Some(1590));
        assert_eq!(peer.tx_bytes, Some(2010));
        assert_eq!(peer.tunnel.as_deref(), Some("tcp"));
        assert_eq!(peer.nat_type, NatType::PortRestricted);
        assert_eq!(peer.version.as_deref(), Some("2.6.4-8428a89d"));
    }

    #[test]
    fn humanized_numbers_parse() {
        assert_eq!(parse_bytes("1.59 kB"), Some(1590));
        assert_eq!(parse_bytes("512 B"), Some(512));
        assert_eq!(parse_bytes("0 B"), Some(0));
        assert_eq!(parse_bytes("3.4 GB"), Some(3_400_000_000));
        assert_eq!(parse_bytes("1 MB"), Some(1_000_000));
        assert_eq!(parse_bytes("-"), None);
        assert_eq!(parse_bytes(""), None);
        assert_eq!(parse_bytes("12 parsecs"), None);
        assert_eq!(parse_latency_ms("0.10"), Some(0.10));
        assert_eq!(parse_latency_ms("-"), None);
        assert_eq!(parse_loss("0.0%"), Some(0.0));
        assert_eq!(parse_loss("12.5%"), Some(0.125));
        assert_eq!(parse_loss("250%"), Some(1.0));
        assert_eq!(parse_loss("-"), None);
    }

    #[test]
    fn route_kinds_follow_the_cost_column() {
        assert_eq!(route_kind("p2p"), RouteKind::Direct);
        assert_eq!(route_kind("relay(2)"), RouteKind::Relayed { hops: 2 });
        assert_eq!(route_kind("relay(5)"), RouteKind::Relayed { hops: 5 });
        assert_eq!(route_kind("relay(1)"), RouteKind::Relayed { hops: 2 });
        assert_eq!(route_kind("Local"), RouteKind::Unknown);
        assert_eq!(route_kind("relay(x)"), RouteKind::Unknown);
        assert_eq!(route_kind(""), RouteKind::Unknown);
    }

    #[test]
    fn route_fixture_agrees_with_the_peer_fixture_on_directness() {
        // `route` reports `path_len` 1 for a direct peer: the same fact `cost = p2p` carries.
        let routes: serde_json::Value = serde_json::from_str(ROUTE).unwrap();
        let direct = routes
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["hostname"] == "example-host-b")
            .unwrap();
        assert_eq!(direct["path_len"], 1);
        assert_eq!(route_kind("p2p"), RouteKind::Direct);
    }

    #[test]
    fn unknown_fields_missing_fields_and_numbers_as_text_are_tolerated() {
        let node = r#"{"peer_id":"77","hostname":"h","brand_new_field":{"a":1},"stun_info":{"udp_nat_type":"3"}}"#;
        let info = node_info(node).unwrap();
        assert_eq!(info.peer_id, 77);
        assert_eq!(info.nat_type, NatType::FullCone);
        assert_eq!(info.virtual_ipv4, None);
        let rows = r#"[
            {"cost":"Local","id":77,"hostname":"h"},
            {"cost":"relay(3)","id":8,"hostname":"far","lat_ms":12,"loss_rate":"1.5%","rx_bytes":"2 MB","tx_bytes":"-","ipv4":"10.0.0.8","tunnel_proto":"-","nat_type":"Symmetric","version":"unknown","extra":true},
            {"cost":"p2p","id":"not a number"},
            {"cost":"p2p","id":77,"hostname":"same id as local"}
        ]"#;
        let peers = peers(rows, &info).unwrap();
        assert_eq!(peers.len(), 1);
        let far = peers.first().unwrap();
        assert_eq!(far.route, RouteKind::Relayed { hops: 3 });
        assert_eq!(far.latency_ms, Some(12.0));
        assert_eq!(far.loss, Some(0.015));
        assert_eq!(far.rx_bytes, Some(2_000_000));
        assert_eq!(far.tx_bytes, None);
        assert_eq!(far.virtual_ipv4, parse_ipv4_net("10.0.0.8"));
        assert_eq!(far.tunnel, None);
        assert_eq!(far.version, None);
        assert_eq!(far.nat_type, NatType::Symmetric);
    }

    #[test]
    fn output_that_is_not_json_is_an_error_not_a_panic() {
        assert_eq!(
            node_info("Error: connection refused"),
            Err(ParseError::Node)
        );
        assert_eq!(snapshot("{}", "{}", 0.0), Err(ParseError::Peers));
        assert_eq!(snapshot("[]", "[]", 0.0), Err(ParseError::Node));
    }
}
