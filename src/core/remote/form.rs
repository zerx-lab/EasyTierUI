//! The editable part of a network's config on a remote machine.
//!
//! easytier-web's `NetworkConfig` has more than sixty fields and gains new ones with each release. The console
//! edits a subset; [`NetworkConfigForm`] keeps the config's original JSON next to that subset and patches only the
//! keys it owns when it is sent back, so fields it does not know (a newer release's, ACLs, port forwards, VPN
//! portal) survive an edit untouched.

use std::{fmt, net::Ipv4Addr};

use ipnet::Ipv4Net;
use serde_json::{Map, Value};
use uuid::Uuid;

use super::{error::RemoteError, ids::new_instance_id};
use crate::core::secret::Secret;

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
    /// Whether routes are chosen by latency first.
    pub latency_first: bool,
    /// Whether direct (P2P) links are disabled.
    pub disable_p2p: bool,
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
    /// A new network config: DHCP, manual peers (none yet), no listeners, a TUN, no exit node, no SOCKS5 portal and
    /// encryption on with `aes-gcm`.
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
            latency_first: false,
            disable_p2p: false,
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
            latency_first: boolean(map, "latency_first"),
            disable_p2p: boolean(map, "disable_p2p"),
            encryption,
            raw,
        })
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
        put(&mut map, "latency_first", Value::Bool(self.latency_first));
        put(&mut map, "disable_p2p", Value::Bool(self.disable_p2p));
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
    const OWNED: [&str; 20] = [
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
        "latency_first",
        "disable_p2p",
        "disable_encryption",
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
    fn a_save_without_edits_changes_nothing_but_nulls_the_form_owns() {
        let original = fixture("web-get-config.json");
        let form = NetworkConfigForm::from_json(original.clone()).unwrap();
        let saved = form.to_json();
        let (Value::Object(original), Value::Object(saved)) = (&original, &saved) else {
            panic!("objects");
        };
        for (key, value) in original {
            if key == "enable_socks5" || key == "socks5_port" || key == "encryption_algorithm" {
                continue;
            }
            assert_eq!(saved.get(key), Some(value), "{key}");
        }
        assert_eq!(saved.len(), original.len());
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
        form.latency_first = true;
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
