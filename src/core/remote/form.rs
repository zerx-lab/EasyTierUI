//! The editable part of a network's config on a remote machine.
//!
//! easytier-web's `NetworkConfig` has more than sixty fields and gains new ones with each release. The console
//! edits a subset; [`NetworkConfigForm`] keeps the config's original JSON next to that subset and patches only the
//! keys it owns when it is sent back, so fields it does not know (a newer release's, ACLs, port forwards, VPN
//! portal) survive an edit untouched.

use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use ipnet::Ipv4Net;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{error::RemoteError, ids::new_instance_id};
use crate::core::{
    acl::{AclConfig, wire},
    secret::Secret,
    settings::{DEFAULT_VPN_PORTAL_PORT, Flag, NodeFlags, PortForward, Proto, VpnPortal},
};

/// The prefix length easytier-web assumes when a config has an address and no length.
const DEFAULT_PREFIX_LEN: u8 = 24;

/// How a node finds its peers (`networking_method`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Networking {
    /// Through one public server: its URL is the only peer. A config without a method means this (the server
    /// reads a missing method as 0).
    PublicServer {
        /// The server's URL, as typed.
        url: String,
    },
    /// Through the peer URLs of the config.
    Manual,
    /// Alone: no peers are dialed; others connect to its listeners.
    Standalone,
}

/// The encryption of the node's links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Encryption {
    /// On, with the core's default algorithm (`aes-gcm` since 2.x).
    CoreDefault,
    /// On, with this algorithm (`aes-gcm`, `aes-256-gcm`, `chacha20`, `xor`). `xor` is obfuscation.
    Algorithm(String),
    /// Off. Every peer must agree.
    Disabled,
}

/// The ACL of a config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AclState {
    /// No ACL: the key is missing or null, or the ACL has no chains and no groups.
    Off,
    /// An ACL the app understands.
    On(AclConfig),
    /// An ACL the app cannot read (a newer release's, or invalid). It is kept as the server sent it on save.
    Unreadable,
}

/// A network config in the app's terms, with the server's original JSON behind it.
#[derive(Clone, PartialEq)]
pub struct NetworkConfigForm {
    /// The instance this config is for; it must match the instance in a save.
    pub instance_id: Uuid,
    /// The network's name.
    pub network_name: String,
    /// The network secret. Empty for a credential-mode node, whose key lives in fields the form leaves alone.
    pub network_secret: Secret,
    /// The hostname peers see; `None` for the machine's own.
    pub hostname: Option<String>,
    /// Whether the address comes from DHCP.
    pub dhcp: bool,
    /// The node's static virtual IPv4 with its prefix; `None` with DHCP or when unset.
    pub virtual_ipv4: Option<Ipv4Net>,
    /// How peers are found.
    pub networking: Networking,
    /// Peer URLs dialed when [`Networking::Manual`].
    pub peers: Vec<String>,
    /// Listener URLs; empty for the core's defaults.
    pub listeners: Vec<String>,
    /// Subnets this node proxies into the network (`cidr` or `cidr->mapped`).
    pub proxy_cidrs: Vec<String>,
    /// Whether the node runs without a TUN device.
    pub no_tun: bool,
    /// Whether this node is an exit node for others.
    pub enable_exit_node: bool,
    /// Exit nodes this node sends its default traffic through.
    pub exit_nodes: Vec<String>,
    /// The SOCKS5 portal's port; `None` for no portal.
    pub socks5_port: Option<u16>,
    /// The behavior switches (`latency_first`, `disable_p2p`, ... ; `accept_dns` is `enable_magic_dns` here).
    pub flags: NodeFlags,
    /// Whether this node relays RPC for every peer, whatever network it is in (`relay_all_peer_rpc`).
    pub relay_all_peer_rpc: bool,
    /// Whether only peers that know the network secret may connect (`enable_private_mode`).
    pub private_mode: bool,
    /// The MTU; `None` for the core's default.
    pub mtu: Option<u16>,
    /// The TUN interface name; `None` lets the system choose.
    pub dev_name: Option<String>,
    /// The limit on traffic received by this instance, in bytes per second; `None` for unlimited.
    pub instance_recv_bps_limit: Option<u64>,
    /// Public addresses of the listeners, for nodes behind a port mapping (`mapped_listeners`).
    pub mapped_listeners: Vec<String>,
    /// Manual routes (CIDRs) when manual routing is on (`enable_manual_routes` plus `routes`); `Some` of an empty
    /// list means no routes at all.
    pub manual_routes: Option<Vec<String>>,
    /// The networks this node relays for when the whitelist is on (`enable_relay_network_whitelist` plus
    /// `relay_network_whitelist`); `Some` of an empty list relays for none.
    pub relay_whitelist: Option<Vec<String>>,
    /// The WireGuard VPN portal; `None` when off. It listens on every interface.
    pub vpn_portal: Option<VpnPortal>,
    /// The port forwards. Rows the form cannot read are kept on the server value until this list is edited
    /// ([`NetworkConfigForm::forwards_unreadable`]).
    pub port_forwards: Vec<PortForward>,
    /// The access control list.
    pub acl: AclState,
    /// The links' encryption.
    pub encryption: Encryption,
    /// The config as the server sent it (or `{}` for a new one): the base the edits are patched onto. It holds the
    /// network secret in clear, so it is never shown or formatted.
    raw: Value,
}

impl fmt::Debug for NetworkConfigForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkConfigForm")
            .field("instance_id", &self.instance_id)
            .field("network_name", &self.network_name)
            .field("network_secret", &self.network_secret)
            .field("dhcp", &self.dhcp)
            .field("networking", &self.networking)
            .finish_non_exhaustive()
    }
}

impl NetworkConfigForm {
    /// A new network config: DHCP, manual peers (none yet), no listeners, a TUN, private mode on, no exit node, no
    /// SOCKS5 portal, no VPN portal, no ACL, EasyTier's default switches and encryption on with `aes-gcm`.
    pub fn new(instance_id: Uuid, network_name: impl Into<String>, network_secret: Secret) -> Self {
        Self {
            instance_id,
            network_name: network_name.into(),
            network_secret,
            hostname: None,
            dhcp: true,
            virtual_ipv4: None,
            networking: Networking::Manual,
            peers: Vec::new(),
            listeners: Vec::new(),
            proxy_cidrs: Vec::new(),
            no_tun: false,
            enable_exit_node: false,
            exit_nodes: Vec::new(),
            socks5_port: None,
            flags: NodeFlags::default(),
            relay_all_peer_rpc: false,
            private_mode: true,
            mtu: None,
            dev_name: None,
            instance_recv_bps_limit: None,
            mapped_listeners: Vec::new(),
            manual_routes: None,
            relay_whitelist: None,
            vpn_portal: None,
            port_forwards: Vec::new(),
            acl: AclState::Off,
            encryption: Encryption::Algorithm("aes-gcm".into()),
            raw: Value::Object(Map::new()),
        }
    }

    /// Reads the server's `NetworkConfig` JSON, keeping it for the round trip.
    ///
    /// A config with no usable `instance_id` gets a fresh one (a config parsed from TOML may have none).
    ///
    /// # Errors
    ///
    /// [`RemoteError::Decode`] when the value is not a JSON object.
    pub fn from_json(raw: Value) -> Result<Self, RemoteError> {
        let Value::Object(map) = &raw else {
            return Err(RemoteError::Decode(
                "unexpected shape: the config is not an object".into(),
            ));
        };
        let instance_id = string(map, "instance_id")
            .and_then(|text| text.parse().ok())
            .unwrap_or_else(new_instance_id);
        let networking = match map.get("networking_method").and_then(Value::as_i64) {
            Some(1) => Networking::Manual,
            Some(2) => Networking::Standalone,
            _ => Networking::PublicServer {
                url: string(map, "public_server_url").unwrap_or_default(),
            },
        };
        let encryption = if boolean(map, "disable_encryption") {
            Encryption::Disabled
        } else {
            match string(map, "encryption_algorithm").filter(|name| !name.is_empty()) {
                Some(name) => Encryption::Algorithm(name),
                None => Encryption::CoreDefault,
            }
        };
        let socks5_port = if boolean(map, "enable_socks5") {
            map.get("socks5_port")
                .and_then(Value::as_i64)
                .and_then(|port| u16::try_from(port).ok())
        } else {
            None
        };
        Ok(Self {
            instance_id,
            network_name: string(map, "network_name").unwrap_or_default(),
            network_secret: Secret::new(string(map, "network_secret").unwrap_or_default()),
            hostname: string(map, "hostname").filter(|name| !name.is_empty()),
            dhcp: boolean(map, "dhcp"),
            virtual_ipv4: virtual_ipv4(map),
            networking,
            peers: strings(map, "peer_urls"),
            listeners: strings(map, "listener_urls"),
            proxy_cidrs: strings(map, "proxy_cidrs"),
            no_tun: boolean(map, "no_tun"),
            enable_exit_node: boolean(map, "enable_exit_node"),
            exit_nodes: strings(map, "exit_nodes"),
            socks5_port,
            flags: read_flags(map),
            relay_all_peer_rpc: boolean(map, "relay_all_peer_rpc"),
            private_mode: boolean(map, "enable_private_mode"),
            mtu: map
                .get("mtu")
                .and_then(Value::as_u64)
                .and_then(|mtu| u16::try_from(mtu).ok()),
            dev_name: string(map, "dev_name").filter(|name| !name.is_empty()),
            instance_recv_bps_limit: map.get("instance_recv_bps_limit").and_then(Value::as_u64),
            mapped_listeners: strings(map, "mapped_listeners"),
            manual_routes: boolean(map, "enable_manual_routes").then(|| strings(map, "routes")),
            relay_whitelist: boolean(map, "enable_relay_network_whitelist")
                .then(|| strings(map, "relay_network_whitelist")),
            vpn_portal: read_vpn_portal(map),
            port_forwards: read_forwards(map.get("port_forwards")).0,
            acl: read_acl(map.get("acl")),
            encryption,
            raw,
        })
    }

    /// Whether the server's `port_forwards` holds rows the form cannot read. They stay on the server value until
    /// [`NetworkConfigForm::port_forwards`] is edited; then the list written is exactly the edited one.
    pub fn forwards_unreadable(&self) -> bool {
        match &self.raw {
            Value::Object(map) => read_forwards(map.get("port_forwards")).1,
            _ => false,
        }
    }

    /// Whether the node refuses to relay other nodes' data (`disable_relay_data`), which the form does not edit.
    pub fn relay_data_disabled(&self) -> bool {
        match &self.raw {
            Value::Object(map) => boolean(map, "disable_relay_data"),
            _ => false,
        }
    }

    /// The JSON to send: the original with the form's fields written over it. Every other key is left exactly as
    /// the server sent it. It contains the network secret in clear.
    pub fn to_json(&self) -> Value {
        let mut map = match &self.raw {
            Value::Object(map) => map.clone(),
            _ => Map::new(),
        };
        put(
            &mut map,
            "instance_id",
            Value::String(self.instance_id.to_string()),
        );
        put(
            &mut map,
            "network_name",
            Value::String(self.network_name.clone()),
        );
        // A credential-mode node has no network secret (its key is in `secure_mode`): an empty secret over an
        // absent one must not invent an empty string.
        let had_secret = matches!(map.get("network_secret"), Some(Value::String(_)));
        if !self.network_secret.is_empty() || had_secret {
            put(
                &mut map,
                "network_secret",
                Value::String(self.network_secret.expose().to_owned()),
            );
        }
        put(
            &mut map,
            "hostname",
            optional_string(self.hostname.as_deref()),
        );
        put(&mut map, "dhcp", Value::Bool(self.dhcp));
        match self.virtual_ipv4 {
            Some(net) => {
                put(
                    &mut map,
                    "virtual_ipv4",
                    Value::String(net.addr().to_string()),
                );
                put(&mut map, "network_length", Value::from(net.prefix_len()));
            }
            None => {
                put(&mut map, "virtual_ipv4", Value::Null);
                put(&mut map, "network_length", Value::Null);
            }
        }
        let (method, public_server_url) = match &self.networking {
            Networking::PublicServer { url } => (0, Value::String(url.clone())),
            Networking::Manual => (1, Value::Null),
            Networking::Standalone => (2, Value::Null),
        };
        put(&mut map, "networking_method", Value::from(method));
        put(&mut map, "public_server_url", public_server_url);
        put(&mut map, "peer_urls", string_array(&self.peers));
        put(&mut map, "listener_urls", string_array(&self.listeners));
        put(&mut map, "proxy_cidrs", string_array(&self.proxy_cidrs));
        put(&mut map, "no_tun", Value::Bool(self.no_tun));
        put(
            &mut map,
            "enable_exit_node",
            Value::Bool(self.enable_exit_node),
        );
        put(&mut map, "exit_nodes", string_array(&self.exit_nodes));
        put(
            &mut map,
            "enable_socks5",
            Value::Bool(self.socks5_port.is_some()),
        );
        put(
            &mut map,
            "socks5_port",
            self.socks5_port.map_or(Value::Null, Value::from),
        );
        self.write_settings(&mut map);
        let (disable_encryption, algorithm) = match &self.encryption {
            Encryption::CoreDefault => (false, Value::Null),
            Encryption::Algorithm(name) => (false, Value::String(name.clone())),
            // The algorithm is irrelevant when off; keep what was there rather than invent a value.
            Encryption::Disabled => (
                true,
                map.get("encryption_algorithm")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
        };
        put(
            &mut map,
            "disable_encryption",
            Value::Bool(disable_encryption),
        );
        put(&mut map, "encryption_algorithm", algorithm);
        Value::Object(map)
    }

    /// Writes the keys of the settings beyond the connection basics. Keys of a switched-off setting (the routes of
    /// a disabled route list, the VPN portal's address) stay as they were: the server ignores them.
    fn write_settings(&self, map: &mut Map<String, Value>) {
        for flag in Flag::ALL {
            put(map, flag_key(flag), Value::Bool(flag.get(&self.flags)));
        }
        put(
            map,
            "relay_all_peer_rpc",
            Value::Bool(self.relay_all_peer_rpc),
        );
        put(map, "enable_private_mode", Value::Bool(self.private_mode));
        put(map, "mtu", self.mtu.map_or(Value::Null, Value::from));
        put(map, "dev_name", optional_string(self.dev_name.as_deref()));
        put(
            map,
            "instance_recv_bps_limit",
            self.instance_recv_bps_limit
                .map_or(Value::Null, Value::from),
        );
        put(
            map,
            "mapped_listeners",
            string_array(&self.mapped_listeners),
        );
        put(
            map,
            "enable_manual_routes",
            Value::Bool(self.manual_routes.is_some()),
        );
        if let Some(routes) = &self.manual_routes {
            put(map, "routes", string_array(routes));
        }
        put(
            map,
            "enable_relay_network_whitelist",
            Value::Bool(self.relay_whitelist.is_some()),
        );
        if let Some(names) = &self.relay_whitelist {
            put(map, "relay_network_whitelist", string_array(names));
        }
        put(
            map,
            "enable_vpn_portal",
            Value::Bool(self.vpn_portal.is_some()),
        );
        if let Some(portal) = &self.vpn_portal {
            put(
                map,
                "vpn_portal_listen_port",
                Value::from(portal.listen_port),
            );
            put(
                map,
                "vpn_portal_client_network_addr",
                Value::String(portal.client_cidr.addr().to_string()),
            );
            put(
                map,
                "vpn_portal_client_network_len",
                Value::from(portal.client_cidr.prefix_len()),
            );
        }
        // Forwards the form cannot read stay as the server sent them until the list is edited.
        if read_forwards(map.get("port_forwards")).0 != self.port_forwards {
            put(
                map,
                "port_forwards",
                Value::Array(self.port_forwards.iter().map(forward_to_json).collect()),
            );
        }
        match &self.acl {
            AclState::Unreadable => {}
            AclState::Off => {
                if read_acl(map.get("acl")) != AclState::Off {
                    put(map, "acl", Value::Null);
                }
            }
            AclState::On(config) => {
                let unchanged =
                    matches!(read_acl(map.get("acl")), AclState::On(current) if &current == config);
                if !unchanged && let Ok(acl) = serde_json::to_value(config.to_wire()) {
                    put(map, "acl", acl);
                }
            }
        }
    }
}

/// The JSON key of `flag`: its field name, except magic DNS (`enable_magic_dns`).
fn flag_key(flag: Flag) -> &'static str {
    match flag {
        Flag::MagicDns => "enable_magic_dns",
        other => other.id(),
    }
}

/// The switches of the config; a missing or null key is EasyTier's default for that switch.
fn read_flags(map: &Map<String, Value>) -> NodeFlags {
    let mut flags = NodeFlags::default();
    for flag in Flag::ALL {
        if let Some(on) = map.get(flag_key(flag)).and_then(Value::as_bool) {
            flag.set(&mut flags, on);
        }
    }
    flags
}

fn read_vpn_portal(map: &Map<String, Value>) -> Option<VpnPortal> {
    if !boolean(map, "enable_vpn_portal") {
        return None;
    }
    let addr: Ipv4Addr = string(map, "vpn_portal_client_network_addr")?
        .trim()
        .parse()
        .ok()?;
    let prefix = map
        .get("vpn_portal_client_network_len")
        .and_then(Value::as_u64)
        .and_then(|len| u8::try_from(len).ok())?;
    let listen_port = map
        .get("vpn_portal_listen_port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .unwrap_or(DEFAULT_VPN_PORTAL_PORT);
    Some(VpnPortal {
        client_cidr: Ipv4Net::new(addr, prefix).ok()?,
        listen_port,
    })
}

/// The readable rows of `port_forwards`, and whether some row (or the whole value) could not be read.
fn read_forwards(value: Option<&Value>) -> (Vec<PortForward>, bool) {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return (Vec::new(), false);
    };
    let Some(rows) = value.as_array() else {
        return (Vec::new(), true);
    };
    let mut forwards = Vec::with_capacity(rows.len());
    let mut unreadable = false;
    for row in rows {
        match forward_from_json(row) {
            Some(forward) => forwards.push(forward),
            None => unreadable = true,
        }
    }
    (forwards, unreadable)
}

fn forward_from_json(row: &Value) -> Option<PortForward> {
    let row = row.as_object()?;
    let address = |ip: &str, port: &str| -> Option<SocketAddr> {
        let ip: IpAddr = string(row, ip)?.trim().parse().ok()?;
        let port = u16::try_from(row.get(port)?.as_u64()?).ok()?;
        Some(SocketAddr::new(ip, port))
    };
    Some(PortForward {
        proto: Proto::parse(&string(row, "proto")?)?,
        bind: address("bind_ip", "bind_port")?,
        dst: address("dst_ip", "dst_port")?,
    })
}

fn forward_to_json(forward: &PortForward) -> Value {
    json!({
        "bind_ip": forward.bind.ip().to_string(),
        "bind_port": forward.bind.port(),
        "dst_ip": forward.dst.ip().to_string(),
        "dst_port": forward.dst.port(),
        "proto": forward.proto.name(),
    })
}

/// The ACL of the config. A missing or null `acl`, or one with no chains and no groups, is [`AclState::Off`].
fn read_acl(value: Option<&Value>) -> AclState {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return AclState::Off;
    };
    let Ok(acl) = serde_json::from_value::<wire::Acl>(value.clone()) else {
        return AclState::Unreadable;
    };
    match AclConfig::from_wire(&acl) {
        Ok(config)
            if config.acl.chains.is_empty()
                && config.acl.declared_groups.is_empty()
                && config.acl.joined_groups.is_empty() =>
        {
            AclState::Off
        }
        Ok(config) => AclState::On(config),
        Err(_) => AclState::Unreadable,
    }
}

fn put(map: &mut Map<String, Value>, key: &str, value: Value) {
    map.insert(key.to_owned(), value);
}

fn string(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn boolean(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn strings(map: &Map<String, Value>, key: &str) -> Vec<String> {
    map.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn optional_string(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |text| Value::String(text.to_owned()))
}

fn string_array(items: &[String]) -> Value {
    Value::Array(items.iter().cloned().map(Value::String).collect())
}

/// The static address with its prefix (`virtual_ipv4` plus `network_length`, 24 when the length is missing).
fn virtual_ipv4(map: &Map<String, Value>) -> Option<Ipv4Net> {
    let addr: Ipv4Addr = string(map, "virtual_ipv4")?.trim().parse().ok()?;
    let prefix = map
        .get("network_length")
        .and_then(Value::as_i64)
        .and_then(|length| u8::try_from(length).ok())
        .unwrap_or(DEFAULT_PREFIX_LEN);
    Ipv4Net::new(addr, prefix).ok()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn fixture(name: &str) -> Value {
        serde_json::from_slice(
            &std::fs::read(format!(
                "{}/tests/fixtures/easytier-2.6.4/{name}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap()
    }

    /// The keys the form writes.
    const OWNED: [&str; 55] = [
        "instance_id",
        "network_name",
        "network_secret",
        "hostname",
        "dhcp",
        "virtual_ipv4",
        "network_length",
        "networking_method",
        "public_server_url",
        "peer_urls",
        "listener_urls",
        "proxy_cidrs",
        "no_tun",
        "enable_exit_node",
        "exit_nodes",
        "enable_socks5",
        "socks5_port",
        "disable_encryption",
        "latency_first",
        "use_smoltcp",
        "disable_ipv6",
        "ipv6_public_addr_auto",
        "enable_kcp_proxy",
        "disable_kcp_input",
        "enable_quic_proxy",
        "disable_quic_input",
        "disable_p2p",
        "p2p_only",
        "lazy_p2p",
        "need_p2p",
        "bind_device",
        "multi_thread",
        "proxy_forward_by_system",
        "disable_tcp_hole_punching",
        "disable_udp_hole_punching",
        "disable_sym_hole_punching",
        "enable_udp_broadcast_relay",
        "disable_upnp",
        "enable_magic_dns",
        "relay_all_peer_rpc",
        "enable_private_mode",
        "mtu",
        "dev_name",
        "instance_recv_bps_limit",
        "mapped_listeners",
        "enable_manual_routes",
        "routes",
        "enable_relay_network_whitelist",
        "relay_network_whitelist",
        "enable_vpn_portal",
        "vpn_portal_listen_port",
        "vpn_portal_client_network_addr",
        "vpn_portal_client_network_len",
        "port_forwards",
        "acl",
    ];

    #[test]
    fn reads_the_fixture_config() {
        let form = NetworkConfigForm::from_json(fixture("web-get-config.json")).unwrap();
        assert_eq!(
            form.instance_id.to_string(),
            "00000000-0000-0000-0000-0000000000aa"
        );
        assert_eq!(form.network_name, "example-net");
        assert_eq!(
            form.network_secret,
            Secret::new("example-secret-do-not-use")
        );
        assert_eq!(form.hostname.as_deref(), Some("example-host-m"));
        assert!(!form.dhcp);
        assert_eq!(form.virtual_ipv4, "10.144.145.1/24".parse().ok());
        assert_eq!(form.networking, Networking::Manual);
        assert_eq!(form.listeners, vec!["tcp://127.0.0.1:41012".to_owned()]);
        assert!(form.no_tun);
        assert!(!form.enable_exit_node);
        assert_eq!(form.socks5_port, None);
        assert_eq!(form.encryption, Encryption::CoreDefault);
    }

    #[test]
    fn reads_a_parsed_toml_config() {
        let reply = fixture("web-parse-config.json");
        let form = NetworkConfigForm::from_json(reply["config"].clone()).unwrap();
        assert_eq!(form.virtual_ipv4, "10.144.144.2/24".parse().ok());
        assert_eq!(form.peers, vec!["tcp://127.0.0.1:41010".to_owned()]);
        assert_eq!(form.listeners, vec!["tcp://127.0.0.1:41011".to_owned()]);
        assert_eq!(form.networking, Networking::Manual);
    }

    #[test]
    fn a_save_without_edits_changes_no_key_the_form_does_not_own_and_is_stable() {
        let original = fixture("web-get-config.json");
        let form = NetworkConfigForm::from_json(original.clone()).unwrap();
        let saved = form.to_json();
        let (Value::Object(original), Value::Object(written)) = (&original, &saved) else {
            panic!("objects");
        };
        for (key, value) in original {
            if OWNED.contains(&key.as_str()) || key == "encryption_algorithm" {
                continue;
            }
            assert_eq!(written.get(key), Some(value), "{key}");
        }
        // Reading what was written and writing it again changes nothing.
        let again = NetworkConfigForm::from_json(saved.clone()).unwrap();
        assert_eq!(again.to_json(), saved);
        assert_eq!(again.flags, form.flags);
    }

    #[test]
    fn a_sparse_config_reads_easytiers_defaults() {
        let form = NetworkConfigForm::from_json(json!({"network_name": "n"})).unwrap();
        assert_eq!(form.flags, NodeFlags::default());
        assert!(form.flags.bind_device && form.flags.multi_thread);
        assert!(!form.private_mode && !form.relay_all_peer_rpc);
        assert_eq!(form.mtu, None);
        assert_eq!(form.dev_name, None);
        assert_eq!(form.instance_recv_bps_limit, None);
        assert!(form.mapped_listeners.is_empty());
        assert_eq!(form.manual_routes, None);
        assert_eq!(form.relay_whitelist, None);
        assert_eq!(form.vpn_portal, None);
        assert!(form.port_forwards.is_empty());
        assert_eq!(form.acl, AclState::Off);

        let sparse = NetworkConfigForm::from_json(fixture("web-get-config.json")).unwrap();
        assert_eq!(sparse.dev_name, None, "an empty interface name is none");
        assert_eq!(sparse.vpn_portal, None, "a null enable flag is off");
        assert_eq!(sparse.acl, AclState::Off);
    }

    fn full() -> Value {
        fixture("web-parse-config-full.json")["config"].clone()
    }

    #[test]
    fn the_full_config_reads_every_owned_field() {
        let form = NetworkConfigForm::from_json(full()).unwrap();
        let flags = &form.flags;
        assert!(flags.latency_first && flags.use_smoltcp && flags.disable_ipv6);
        assert!(flags.ipv6_public_addr_auto && flags.enable_kcp_proxy && flags.disable_kcp_input);
        assert!(flags.enable_quic_proxy && flags.disable_quic_input && flags.disable_p2p);
        assert!(flags.p2p_only && flags.lazy_p2p && flags.need_p2p);
        assert!(!flags.bind_device && !flags.multi_thread);
        assert!(flags.proxy_forward_by_system && flags.disable_tcp_hole_punching);
        assert!(flags.disable_udp_hole_punching && flags.disable_sym_hole_punching);
        assert!(flags.enable_udp_broadcast_relay && flags.disable_upnp && flags.accept_dns);
        assert!(form.relay_all_peer_rpc && form.private_mode);
        assert_eq!(form.mtu, Some(1300));
        assert_eq!(form.dev_name.as_deref(), Some("et0"));
        assert_eq!(form.instance_recv_bps_limit, Some(1_048_576));
        assert_eq!(form.mapped_listeners, vec!["tcp://203.0.113.5:11223"]);
        assert_eq!(form.manual_routes, Some(vec!["192.168.0.0/16".to_owned()]));
        assert_eq!(
            form.relay_whitelist,
            Some(vec!["team-*".to_owned(), "other".to_owned()])
        );
        assert_eq!(
            form.vpn_portal,
            Some(VpnPortal {
                client_cidr: "10.14.14.0/24".parse().unwrap(),
                listen_port: 22022,
            })
        );
        assert_eq!(form.port_forwards.len(), 1);
        assert_eq!(
            form.port_forwards[0].to_string(),
            "tcp 127.0.0.1:8080 10.144.144.2:80"
        );
        assert!(!form.forwards_unreadable());
        let AclState::On(acl) = &form.acl else {
            panic!("the ACL is readable");
        };
        assert_eq!(acl.acl.chains.len(), 1);
        assert_eq!(acl.acl.declared_groups, vec!["admins"]);
    }

    #[test]
    fn the_full_config_round_trips_with_every_value_kept() {
        let original = full();
        let saved = NetworkConfigForm::from_json(original.clone())
            .unwrap()
            .to_json();
        let (Value::Object(original), Value::Object(saved)) = (&original, &saved) else {
            panic!("objects");
        };
        for (key, value) in original {
            assert_eq!(saved.get(key), Some(value), "{key}");
        }
        assert_eq!(saved.len(), original.len());
    }

    /// The full config with `edit` applied to its form, as written.
    fn edited(edit: impl FnOnce(&mut NetworkConfigForm)) -> Map<String, Value> {
        let mut form = NetworkConfigForm::from_json(full()).unwrap();
        edit(&mut form);
        let Value::Object(saved) = form.to_json() else {
            panic!("an object");
        };
        saved
    }

    /// The keys whose written value differs from the full fixture's.
    fn changed_keys(saved: &Map<String, Value>) -> Vec<String> {
        let Value::Object(original) = full() else {
            panic!("an object");
        };
        let mut keys: Vec<String> = saved
            .iter()
            .filter(|(key, value)| original.get(*key) != Some(*value))
            .map(|(key, _)| key.clone())
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn editing_a_switch_writes_only_its_key() {
        let saved = edited(|form| form.flags.accept_dns = false);
        assert_eq!(changed_keys(&saved), vec!["enable_magic_dns"]);
        let saved = edited(|form| form.flags.disable_ipv6 = false);
        assert_eq!(changed_keys(&saved), vec!["disable_ipv6"]);
        let saved = edited(|form| form.flags.bind_device = true);
        assert_eq!(changed_keys(&saved), vec!["bind_device"]);
        let saved = edited(|form| form.flags.ipv6_public_addr_auto = false);
        assert_eq!(changed_keys(&saved), vec!["ipv6_public_addr_auto"]);
    }

    #[test]
    fn editing_a_scalar_writes_only_its_key() {
        let saved = edited(|form| form.relay_all_peer_rpc = false);
        assert_eq!(changed_keys(&saved), vec!["relay_all_peer_rpc"]);
        let saved = edited(|form| form.private_mode = false);
        assert_eq!(changed_keys(&saved), vec!["enable_private_mode"]);
        let saved = edited(|form| form.mtu = None);
        assert_eq!(changed_keys(&saved), vec!["mtu"]);
        assert_eq!(saved["mtu"], Value::Null);
        let saved = edited(|form| form.mtu = Some(1400));
        assert_eq!(saved["mtu"], 1400);
        let saved = edited(|form| form.dev_name = None);
        assert_eq!(changed_keys(&saved), vec!["dev_name"]);
        let saved = edited(|form| form.instance_recv_bps_limit = None);
        assert_eq!(changed_keys(&saved), vec!["instance_recv_bps_limit"]);
        let saved = edited(|form| form.instance_recv_bps_limit = Some(5000));
        assert_eq!(saved["instance_recv_bps_limit"], 5000);
        let saved = edited(|form| form.mapped_listeners.clear());
        assert_eq!(changed_keys(&saved), vec!["mapped_listeners"]);
    }

    #[test]
    fn lists_and_portals_write_their_enable_flag_and_values() {
        let saved = edited(|form| form.manual_routes = Some(Vec::new()));
        assert_eq!(changed_keys(&saved), vec!["routes"]);
        assert_eq!(saved["enable_manual_routes"], true);
        let saved = edited(|form| form.manual_routes = None);
        assert_eq!(changed_keys(&saved), vec!["enable_manual_routes"]);
        assert_eq!(saved["routes"], json!(["192.168.0.0/16"]));

        let saved = edited(|form| form.relay_whitelist = Some(vec!["lab".into()]));
        assert_eq!(changed_keys(&saved), vec!["relay_network_whitelist"]);
        let saved = edited(|form| form.relay_whitelist = None);
        assert_eq!(changed_keys(&saved), vec!["enable_relay_network_whitelist"]);

        let saved = edited(|form| {
            form.vpn_portal = Some(VpnPortal {
                client_cidr: "10.99.0.0/16".parse().unwrap(),
                listen_port: 4000,
            });
        });
        assert_eq!(
            changed_keys(&saved),
            vec![
                "vpn_portal_client_network_addr",
                "vpn_portal_client_network_len",
                "vpn_portal_listen_port"
            ]
        );
        assert_eq!(saved["vpn_portal_client_network_addr"], "10.99.0.0");
        assert_eq!(saved["vpn_portal_client_network_len"], 16);
        let saved = edited(|form| form.vpn_portal = None);
        assert_eq!(changed_keys(&saved), vec!["enable_vpn_portal"]);
    }

    #[test]
    fn port_forwards_map_to_the_servers_rows() {
        let saved = edited(|form| {
            form.port_forwards.push(PortForward {
                proto: Proto::Udp,
                bind: "0.0.0.0:5353".parse().unwrap(),
                dst: "10.144.144.3:53".parse().unwrap(),
            });
        });
        assert_eq!(changed_keys(&saved), vec!["port_forwards"]);
        assert_eq!(
            saved["port_forwards"][1],
            json!({"bind_ip": "0.0.0.0", "bind_port": 5353, "dst_ip": "10.144.144.3", "dst_port": 53, "proto": "udp"})
        );
        let saved = edited(|form| form.port_forwards.clear());
        assert_eq!(saved["port_forwards"], json!([]));
    }

    #[test]
    fn unreadable_forwards_stay_until_the_list_is_edited() {
        let mut raw = full();
        raw["port_forwards"] = json!([
            {"bind_ip": "127.0.0.1", "bind_port": 8080, "dst_ip": "10.144.144.2", "dst_port": 80, "proto": "tcp"},
            {"bind_ip": "not-an-ip", "bind_port": 1, "dst_ip": "10.144.144.2", "dst_port": 80, "proto": "tcp"},
            {"bind_ip": "127.0.0.1", "bind_port": 70000, "dst_ip": "10.144.144.2", "dst_port": 80, "proto": "tcp"},
            {"bind_ip": "127.0.0.1", "bind_port": 1, "dst_ip": "10.144.144.2", "dst_port": 80, "proto": "sctp", "extra": 1}
        ]);
        let mut form = NetworkConfigForm::from_json(raw.clone()).unwrap();
        assert_eq!(form.port_forwards.len(), 1);
        assert!(form.forwards_unreadable());
        assert_eq!(form.to_json()["port_forwards"], raw["port_forwards"]);
        form.port_forwards.clear();
        assert_eq!(form.to_json()["port_forwards"], json!([]));

        raw["port_forwards"] = json!("garbage");
        let form = NetworkConfigForm::from_json(raw.clone()).unwrap();
        assert!(form.forwards_unreadable());
        assert_eq!(form.to_json()["port_forwards"], "garbage");
    }

    #[test]
    fn the_acl_is_kept_off_or_rewritten_by_its_state() {
        // On and unchanged: the server's value, byte for byte (the chain has no `group` key order to lose).
        let original = full();
        assert_eq!(
            NetworkConfigForm::from_json(original.clone())
                .unwrap()
                .to_json()["acl"],
            original["acl"]
        );
        // On and edited: rewritten in EasyTier's shape.
        let saved = edited(|form| {
            if let AclState::On(config) = &mut form.acl {
                config.acl.chains[0].enabled = false;
            }
        });
        assert_eq!(changed_keys(&saved), vec!["acl"]);
        assert_eq!(saved["acl"]["acl_v1"]["chains"][0]["enabled"], false);
        assert_eq!(
            saved["acl"]["acl_v1"]["group"]["declares"][0]["group_secret"],
            "example-group-secret"
        );
        // Off over a readable ACL: null, so the core runs without one.
        let saved = edited(|form| form.acl = AclState::Off);
        assert_eq!(saved["acl"], Value::Null);
        // Off over nothing: untouched, not invented.
        let mut empty = NetworkConfigForm::new(Uuid::nil(), "n", Secret::new("s"));
        empty.acl = AclState::Off;
        assert!(empty.to_json().get("acl").is_none());
        // Off over an empty ACL: untouched.
        let raw = json!({"acl": {"acl_v1": {"chains": []}}});
        assert_eq!(
            NetworkConfigForm::from_json(raw.clone()).unwrap().to_json()["acl"],
            raw["acl"]
        );
        // On over nothing: written.
        let mut form = NetworkConfigForm::new(Uuid::nil(), "n", Secret::new("s"));
        let AclState::On(config) = NetworkConfigForm::from_json(full()).unwrap().acl else {
            panic!("readable");
        };
        form.acl = AclState::On(config);
        assert_eq!(form.to_json()["acl"], original["acl"]);
    }

    #[test]
    fn an_unreadable_acl_is_kept_whatever_else_is_edited() {
        let mut raw = full();
        raw["acl"] = json!({"acl_v1": {"chains": [{"name": "x", "chain_type": 99, "rules": []}]}});
        let mut form = NetworkConfigForm::from_json(raw.clone()).unwrap();
        assert_eq!(form.acl, AclState::Unreadable);
        form.network_name = "renamed".into();
        assert_eq!(form.to_json()["acl"], raw["acl"]);

        raw["acl"] = json!("not an object");
        let form = NetworkConfigForm::from_json(raw.clone()).unwrap();
        assert_eq!(form.acl, AclState::Unreadable);
        assert_eq!(form.to_json()["acl"], raw["acl"]);
    }

    #[test]
    fn a_new_config_has_private_mode_on_and_easytiers_switches() {
        let form = NetworkConfigForm::new(Uuid::nil(), "n", Secret::new("s"));
        assert!(form.private_mode && !form.relay_all_peer_rpc);
        assert_eq!(form.flags, NodeFlags::default());
        let saved = form.to_json();
        assert_eq!(saved["enable_private_mode"], true);
        assert_eq!(saved["bind_device"], true);
        assert_eq!(saved["multi_thread"], true);
        assert_eq!(saved["enable_manual_routes"], false);
        assert_eq!(saved["enable_vpn_portal"], false);
        assert_eq!(saved["enable_relay_network_whitelist"], false);
        assert_eq!(saved["mapped_listeners"], json!([]));
    }

    #[test]
    fn unknown_fields_survive_an_edit() {
        let mut original = fixture("web-get-config.json");
        let Value::Object(map) = &mut original else {
            panic!("object");
        };
        map.insert(
            "field_from_a_newer_release".into(),
            json!({"nested": [1, 2, 3]}),
        );
        map.insert(
            "port_forwards".into(),
            json!([{"bind_ip": "0.0.0.0", "bind_port": 8080, "dst_ip": "10.144.145.2", "dst_port": 80, "proto": "tcp"}]),
        );
        map.insert(
            "secure_mode".into(),
            json!({"enabled": true, "local_private_key": "example-secret-do-not-use"}),
        );
        map.insert("acl".into(), json!({"acl_v1": {"chains": []}}));

        let mut form = NetworkConfigForm::from_json(original.clone()).unwrap();
        form.network_name = "renamed".into();
        form.peers.push("tcp://192.0.2.7:11010".into());
        form.flags.latency_first = true;
        form.socks5_port = Some(1080);
        form.encryption = Encryption::Algorithm("chacha20".into());
        let saved = form.to_json();

        assert_eq!(saved["network_name"], "renamed");
        assert_eq!(saved["peer_urls"], json!(["tcp://192.0.2.7:11010"]));
        assert_eq!(saved["latency_first"], true);
        assert_eq!(saved["enable_socks5"], true);
        assert_eq!(saved["socks5_port"], 1080);
        assert_eq!(saved["encryption_algorithm"], "chacha20");
        assert_eq!(saved["disable_encryption"], false);

        let (Value::Object(original), Value::Object(saved)) = (&original, &saved) else {
            panic!("objects");
        };
        for (key, value) in original {
            if OWNED.contains(&key.as_str()) || key == "encryption_algorithm" {
                continue;
            }
            assert_eq!(saved.get(key), Some(value), "{key} must be untouched");
        }
        assert_eq!(
            saved["field_from_a_newer_release"],
            json!({"nested": [1, 2, 3]})
        );
    }

    #[test]
    fn dhcp_clears_the_static_address() {
        let mut form = NetworkConfigForm::from_json(fixture("web-get-config.json")).unwrap();
        form.dhcp = true;
        form.virtual_ipv4 = None;
        let saved = form.to_json();
        assert_eq!(saved["dhcp"], true);
        assert_eq!(saved["virtual_ipv4"], Value::Null);
        assert_eq!(saved["network_length"], Value::Null);
    }

    #[test]
    fn a_credential_mode_config_keeps_its_absent_secret() {
        let original = json!({
            "instance_id": "00000000-0000-0000-0000-0000000000aa",
            "network_name": "cred-net",
            "network_secret": null,
            "secure_mode": {"enabled": true, "local_private_key": "example-secret-do-not-use"}
        });
        let form = NetworkConfigForm::from_json(original).unwrap();
        assert!(form.network_secret.is_empty());
        let saved = form.to_json();
        assert_eq!(saved["network_secret"], Value::Null);
        assert_eq!(
            saved["secure_mode"]["local_private_key"],
            "example-secret-do-not-use"
        );
    }

    #[test]
    fn a_new_config_has_safe_defaults_and_round_trips() {
        let id: Uuid = "00000000-0000-0000-0000-0000000000bb".parse().unwrap();
        let form = NetworkConfigForm::new(id, "fresh", Secret::new("example-secret-do-not-use"));
        let saved = form.to_json();
        assert_eq!(saved["instance_id"], "00000000-0000-0000-0000-0000000000bb");
        assert_eq!(saved["dhcp"], true);
        assert_eq!(saved["networking_method"], 1);
        assert_eq!(saved["disable_encryption"], false);
        assert_eq!(saved["encryption_algorithm"], "aes-gcm");
        assert_eq!(saved["enable_exit_node"], false);
        assert_eq!(saved["listener_urls"], json!([]));
        let again = NetworkConfigForm::from_json(saved).unwrap();
        assert_eq!(again.instance_id, id);
        assert_eq!(again.encryption, Encryption::Algorithm("aes-gcm".into()));
        assert_eq!(again.network_secret, form.network_secret);
    }

    #[test]
    fn networking_methods_round_trip() {
        let id = Uuid::nil();
        let mut form = NetworkConfigForm::new(id, "n", Secret::new("s"));
        for networking in [
            Networking::Manual,
            Networking::Standalone,
            Networking::PublicServer {
                url: "tcp://public.example.com:11010".into(),
            },
        ] {
            form.networking = networking.clone();
            let again = NetworkConfigForm::from_json(form.to_json()).unwrap();
            assert_eq!(again.networking, networking);
        }
        // A missing method is what the server reads as PublicServer.
        let bare = NetworkConfigForm::from_json(json!({})).unwrap();
        assert_eq!(
            bare.networking,
            Networking::PublicServer { url: String::new() }
        );
    }

    #[test]
    fn disabled_encryption_is_explicit() {
        let mut form = NetworkConfigForm::new(Uuid::nil(), "n", Secret::new("s"));
        form.encryption = Encryption::Disabled;
        let saved = form.to_json();
        assert_eq!(saved["disable_encryption"], true);
        assert_eq!(
            NetworkConfigForm::from_json(saved).unwrap().encryption,
            Encryption::Disabled
        );
    }

    #[test]
    fn a_non_object_is_a_decode_error_and_debug_hides_secrets() {
        assert!(matches!(
            NetworkConfigForm::from_json(json!([1])),
            Err(RemoteError::Decode(_))
        ));
        let form = NetworkConfigForm::from_json(fixture("web-get-config.json")).unwrap();
        let text = format!("{form:?}");
        assert!(!text.contains("example-secret-do-not-use"), "{text}");
        assert!(text.contains("Secret(***)"));
    }
}
