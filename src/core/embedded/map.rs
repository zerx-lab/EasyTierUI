//! The adapter from EasyTier's `NetworkInstanceRunningInfo` JSON (what the bridge crate returns; the schema
//! easytier-web's `networks/info` serves too) to the app's [`NodeSnapshot`]; its peers go through
//! [`crate::core::pairs`].
//!
//! Pure functions over text: no EasyTier crate, so it compiles and is tested on every platform. Parsing is
//! tolerant (`#[serde(default)]`, unknown fields ignored); a renamed upstream field is fixed here only.

use ipnet::Ipv4Net;
use serde::Deserialize;

use crate::core::{
    pairs::{self, InetDoc, PairDoc, StunDoc, inet, nat},
    state::{NodeInfo, NodeSnapshot},
};

/// Why a running-info document did not become a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MapError {
    /// The text is not the expected JSON.
    #[error("the running info could not be read: {0}")]
    Parse(String),
    /// The instance has not published its own node yet.
    #[error("the instance has not reported its node yet")]
    NoNode,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InfoDoc {
    my_node_info: Option<NodeDoc>,
    peer_route_pairs: Vec<PairDoc>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct NodeDoc {
    virtual_ipv4: Option<InetDoc>,
    hostname: String,
    version: String,
    stun_info: Option<StunDoc>,
    listeners: Vec<UrlDoc>,
    peer_id: u32,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UrlDoc {
    url: String,
}

/// Maps the running-info JSON of one instance to a snapshot read at `read_at_ms`.
///
/// `proxy_cidrs` are the subnets the local config proxies: the running info does not carry them.
///
/// # Errors
///
/// [`MapError::Parse`] when `json` is not a running-info document, [`MapError::NoNode`] when it has no node of its
/// own yet.
pub fn snapshot_from_json(
    json: &str,
    read_at_ms: f64,
    proxy_cidrs: Vec<Ipv4Net>,
) -> Result<NodeSnapshot, MapError> {
    let doc: InfoDoc =
        serde_json::from_str(json).map_err(|error| MapError::Parse(error.to_string()))?;
    let node = doc.my_node_info.ok_or(MapError::NoNode)?;
    let peers = pairs::peers(&doc.peer_route_pairs, node.peer_id);
    Ok(NodeSnapshot {
        read_at_ms,
        node: NodeInfo {
            peer_id: node.peer_id,
            hostname: node.hostname,
            virtual_ipv4: node.virtual_ipv4.as_ref().and_then(inet),
            // `ring://` is the core's in-process listener; it is not an address anyone can dial.
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
            nat_type: nat(node.stun_info.as_ref()),
            proxy_cidrs,
            version: node.version,
        },
        peers,
    })
}

/// The subnets a rendered config proxies (`[[proxy_network]] cidr`), skipping anything that is not an IPv4 CIDR.
pub fn proxy_cidrs_from_config(config_toml: &str) -> Vec<Ipv4Net> {
    let Ok(table) = config_toml.parse::<toml::Table>() else {
        return Vec::new();
    };
    table
        .get("proxy_network")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("cidr")?.as_str()?.parse::<Ipv4Net>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::state::{NatType, RouteKind};

    /// Captured from two `--no-tun` nodes on loopback through easytier-web (`networks/info`), addresses redacted.
    const TWO_NODES: &str =
        include_str!("../../../tests/fixtures/easytier-2.6.4/embedded-running-info.json");

    #[test]
    fn a_direct_peer_is_mapped_with_its_link_numbers() {
        let snapshot = snapshot_from_json(TWO_NODES, 1_000.0, Vec::new()).unwrap();
        assert_eq!(snapshot.read_at_ms, 1_000.0);
        assert_eq!(snapshot.node.hostname, "example-host-b");
        assert_eq!(snapshot.node.peer_id, 246_416_169);
        assert_eq!(snapshot.node.virtual_ipv4, "10.144.146.2/24".parse().ok());
        assert_eq!(snapshot.node.version, "2.6.4-8428a89d");
        assert_eq!(snapshot.node.nat_type, NatType::PortRestricted);
        assert_eq!(snapshot.node.public_ips, ["203.0.113.1", "2001:db8::1"]);
        assert_eq!(snapshot.node.listeners, ["tcp://127.0.0.1:42011"]);

        let [peer] = snapshot.peers.as_slice() else {
            panic!("one peer expected, got {:?}", snapshot.peers);
        };
        assert_eq!(peer.peer_id, 1_088_216_467);
        assert_eq!(peer.hostname, "example-host-a");
        assert_eq!(peer.virtual_ipv4, "10.144.146.1/24".parse().ok());
        assert_eq!(peer.route, RouteKind::Direct);
        assert_eq!(peer.latency_ms, Some(0.097));
        assert_eq!(peer.loss, Some(0.0));
        assert_eq!(peer.rx_bytes, Some(1683));
        assert_eq!(peer.tx_bytes, Some(1871));
        assert_eq!(peer.tunnel.as_deref(), Some("tcp"));
        assert_eq!(peer.nat_type, NatType::PortRestricted);
        assert_eq!(peer.version.as_deref(), Some("2.6.4-8428a89d"));
    }

    #[test]
    fn a_relayed_peer_uses_the_path_numbers_and_has_no_link() {
        let json = r#"{
            "my_node_info": {"peer_id": 1, "hostname": "me", "version": "2.6.4"},
            "peer_route_pairs": [
                {"route": {"peer_id": 1, "hostname": "me", "cost": 0}},
                {"route": {"peer_id": 9, "hostname": "far", "cost": 3, "path_latency": 80,
                           "path_latency_latency_first": 60,
                           "ipv4_addr": {"address": {"addr": 167772170}, "network_length": 8}},
                 "peer": null}
            ]
        }"#;
        let snapshot = snapshot_from_json(json, 0.0, Vec::new()).unwrap();
        let [peer] = snapshot.peers.as_slice() else {
            panic!("the local node is not a peer: {:?}", snapshot.peers);
        };
        assert_eq!(peer.route, RouteKind::Relayed { hops: 3 });
        assert_eq!(peer.latency_ms, Some(60.0));
        assert_eq!(peer.virtual_ipv4, "10.0.0.10/8".parse().ok());
        assert_eq!(peer.rx_bytes, None);
        assert_eq!(peer.tunnel, None);
        assert_eq!(peer.loss, None);
    }

    #[test]
    fn an_unassigned_address_is_none() {
        let json = r#"{"my_node_info": {"peer_id": 1, "virtual_ipv4": {"address": {"addr": 0}, "network_length": 24}}}"#;
        let snapshot = snapshot_from_json(json, 0.0, Vec::new()).unwrap();
        assert_eq!(snapshot.node.virtual_ipv4, None);
        assert!(snapshot.peers.is_empty());
    }

    #[test]
    fn a_document_without_a_node_is_not_a_snapshot() {
        assert_eq!(
            snapshot_from_json("{}", 0.0, Vec::new()),
            Err(MapError::NoNode)
        );
        assert!(matches!(
            snapshot_from_json("not json", 0.0, Vec::new()),
            Err(MapError::Parse(_))
        ));
    }

    #[test]
    fn proxy_networks_come_from_the_rendered_config() {
        let toml = r#"
            [[proxy_network]]
            cidr = "192.168.50.0/24"
            [[proxy_network]]
            cidr = "not-a-cidr"
        "#;
        assert_eq!(
            proxy_cidrs_from_config(toml),
            ["192.168.50.0/24".parse::<Ipv4Net>().unwrap()]
        );
        assert!(proxy_cidrs_from_config("= broken").is_empty());
    }
}
