//! The adapter from `easytier-cli -o json` (2.6.x) to the app's [`NodeSnapshot`]: the one place that names the CLI's
//! JSON fields. Parsing is tolerant (`#[serde(default)]`, unknown fields ignored, numbers or strings accepted) so a
//! point release that adds a field, or prints a number as text, does not blank the peer list.
//!
//! The peers come from `easytier-cli -v peer`: the raw peer-route pairs, mapped by [`crate::core::pairs`], whose byte
//! counters are exact. Plain `easytier-cli peer` humanizes them to three significant digits (`"1.59 kB"`), too
//! coarse for a rate: at 10 MB moved, a two-second step reads either nothing or 5 kB/s.

use serde::{Deserialize, Deserializer};

use crate::core::{
    pairs::{self, PairDoc},
    state::{NatType, NodeInfo, NodeSnapshot, PeerSummary, parse_ipv4_net},
};

/// Why the CLI's output could not be mapped.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// `easytier-cli node` printed something that is not a JSON object.
    #[error("the node report is not valid JSON")]
    Node,
    /// `easytier-cli -v peer` printed something that is not a JSON array of peer-route pairs.
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

/// The peers of `local`, from `easytier-cli -v -o json peer` (the local node is never among them, and is left out if
/// a later release lists it).
///
/// # Errors
///
/// [`ParseError::Peers`] when the text is not a JSON array of peer-route pairs.
pub fn peers(json: &str, local: &NodeInfo) -> Result<Vec<PeerSummary>, ParseError> {
    let pairs: Vec<PairDoc> = serde_json::from_str(json).map_err(|_| ParseError::Peers)?;
    Ok(pairs::peers(&pairs, local.peer_id))
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
    const PEERS: &str =
        include_str!("../../../tests/fixtures/easytier-2.6.4/cli-peer-verbose.json");

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
    fn the_verbose_peer_list_maps_with_exact_counters() {
        let snapshot = snapshot(NODE, PEERS, 1234.0).unwrap();
        assert_eq!(snapshot.read_at_ms, 1234.0);
        let [peer] = snapshot.peers.as_slice() else {
            panic!("one peer expected, got {:?}", snapshot.peers);
        };
        assert_eq!(peer.hostname, "example-host-b");
        assert_eq!(peer.virtual_ipv4, parse_ipv4_net("10.144.144.2/24"));
        assert_eq!(peer.route, crate::core::state::RouteKind::Direct);
        assert_eq!(peer.rx_bytes, Some(1474));
        assert_eq!(peer.tx_bytes, Some(1846));
    }

    #[test]
    fn a_listed_local_node_is_not_a_peer() {
        let info = node_info(r#"{"peer_id": 77, "hostname": "h"}"#).unwrap();
        let rows = r#"[
            {"route": {"peer_id": 77, "hostname": "h", "cost": 0}},
            {"route": {"peer_id": 8, "hostname": "far", "cost": 3, "path_latency": 12, "brand_new": true}}
        ]"#;
        let peers = peers(rows, &info).unwrap();
        let [far] = peers.as_slice() else {
            panic!("one peer expected, got {peers:?}");
        };
        assert_eq!(far.peer_id, 8);
        assert_eq!(far.latency_ms, Some(12.0));
    }

    #[test]
    fn unknown_fields_missing_fields_and_numbers_as_text_are_tolerated() {
        let node = r#"{"peer_id":"77","hostname":"h","brand_new_field":{"a":1},"stun_info":{"udp_nat_type":"3"}}"#;
        let info = node_info(node).unwrap();
        assert_eq!(info.peer_id, 77);
        assert_eq!(info.nat_type, NatType::FullCone);
        assert_eq!(info.virtual_ipv4, None);
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
