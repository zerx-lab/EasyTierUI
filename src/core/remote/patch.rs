//! A change to the config of a running instance, built from checked values and serialized the way the core's
//! `PatchConfig` reads it.
//!
//! The core is unforgiving: a `UrlPatch` whose url does not parse panics the whole `easytier-core` process (every
//! later request to the machine then times out), and the JSON of a patch needs all six repeated lists even when
//! empty. So every value is validated when it is added ([`ConfigPatch`]'s methods return
//! [`RemoteError::Invalid`] and leave the patch unchanged), a `CLEAR` carries a fixed parseable url, and
//! serialization always writes every list.

use std::net::{IpAddr, SocketAddr};

use ipnet::Ipv4Net;
use serde_json::{Map, Value, json};

use super::{
    error::RemoteError,
    rpc_model::{PortForwardEntry, TransportProtocol},
};
use crate::core::net::{
    NetError, PeerUrl, parse_ip_addr, parse_ipv4_inet, parse_ipv4_subnet, parse_port,
    parse_socket_addr,
};

/// The url a `CLEAR` carries: the core ignores it for that action but still parses it, and a url that does not
/// parse panics the core.
const CLEAR_PLACEHOLDER_URL: &str = "tcp://0.0.0.0:0";

/// Longest hostname accepted (the same limit as a profile's).
const MAX_HOSTNAME_CHARS: usize = 32;

/// The core's `ConfigPatchAction` codes.
const ACTION_ADD: u8 = 0;
const ACTION_REMOVE: u8 = 1;
const ACTION_CLEAR: u8 = 2;

/// One operation on a list of the config.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Op<T> {
    /// Appends the value.
    Add(T),
    /// Removes the entries equal to the value.
    Remove(T),
    /// Empties the list.
    Clear,
}

/// A subnet to proxy, optionally announced under another range of the same size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProxyNetwork {
    cidr: Ipv4Net,
    mapped: Option<Ipv4Net>,
}

/// A change to a running instance's config: any mix of the supported kinds. Operations on one list run in the
/// order they were added (`clear` then `add` replaces the list).
///
/// Sent with [`RemoteClient::patch_config`](super::RemoteClient::patch_config), which changes the running instance
/// only.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigPatch {
    hostname: Option<String>,
    ipv4: Option<Ipv4Net>,
    disable_relay_data: Option<bool>,
    port_forwards: Vec<Op<PortForwardEntry>>,
    proxy_networks: Vec<Op<ProxyNetwork>>,
    routes: Vec<Op<Ipv4Net>>,
    exit_nodes: Vec<Op<IpAddr>>,
    mapped_listeners: Vec<Op<String>>,
    connectors: Vec<Op<String>>,
    tcp_whitelist: Vec<Op<String>>,
    udp_whitelist: Vec<Op<String>>,
}

impl ConfigPatch {
    /// An empty patch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the patch changes nothing.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Sets the hostname the node announces.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] for an empty name, control characters, or more than 32 characters.
    pub fn hostname(&mut self, hostname: &str) -> Result<&mut Self, RemoteError> {
        let name = hostname.trim();
        if name.is_empty() {
            return Err(invalid_text("hostname: this is empty"));
        }
        if name.chars().any(char::is_control) {
            return Err(invalid_text("hostname: contains control characters"));
        }
        if name.chars().count() > MAX_HOSTNAME_CHARS {
            return Err(invalid_text("hostname: longer than 32 characters"));
        }
        self.hostname = Some(name.to_owned());
        Ok(self)
    }

    /// Sets the node's virtual IPv4 address with its prefix (`10.144.144.1/24`). The core ignores it while the
    /// instance uses DHCP.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] when it is not `a.b.c.d/len` with a prefix of 8 to 30 and a usable host address.
    pub fn virtual_ipv4(&mut self, cidr: &str) -> Result<&mut Self, RemoteError> {
        self.ipv4 = Some(parse_ipv4_inet("ipv4", cidr).map_err(invalid)?);
        Ok(self)
    }

    /// Turns the refusal to relay other nodes' data on or off.
    pub fn disable_relay_data(&mut self, disabled: bool) -> &mut Self {
        self.disable_relay_data = Some(disabled);
        self
    }

    /// Adds a port forward: connections to `bind` on this node go to `dst` through the mesh.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] when either is not `ip:port` with a port from 1 to 65535.
    pub fn add_port_forward(
        &mut self,
        protocol: TransportProtocol,
        bind: &str,
        dst: &str,
    ) -> Result<&mut Self, RemoteError> {
        let entry = port_forward(protocol, bind, dst)?;
        self.port_forwards.push(Op::Add(entry));
        Ok(self)
    }

    /// Removes the port forward equal to these values.
    ///
    /// # Errors
    ///
    /// As [`ConfigPatch::add_port_forward`].
    pub fn remove_port_forward(
        &mut self,
        protocol: TransportProtocol,
        bind: &str,
        dst: &str,
    ) -> Result<&mut Self, RemoteError> {
        let entry = port_forward(protocol, bind, dst)?;
        self.port_forwards.push(Op::Remove(entry));
        Ok(self)
    }

    /// Adds a subnet to proxy (`192.168.50.0/24`); `mapped` announces it to the mesh as another range of the
    /// same prefix length.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] when a range is not a network address with a prefix of 8 to 32, or the two prefix
    /// lengths differ.
    pub fn add_proxy_network(
        &mut self,
        cidr: &str,
        mapped: Option<&str>,
    ) -> Result<&mut Self, RemoteError> {
        let cidr = parse_ipv4_subnet("proxy_network", cidr).map_err(invalid)?;
        let mapped = mapped
            .map(|text| parse_ipv4_subnet("mapped_cidr", text).map_err(invalid))
            .transpose()?;
        if mapped.is_some_and(|mapped| mapped.prefix_len() != cidr.prefix_len()) {
            return Err(invalid_text(
                "mapped_cidr: must have the same prefix length as the proxied subnet",
            ));
        }
        self.proxy_networks
            .push(Op::Add(ProxyNetwork { cidr, mapped }));
        Ok(self)
    }

    /// Stops proxying a subnet.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] when it is not a network address with a prefix of 8 to 32.
    pub fn remove_proxy_network(&mut self, cidr: &str) -> Result<&mut Self, RemoteError> {
        let cidr = parse_ipv4_subnet("proxy_network", cidr).map_err(invalid)?;
        self.proxy_networks
            .push(Op::Remove(ProxyNetwork { cidr, mapped: None }));
        Ok(self)
    }

    /// Adds a manual route (`10.0.0.0/16`).
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] when it is not a network address with a prefix of 8 to 32.
    pub fn add_route(&mut self, cidr: &str) -> Result<&mut Self, RemoteError> {
        let cidr = parse_ipv4_subnet("route", cidr).map_err(invalid)?;
        self.routes.push(Op::Add(cidr));
        Ok(self)
    }

    /// Removes a manual route.
    ///
    /// # Errors
    ///
    /// As [`ConfigPatch::add_route`].
    pub fn remove_route(&mut self, cidr: &str) -> Result<&mut Self, RemoteError> {
        let cidr = parse_ipv4_subnet("route", cidr).map_err(invalid)?;
        self.routes.push(Op::Remove(cidr));
        Ok(self)
    }

    /// Adds an exit node: the peer whose virtual address internet traffic is sent through.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] when it is not an IP address.
    pub fn add_exit_node(&mut self, ip: &str) -> Result<&mut Self, RemoteError> {
        let ip = parse_ip_addr("exit_node", ip).map_err(invalid)?;
        self.exit_nodes.push(Op::Add(ip));
        Ok(self)
    }

    /// Removes an exit node.
    ///
    /// # Errors
    ///
    /// As [`ConfigPatch::add_exit_node`].
    pub fn remove_exit_node(&mut self, ip: &str) -> Result<&mut Self, RemoteError> {
        let ip = parse_ip_addr("exit_node", ip).map_err(invalid)?;
        self.exit_nodes.push(Op::Remove(ip));
        Ok(self)
    }

    /// Adds a mapped listener: the public address other nodes reach one of this node's listeners at.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] for a url the scheme table does not accept, or a scheme that can only find peers
    /// (`http`, `https`, `txt`, `srv`).
    pub fn add_mapped_listener(&mut self, url: &str) -> Result<&mut Self, RemoteError> {
        let url = checked_url("mapped_listener", url, false)?;
        self.mapped_listeners.push(Op::Add(url));
        Ok(self)
    }

    /// Removes a mapped listener (the url as the core lists it).
    ///
    /// # Errors
    ///
    /// As [`ConfigPatch::add_mapped_listener`].
    pub fn remove_mapped_listener(&mut self, url: &str) -> Result<&mut Self, RemoteError> {
        let url = checked_url("mapped_listener", url, false)?;
        self.mapped_listeners.push(Op::Remove(url));
        Ok(self)
    }

    /// Removes every mapped listener.
    pub fn clear_mapped_listeners(&mut self) -> &mut Self {
        self.mapped_listeners.push(Op::Clear);
        self
    }

    /// Adds a connector: a peer address the node dials by itself.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] for a url the scheme table does not accept.
    pub fn add_connector(&mut self, url: &str) -> Result<&mut Self, RemoteError> {
        let url = checked_url("connector", url, true)?;
        self.connectors.push(Op::Add(url));
        Ok(self)
    }

    /// Removes a connector (the url as the core lists it).
    ///
    /// # Errors
    ///
    /// As [`ConfigPatch::add_connector`].
    pub fn remove_connector(&mut self, url: &str) -> Result<&mut Self, RemoteError> {
        let url = checked_url("connector", url, true)?;
        self.connectors.push(Op::Remove(url));
        Ok(self)
    }

    /// Removes every connector.
    pub fn clear_connectors(&mut self) -> &mut Self {
        self.connectors.push(Op::Clear);
        self
    }

    /// Opens a port (`80`) or a range (`1000-2000`) of the whitelist.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] when it is not a port from 1 to 65535 or an ascending range of them.
    pub fn add_whitelist_port(
        &mut self,
        protocol: TransportProtocol,
        port: &str,
    ) -> Result<&mut Self, RemoteError> {
        let entry = whitelist_entry(port)?;
        self.whitelist_mut(protocol).push(Op::Add(entry));
        Ok(self)
    }

    /// Closes a port or range of the whitelist (the entry as the core lists it).
    ///
    /// # Errors
    ///
    /// As [`ConfigPatch::add_whitelist_port`].
    pub fn remove_whitelist_port(
        &mut self,
        protocol: TransportProtocol,
        port: &str,
    ) -> Result<&mut Self, RemoteError> {
        let entry = whitelist_entry(port)?;
        self.whitelist_mut(protocol).push(Op::Remove(entry));
        Ok(self)
    }

    /// Empties the whitelist of one protocol.
    pub fn clear_whitelist(&mut self, protocol: TransportProtocol) -> &mut Self {
        self.whitelist_mut(protocol).push(Op::Clear);
        self
    }

    fn whitelist_mut(&mut self, protocol: TransportProtocol) -> &mut Vec<Op<String>> {
        match protocol {
            TransportProtocol::Tcp => &mut self.tcp_whitelist,
            TransportProtocol::Udp => &mut self.udp_whitelist,
        }
    }

    /// The patch as the core's JSON: all six repeated lists always present, `acl` only when a whitelist changes.
    pub(super) fn to_json(&self) -> Value {
        let mut patch = Map::new();
        if let Some(hostname) = &self.hostname {
            patch.insert("hostname".into(), json!(hostname));
        }
        if let Some(ipv4) = self.ipv4 {
            patch.insert("ipv4".into(), inet_json(ipv4));
        }
        if let Some(disabled) = self.disable_relay_data {
            patch.insert("disable_relay_data".into(), json!(disabled));
        }
        patch.insert(
            "port_forwards".into(),
            entries(
                &self.port_forwards,
                |entry| {
                    vec![(
                        "cfg",
                        json!({
                            "bind_addr": socket_json(entry.bind),
                            "dst_addr": socket_json(entry.dst),
                            "socket_type": socket_type(entry.protocol),
                        }),
                    )]
                },
                &[],
            ),
        );
        patch.insert(
            "proxy_networks".into(),
            entries(
                &self.proxy_networks,
                |network| {
                    let mut fields = vec![("cidr", inet_json(network.cidr))];
                    if let Some(mapped) = network.mapped {
                        fields.push(("mapped_cidr", inet_json(mapped)));
                    }
                    fields
                },
                &[],
            ),
        );
        patch.insert(
            "routes".into(),
            entries(&self.routes, |cidr| vec![("cidr", inet_json(*cidr))], &[]),
        );
        patch.insert(
            "exit_nodes".into(),
            entries(&self.exit_nodes, |ip| vec![("node", ip_json(*ip))], &[]),
        );
        patch.insert(
            "mapped_listeners".into(),
            url_entries(&self.mapped_listeners),
        );
        patch.insert("connectors".into(), url_entries(&self.connectors));
        if !self.tcp_whitelist.is_empty() || !self.udp_whitelist.is_empty() {
            patch.insert(
                "acl".into(),
                json!({
                    "tcp_whitelist": string_entries(&self.tcp_whitelist),
                    "udp_whitelist": string_entries(&self.udp_whitelist),
                }),
            );
        }
        Value::Object(patch)
    }
}

fn invalid(error: NetError) -> RemoteError {
    RemoteError::Invalid(error.to_string())
}

fn invalid_text(message: &str) -> RemoteError {
    RemoteError::Invalid(message.to_owned())
}

/// A url the scheme table accepts, as the person wrote it (trimmed): the core compares parsed urls when it
/// removes one, so the text only has to parse, and a normalized copy would differ from what the core lists.
fn checked_url(field: &str, text: &str, allow_discovery: bool) -> Result<String, RemoteError> {
    let url = PeerUrl::parse(field, text).map_err(invalid)?;
    if !allow_discovery && url.scheme().is_connector_only() {
        return Err(invalid(NetError::ConnectorOnly {
            field: field.to_owned(),
            scheme: url.scheme(),
        }));
    }
    Ok(text.trim().to_owned())
}

fn port_forward(
    protocol: TransportProtocol,
    bind: &str,
    dst: &str,
) -> Result<PortForwardEntry, RemoteError> {
    Ok(PortForwardEntry {
        protocol,
        bind: parse_socket_addr("bind_addr", bind).map_err(invalid)?,
        dst: parse_socket_addr("dst_addr", dst).map_err(invalid)?,
    })
}

/// A whitelist entry: one port or an ascending `first-last` range, normalized.
fn whitelist_entry(text: &str) -> Result<String, RemoteError> {
    const FIELD: &str = "whitelist_port";
    let text = text.trim();
    let Some((first, last)) = text.split_once('-') else {
        return parse_port(FIELD, text)
            .map(|port| port.to_string())
            .map_err(invalid);
    };
    let first = parse_port(FIELD, first).map_err(invalid)?;
    let last = parse_port(FIELD, last).map_err(invalid)?;
    if first > last {
        return Err(invalid_text("whitelist_port: the range runs backwards"));
    }
    Ok(format!("{first}-{last}"))
}

// --- Serialization ------------------------------------------------------------------------------------------

fn socket_type(protocol: TransportProtocol) -> u8 {
    match protocol {
        TransportProtocol::Tcp => 0,
        TransportProtocol::Udp => 1,
    }
}

fn inet_json(net: Ipv4Net) -> Value {
    json!({
        "address": { "addr": u32::from(net.addr()) },
        "network_length": net.prefix_len(),
    })
}

/// `common.IpAddr` / `common.SocketAddr`'s oneof: `{"Ipv4": {"addr": n}}` or `{"Ipv6": {"part1": .., ..}}`.
fn ip_oneof(ip: IpAddr) -> Value {
    match ip {
        IpAddr::V4(v4) => json!({ "Ipv4": { "addr": u32::from(v4) } }),
        IpAddr::V6(v6) => {
            let [
                a0,
                a1,
                a2,
                a3,
                b0,
                b1,
                b2,
                b3,
                c0,
                c1,
                c2,
                c3,
                d0,
                d1,
                d2,
                d3,
            ] = v6.octets();
            json!({ "Ipv6": {
                "part1": u32::from_be_bytes([a0, a1, a2, a3]),
                "part2": u32::from_be_bytes([b0, b1, b2, b3]),
                "part3": u32::from_be_bytes([c0, c1, c2, c3]),
                "part4": u32::from_be_bytes([d0, d1, d2, d3]),
            } })
        }
    }
}

fn ip_json(ip: IpAddr) -> Value {
    json!({ "ip": ip_oneof(ip) })
}

fn socket_json(addr: SocketAddr) -> Value {
    json!({ "ip": ip_oneof(addr.ip()), "port": addr.port() })
}

/// The `{"action": .., <fields>}` objects of one list. `clear_fields` is what a `CLEAR` carries in place of a
/// value.
fn entries<T>(
    ops: &[Op<T>],
    encode: impl Fn(&T) -> Vec<(&'static str, Value)>,
    clear_fields: &[(&'static str, Value)],
) -> Value {
    Value::Array(
        ops.iter()
            .map(|op| {
                let (action, fields) = match op {
                    Op::Add(value) => (ACTION_ADD, encode(value)),
                    Op::Remove(value) => (ACTION_REMOVE, encode(value)),
                    Op::Clear => (ACTION_CLEAR, clear_fields.to_vec()),
                };
                let mut object = Map::new();
                object.insert("action".into(), json!(action));
                for (name, value) in fields {
                    object.insert(name.into(), value);
                }
                Value::Object(object)
            })
            .collect(),
    )
}

fn url_entries(ops: &[Op<String>]) -> Value {
    entries(
        ops,
        |url| vec![("url", json!({ "url": url }))],
        &[("url", json!({ "url": CLEAR_PLACEHOLDER_URL }))],
    )
}

fn string_entries(ops: &[Op<String>]) -> Value {
    entries(
        ops,
        |value| vec![("value", json!(value))],
        &[("value", json!(""))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_urls(patch: &Value) -> Vec<String> {
        ["mapped_listeners", "connectors"]
            .iter()
            .flat_map(|list| {
                patch[*list]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| entry["url"]["url"].as_str().map(str::to_owned))
            })
            .collect()
    }

    #[test]
    fn rejected_values_leave_the_patch_unchanged() {
        let mut patch = ConfigPatch::new();
        for url in [
            "",
            "   ",
            "not a url",
            "tcp://",
            "tcp://host:0",
            "nope://host:1",
            "tcp://host:99999",
            "tcp://user:pw@host:1",
        ] {
            assert!(
                matches!(patch.add_connector(url), Err(RemoteError::Invalid(_))),
                "{url:?}"
            );
            assert!(patch.remove_connector(url).is_err(), "{url:?}");
            assert!(patch.add_mapped_listener(url).is_err(), "{url:?}");
        }
        assert!(patch.is_empty(), "no rejected url was recorded");
        // Peer-discovery schemes can be connectors but never mapped listeners.
        assert!(
            patch
                .add_mapped_listener("txt://peers.example.com")
                .is_err()
        );
        assert!(patch.add_connector("txt://peers.example.com").is_ok());
        let mut patch = ConfigPatch::new();
        assert!(
            patch
                .add_port_forward(TransportProtocol::Tcp, "127.0.0.1:0", "10.0.0.2:22")
                .is_err()
        );
        assert!(
            patch
                .add_port_forward(TransportProtocol::Tcp, "localhost:80", "10.0.0.2:22")
                .is_err()
        );
        assert!(
            patch.add_proxy_network("10.0.0.7/24", None).is_err(),
            "host bits"
        );
        assert!(
            patch
                .add_proxy_network("10.0.0.0/24", Some("10.1.0.0/16"))
                .is_err()
        );
        assert!(patch.add_route("10.0.0.0/4").is_err());
        assert!(patch.add_exit_node("not-an-ip").is_err());
        assert!(
            patch.virtual_ipv4("10.144.144.0/24").is_err(),
            "network address"
        );
        assert!(patch.hostname("  ").is_err());
        assert!(patch.hostname(&"x".repeat(33)).is_err());
        assert!(patch.hostname("bad\nname").is_err());
        assert!(patch.is_empty(), "nothing invalid was recorded");
    }

    #[test]
    fn whitelist_entries_are_a_port_or_an_ascending_range() {
        let mut patch = ConfigPatch::new();
        for bad in [
            "", "0", "65536", "http", "80-", "-80", "90-80", "1-2-3", "80,81",
        ] {
            assert!(
                patch
                    .add_whitelist_port(TransportProtocol::Tcp, bad)
                    .is_err(),
                "{bad:?}"
            );
        }
        assert!(patch.is_empty());
        patch
            .add_whitelist_port(TransportProtocol::Tcp, " 80 ")
            .unwrap()
            .add_whitelist_port(TransportProtocol::Udp, "1000-2000")
            .unwrap();
        let json = patch.to_json();
        assert_eq!(json["acl"]["tcp_whitelist"][0]["value"], "80");
        assert_eq!(json["acl"]["udp_whitelist"][0]["value"], "1000-2000");
    }

    #[test]
    fn every_list_is_always_present_and_acl_only_with_a_whitelist() {
        let mut patch = ConfigPatch::new();
        patch.hostname("renamed").unwrap();
        let json = patch.to_json();
        for list in [
            "port_forwards",
            "proxy_networks",
            "routes",
            "exit_nodes",
            "mapped_listeners",
            "connectors",
        ] {
            assert_eq!(json[list], json!([]), "{list}");
        }
        assert_eq!(json["hostname"], "renamed");
        assert!(json.get("acl").is_none());
        assert!(json.get("ipv4").is_none());
    }

    #[test]
    fn clear_carries_a_url_the_core_can_parse() {
        let mut patch = ConfigPatch::new();
        patch.clear_connectors().clear_mapped_listeners();
        patch.clear_whitelist(TransportProtocol::Tcp);
        let json = patch.to_json();
        assert_eq!(json["connectors"][0]["action"], 2);
        assert_eq!(json["mapped_listeners"][0]["action"], 2);
        let urls = all_urls(&json);
        assert_eq!(urls.len(), 2);
        for url in urls {
            assert!(url::Url::parse(&url).is_ok(), "{url}");
        }
        // The whitelist clear names no port and the other protocol's list is still present.
        assert_eq!(json["acl"]["tcp_whitelist"][0]["action"], 2);
        assert_eq!(json["acl"]["udp_whitelist"], json!([]));
    }

    #[test]
    fn operations_keep_their_order_and_urls_stay_as_written() {
        let mut patch = ConfigPatch::new();
        patch
            .clear_connectors()
            .add_connector(" ws://relay.example.com:8080/path ")
            .unwrap();
        let json = patch.to_json();
        assert_eq!(json["connectors"][0]["action"], 2);
        assert_eq!(json["connectors"][1]["action"], 0);
        assert_eq!(
            json["connectors"][1]["url"]["url"],
            "ws://relay.example.com:8080/path"
        );
    }

    #[test]
    fn ipv6_addresses_are_split_into_four_parts() {
        let mut patch = ConfigPatch::new();
        patch.add_exit_node("2001:db8::2").unwrap();
        patch
            .add_port_forward(TransportProtocol::Udp, "[::1]:5353", "[2001:db8::1]:53")
            .unwrap();
        let json = patch.to_json();
        assert_eq!(
            json["exit_nodes"][0]["node"]["ip"]["Ipv6"],
            json!({"part1": 0x2001_0db8_u32, "part2": 0, "part3": 0, "part4": 2})
        );
        let forward = &json["port_forwards"][0]["cfg"];
        assert_eq!(forward["bind_addr"]["ip"]["Ipv6"]["part4"], 1);
        assert_eq!(forward["bind_addr"]["port"], 5353);
        assert_eq!(forward["socket_type"], 1);
    }
}
