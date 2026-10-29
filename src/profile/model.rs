//! The network profile: everything a person configures about one EasyTier network on this device, as domain types
//! with safe defaults. No secret lives here (the network secret and the control-plane address rest in the keychain
//! under [`ProfileId::network_secret_account`] / [`ProfileId::config_server_account`]).

use std::{fmt, net::IpAddr, str::FromStr};

use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};

use super::error::ProfileError;
use crate::core::{
    acl::Acl,
    net::{self, ListenerUrl, NetError, PeerUrl},
    node::{InstanceKey, Privilege},
    settings::{MappedListener, NodeFlags, PortForward, VpnPortal},
};

/// The longest display name or network name, in characters.
pub const MAX_NAME_CHARS: usize = 64;
/// The longest hostname, in characters (EasyTier itself keeps 32).
pub const MAX_HOSTNAME_CHARS: usize = 32;
/// The longest network device name (Linux limits interface names to 15 bytes).
pub const MAX_DEV_NAME_CHARS: usize = 15;
/// Valid MTU values: IPv6's minimum up to Ethernet's.
pub const MTU_RANGE: (u16, u16) = (1280, 1500);
/// The MTU EasyTier uses when none is set.
pub const DEFAULT_MTU: u16 = 1380;
/// The most peers, listeners, subnets, forwards, exit nodes or whitelist entries a profile may carry.
pub const MAX_LIST_ENTRIES: usize = 64;

/// Parses `a.b.c.d/len` as a route: any prefix length from 0 to 32 (a default route is `0.0.0.0/0`). Whether the
/// address is the network's own is checked by [`NetworkProfile::validate`].
///
/// # Errors
///
/// [`NetError::BadCidr`] under `field`.
pub fn parse_route(field: &str, text: &str) -> Result<Ipv4Net, NetError> {
    Ipv4Net::from_str(text.trim()).map_err(|_| NetError::BadCidr {
        field: field.to_owned(),
    })
}

/// A person's text made fit for a name field: surrounding whitespace and control characters removed, cut to `max`
/// characters (never bytes, so CJK names stay whole characters).
pub fn clean_name(text: &str, max: usize) -> String {
    text.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect()
}

/// A profile's identity: 32 lowercase hexadecimal characters (a UUID without hyphens), generated once and never
/// derived from user text. It names the profile's keychain entries, its instance key and its config's `instance_id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProfileId(String);

impl ProfileId {
    /// A new random id (UUID v4).
    ///
    /// # Errors
    ///
    /// [`ProfileError::NoRandomSource`] on the web build or when the OS random source fails.
    pub fn generate() -> Result<Self, ProfileError> {
        let uuid = random_uuid()?;
        Ok(Self(uuid.simple().to_string()))
    }

    /// The id written as `text`.
    ///
    /// # Errors
    ///
    /// [`ProfileError::BadId`] unless it is exactly 32 characters of `0-9a-f`.
    pub fn parse(text: &str) -> Result<Self, ProfileError> {
        let valid =
            text.len() == 32 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(ProfileError::BadId)
        }
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The keychain account of the network secret.
    pub fn network_secret_account(&self) -> String {
        format!("profile/{}/network-secret", self.0)
    }

    /// The keychain account of the control-plane address of a managed profile.
    pub fn config_server_account(&self) -> String {
        format!("profile/{}/config-server", self.0)
    }

    /// The keychain account of the ACL group secrets: one value holding every declared group's secret
    /// ([`GroupSecrets::to_keychain`]).
    pub fn acl_group_secrets_account(&self) -> String {
        format!("profile/{}/acl-group-secrets", self.0)
    }

    /// The instance key (`p-<id>`) the node backends file this profile's config, logs and service under.
    ///
    /// # Errors
    ///
    /// [`ProfileError::BadId`] if the id is not a plain id (cannot happen for a parsed one).
    pub fn instance_key(&self) -> Result<InstanceKey, ProfileError> {
        InstanceKey::for_profile(&self.0).ok_or(ProfileError::BadId)
    }

    /// The stable `instance_id` UUID (hyphenated) of the profile's config file: the id itself, hyphenated.
    ///
    /// # Errors
    ///
    /// [`ProfileError::BadId`] if the id is not a UUID's 32 digits (cannot happen for a parsed one).
    pub fn instance_uuid(&self) -> Result<String, ProfileError> {
        uuid::Uuid::try_parse(&self.0)
            .map(|uuid| uuid.hyphenated().to_string())
            .map_err(|_| ProfileError::BadId)
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ProfileId {
    type Error = ProfileError;

    fn try_from(text: String) -> Result<Self, ProfileError> {
        Self::parse(&text)
    }
}

impl From<ProfileId> for String {
    fn from(id: ProfileId) -> Self {
        id.0
    }
}

/// A fresh random UUID (version 4) from the OS CSPRNG.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn random_uuid() -> Result<uuid::Uuid, ProfileError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| ProfileError::NoRandomSource)?;
    Ok(uuid::Builder::from_random_bytes(bytes).into_uuid())
}

/// The web build has no secure random source the app can reach.
#[cfg(target_arch = "wasm32")]
pub(crate) fn random_uuid() -> Result<uuid::Uuid, ProfileError> {
    Err(ProfileError::NoRandomSource)
}

/// Where the node's network config comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSource {
    /// This profile, rendered to EasyTier's config file.
    #[default]
    Local,
    /// A control plane pushes the config: the node enrolls with the control-plane address (kept as a secret under
    /// [`ProfileId::config_server_account`]) and the installation's machine id. Everything else in the profile but
    /// the display name and hostname is unused.
    Managed,
}

/// How the node gets its virtual address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IpMode {
    /// Pick a free address in the network automatically (the default for a joiner).
    #[default]
    Dhcp,
    /// This address and prefix (`10.144.144.1/24`).
    Static(Ipv4Net),
    /// No virtual address: a relay-only node. Imports can produce it; an editor need not offer it.
    None,
}

/// How peer traffic is protected. Both ends of a link must agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Encryption {
    /// AES-128-GCM: the default.
    #[default]
    #[serde(rename = "aes-gcm")]
    AesGcm,
    /// AES-256-GCM.
    #[serde(rename = "aes-256-gcm")]
    Aes256Gcm,
    /// ChaCha20.
    #[serde(rename = "chacha20")]
    ChaCha20,
    /// XOR with a key derived from the secret: obfuscation that stops casual inspection and nothing else.
    /// Never call it encryption.
    #[serde(rename = "xor")]
    Xor,
    /// Traffic between peers is not protected at all.
    #[serde(rename = "off")]
    Off,
}

impl Encryption {
    /// Every choice, safest first.
    pub const ALL: [Self; 5] = [
        Self::AesGcm,
        Self::Aes256Gcm,
        Self::ChaCha20,
        Self::Xor,
        Self::Off,
    ];

    /// The algorithm name in the config file; `None` when encryption is off.
    pub const fn algorithm(self) -> Option<&'static str> {
        match self {
            Self::AesGcm => Some("aes-gcm"),
            Self::Aes256Gcm => Some("aes-256-gcm"),
            Self::ChaCha20 => Some("chacha20"),
            Self::Xor => Some("xor"),
            Self::Off => None,
        }
    }

    /// The choice named `algorithm` in a config file.
    pub fn from_algorithm(algorithm: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.algorithm() == Some(algorithm))
    }

    /// Whether this is real encryption (authenticated, not obfuscation and not off).
    pub const fn is_encryption(self) -> bool {
        matches!(self, Self::AesGcm | Self::Aes256Gcm | Self::ChaCha20)
    }
}

/// A SOCKS5 proxy this node serves into the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Socks5 {
    /// The address it binds: loopback unless an import said otherwise.
    pub host: IpAddr,
    /// Its port.
    pub port: u16,
}

impl Socks5 {
    /// A proxy on this device only (`127.0.0.1:<port>`).
    pub const fn local(port: u16) -> Self {
        Self {
            host: IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            port,
        }
    }
}

/// Which other networks this node relays for.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelayWhitelist {
    /// Any network (EasyTier's default, `*`); with private mode on, foreign networks still need the secret.
    #[default]
    Any,
    /// Only networks matching these names (`*` wildcards allowed); an empty list relays for no other network.
    Only(Vec<String>),
}

/// One subnet behind this node that peers may reach through it, optionally presented to them under another network
/// of the same size (`192.168.50.0/24->10.50.0.0/24`). Stored and written in the config as that text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SubnetProxy {
    /// The real subnet.
    pub cidr: Ipv4Net,
    /// The subnet peers see it as; `None` shows it as it is.
    pub mapped: Option<Ipv4Net>,
}

impl SubnetProxy {
    /// A subnet shared as it is.
    pub const fn plain(cidr: Ipv4Net) -> Self {
        Self { cidr, mapped: None }
    }

    /// Reads one line of the editors' list: `192.168.50.0/24` or `192.168.50.0/24->10.50.0.0/24`. Both parts must be
    /// usable subnets; whether the two have the same size is checked by [`NetworkProfile::validate`].
    ///
    /// # Errors
    ///
    /// The [`NetError`] of [`net::parse_ipv4_subnet`] under `field` (`{field}.mapped` for the second part).
    pub fn parse_line(field: &str, line: &str) -> Result<Self, NetError> {
        let (real, mapped) = match line.split_once("->") {
            Some((real, mapped)) => (real, Some(mapped)),
            None => (line, None),
        };
        Ok(Self {
            cidr: net::parse_ipv4_subnet(field, real.trim())?,
            mapped: mapped
                .map(|text| net::parse_ipv4_subnet(&format!("{field}.mapped"), text.trim()))
                .transpose()?,
        })
    }
}

impl fmt::Display for SubnetProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.mapped {
            Some(mapped) => write!(f, "{}->{mapped}", self.cidr),
            None => write!(f, "{}", self.cidr),
        }
    }
}

impl TryFrom<String> for SubnetProxy {
    type Error = NetError;

    /// Reads stored text; only the shape is checked here, [`NetworkProfile::validate`] checks the networks.
    fn try_from(text: String) -> Result<Self, NetError> {
        let bad = || NetError::BadCidr {
            field: "subnet_proxy".to_owned(),
        };
        let (real, mapped) = match text.split_once("->") {
            Some((real, mapped)) => (real, Some(mapped)),
            None => (text.as_str(), None),
        };
        Ok(Self {
            cidr: Ipv4Net::from_str(real.trim()).map_err(|_| bad())?,
            mapped: mapped
                .map(|mapped| Ipv4Net::from_str(mapped.trim()).map_err(|_| bad()))
                .transpose()?,
        })
    }
}

impl From<SubnetProxy> for String {
    fn from(subnet: SubnetProxy) -> Self {
        subnet.to_string()
    }
}

/// One EasyTier network on this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkProfile {
    /// The profile's identity.
    pub id: ProfileId,
    /// The name shown in the app.
    pub name: String,
    /// Local (edited here) or managed (pushed by a control plane).
    #[serde(default)]
    pub source: ConfigSource,
    /// The EasyTier network name (the network's identity together with its secret). Unused when managed.
    #[serde(default)]
    pub network_name: String,
    /// The name this node reports to its peers; the device's own when `None`.
    #[serde(default)]
    pub hostname: Option<String>,
    /// How the node gets its virtual address.
    #[serde(default)]
    pub ip: IpMode,
    /// Nodes to connect to.
    #[serde(default)]
    pub peers: Vec<PeerUrl>,
    /// Addresses this node accepts connections on; none unless the person asked.
    #[serde(default)]
    pub listeners: Vec<ListenerUrl>,
    /// Whether the node creates a network interface (needs administrator rights); off runs without one (`no_tun`).
    #[serde(default = "yes")]
    pub tun: bool,
    /// How peer traffic is protected.
    #[serde(default)]
    pub encryption: Encryption,
    /// The MTU; EasyTier's default (1380) when `None`.
    #[serde(default)]
    pub mtu: Option<u16>,
    /// The core's behavior switches (routing, NAT traversal, KCP / QUIC, system integration). Stored flattened
    /// beside the other fields, so profiles saved before they existed keep loading.
    #[serde(flatten)]
    pub flags: NodeFlags,
    /// Mapped listeners: public addresses of this node's listeners announced to peers.
    #[serde(default)]
    pub mapped_listeners: Vec<MappedListener>,
    /// The most bytes per second this instance receives; unlimited when `None`.
    #[serde(default)]
    pub instance_recv_bps_limit: Option<u64>,
    /// Subnets behind this node that other peers may reach through it (subnet proxy). Off unless listed.
    #[serde(default)]
    pub subnet_proxy: Vec<SubnetProxy>,
    /// Manual routes: `Some` turns manual routing on (an empty list then means no routes at all), `None` leaves the
    /// routes to EasyTier.
    #[serde(default)]
    pub manual_routes: Option<Vec<Ipv4Net>>,
    /// The WireGuard VPN portal; off unless set.
    #[serde(default)]
    pub vpn_portal: Option<VpnPortal>,
    /// Access control; none unless set. Its group secrets rest in the keychain
    /// ([`ProfileId::acl_group_secrets_account`]).
    #[serde(default)]
    pub acl: Option<Acl>,
    /// Whether this node lets peers route their internet traffic through it (an exit node).
    #[serde(default)]
    pub offers_exit_node: bool,
    /// Virtual addresses of exit nodes this node sends its traffic through.
    #[serde(default)]
    pub exit_nodes: Vec<IpAddr>,
    /// A SOCKS5 proxy into the network; off unless set.
    #[serde(default)]
    pub socks5: Option<Socks5>,
    /// Local ports forwarded into the network.
    #[serde(default)]
    pub port_forwards: Vec<PortForward>,
    /// Relay every peer's control traffic, not only the local network's.
    #[serde(default)]
    pub relay_all_peer_rpc: bool,
    /// Refuse peers of other networks unless they know this network's secret. On by default.
    #[serde(default = "yes")]
    pub private_mode: bool,
    /// Which other networks to relay for.
    #[serde(default)]
    pub relay_whitelist: RelayWhitelist,
    /// The network interface's name (`et0`); the system's choice when `None`.
    #[serde(default)]
    pub dev_name: Option<String>,
}

/// Serde default for the settings that are on unless turned off.
const fn yes() -> bool {
    true
}

impl NetworkProfile {
    /// A local profile that joins `network_name` with the safe defaults: DHCP, no listeners, a network interface,
    /// AES-GCM, private mode, nothing exposed.
    pub fn new(id: ProfileId, name: impl Into<String>, network_name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            source: ConfigSource::Local,
            network_name: network_name.into(),
            hostname: None,
            ip: IpMode::Dhcp,
            peers: Vec::new(),
            listeners: Vec::new(),
            tun: true,
            encryption: Encryption::AesGcm,
            mtu: None,
            flags: NodeFlags::default(),
            mapped_listeners: Vec::new(),
            instance_recv_bps_limit: None,
            manual_routes: None,
            vpn_portal: None,
            acl: None,
            subnet_proxy: Vec::new(),
            offers_exit_node: false,
            exit_nodes: Vec::new(),
            socks5: None,
            port_forwards: Vec::new(),
            relay_all_peer_rpc: false,
            private_mode: true,
            relay_whitelist: RelayWhitelist::Any,
            dev_name: None,
        }
    }

    /// A profile whose config a control plane pushes; its enrollment address is stored as a secret.
    pub fn managed(id: ProfileId, name: impl Into<String>) -> Self {
        Self {
            source: ConfigSource::Managed,
            ..Self::new(id, name, String::new())
        }
    }

    /// Whether a control plane owns the config.
    pub fn is_managed(&self) -> bool {
        self.source == ConfigSource::Managed
    }

    /// What the node needs to be allowed to do: a network interface needs the OS service route
    /// ([`Privilege::System`]); managed nodes always run as the user.
    pub fn privilege(&self) -> Privilege {
        if self.tun && !self.is_managed() {
            Privilege::System
        } else {
            Privilege::User
        }
    }

    /// The keychain account of the secret this profile needs: the network secret, or for a managed profile the
    /// control-plane address.
    pub fn secret_account(&self) -> String {
        match self.source {
            ConfigSource::Local => self.id.network_secret_account(),
            ConfigSource::Managed => self.id.config_server_account(),
        }
    }

    /// Everything the profile does that widens who or what can reach this device or the network, for the editor's
    /// and the import preview's warnings.
    pub fn exposures(&self) -> Vec<Exposure> {
        let mut found = Vec::new();
        for listener in &self.listeners {
            if listener.exposes_all_interfaces() {
                found.push(Exposure::ListenerOnAllInterfaces(listener.clone()));
            }
        }
        if self.offers_exit_node {
            found.push(Exposure::OffersExitNode);
        }
        for node in &self.exit_nodes {
            found.push(Exposure::UsesExitNode(*node));
        }
        for subnet in &self.subnet_proxy {
            found.push(Exposure::SharesSubnet(subnet.cidr));
        }
        if let Some(portal) = self.vpn_portal {
            found.push(Exposure::VpnPortal(portal));
        }
        if self.flags.proxy_forward_by_system {
            found.push(Exposure::ForwardBySystem);
        }
        for forward in &self.port_forwards {
            found.push(Exposure::ForwardsPort {
                forward: *forward,
                public: !forward.bind.ip().is_loopback(),
            });
        }
        if let Some(socks5) = &self.socks5 {
            found.push(Exposure::Socks5 {
                public: !socks5.host.is_loopback(),
            });
        }
        match self.encryption {
            Encryption::Off => found.push(Exposure::EncryptionOff),
            Encryption::Xor => found.push(Exposure::ObfuscationOnly),
            Encryption::AesGcm | Encryption::Aes256Gcm | Encryption::ChaCha20 => {}
        }
        if !self.private_mode {
            found.push(Exposure::PrivateModeOff);
        }
        if self.relay_all_peer_rpc {
            found.push(Exposure::RelaysAllPeerRpc);
        }
        let wide_whitelist = match &self.relay_whitelist {
            RelayWhitelist::Any => !self.private_mode,
            RelayWhitelist::Only(names) => names.iter().any(|name| name == "*"),
        };
        if wide_whitelist {
            found.push(Exposure::WideRelayWhitelist);
        }
        found
    }
}

/// Something a profile does that makes the device or network more reachable, or weakens protection. Each is off by
/// default and explained where it is turned on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exposure {
    /// A listener on `0.0.0.0` / `[::]`: reachable from every network this device is on.
    ListenerOnAllInterfaces(ListenerUrl),
    /// Peers may route their internet traffic through this device.
    OffersExitNode,
    /// This node sends its traffic through that exit node.
    UsesExitNode(IpAddr),
    /// Peers may reach this subnet through this device.
    SharesSubnet(Ipv4Net),
    /// A local port is forwarded into the network; `public` when it listens beyond this device.
    ForwardsPort {
        /// The forward.
        forward: PortForward,
        /// Whether the bind address is not loopback.
        public: bool,
    },
    /// A SOCKS5 proxy into the network; `public` when it listens beyond this device.
    Socks5 {
        /// Whether the bind address is not loopback.
        public: bool,
    },
    /// Peer traffic is not encrypted.
    EncryptionOff,
    /// Peer traffic is only obfuscated (XOR), not encrypted.
    ObfuscationOnly,
    /// Peers of other networks are accepted without the secret.
    PrivateModeOff,
    /// Every peer's control traffic is relayed.
    RelaysAllPeerRpc,
    /// Relays for any network without needing its secret.
    WideRelayWhitelist,
    /// WireGuard clients may join the network through this node, from every interface it has.
    VpnPortal(VpnPortal),
    /// Subnet-proxy traffic is forwarded by the OS kernel, so the system's firewall and routing apply to it.
    ForwardBySystem,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::settings::Proto;

    fn id() -> ProfileId {
        ProfileId::parse("0123456789abcdef0123456789abcdef").unwrap()
    }

    #[test]
    fn ids_are_32_lowercase_hex() {
        assert!(ProfileId::parse("0123456789abcdef0123456789abcdef").is_ok());
        assert_eq!(
            ProfileId::parse("0123456789ABCDEF0123456789abcdef"),
            Err(ProfileError::BadId)
        );
        assert_eq!(ProfileId::parse("0123"), Err(ProfileError::BadId));
        assert_eq!(
            ProfileId::parse("../../etc/passwd../../etc/passwd"),
            Err(ProfileError::BadId)
        );
        assert_eq!(ProfileId::parse(""), Err(ProfileError::BadId));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn generated_ids_are_valid_and_distinct() {
        let a = ProfileId::generate().unwrap();
        let b = ProfileId::generate().unwrap();
        assert_ne!(a, b);
        assert!(ProfileId::parse(a.as_str()).is_ok());
        assert!(a.instance_key().is_ok());
    }

    #[test]
    fn ids_name_their_keychain_entries_instance_and_uuid() {
        let id = id();
        assert_eq!(
            id.network_secret_account(),
            "profile/0123456789abcdef0123456789abcdef/network-secret"
        );
        assert_eq!(
            id.config_server_account(),
            "profile/0123456789abcdef0123456789abcdef/config-server"
        );
        assert_eq!(
            id.instance_key().unwrap().as_str(),
            "p-0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            id.instance_uuid().unwrap(),
            "01234567-89ab-cdef-0123-456789abcdef"
        );
    }

    #[test]
    fn ids_validate_on_deserialize() {
        assert!(serde_json::from_str::<ProfileId>("\"nope\"").is_err());
        let json = serde_json::to_string(&id()).unwrap();
        assert_eq!(serde_json::from_str::<ProfileId>(&json).unwrap(), id());
    }

    #[test]
    fn new_profiles_start_safe() {
        let profile = NetworkProfile::new(id(), "Home", "home-net");
        assert_eq!(profile.ip, IpMode::Dhcp);
        assert!(profile.listeners.is_empty());
        assert!(profile.peers.is_empty());
        assert_eq!(profile.encryption, Encryption::AesGcm);
        assert!(profile.private_mode);
        assert!(profile.socks5.is_none());
        assert!(profile.subnet_proxy.is_empty());
        assert!(!profile.offers_exit_node);
        assert!(profile.exposures().is_empty());
        assert_eq!(profile.privilege(), Privilege::System);
        assert_eq!(profile.secret_account(), id().network_secret_account());
    }

    #[test]
    fn managed_profiles_run_as_the_user_with_their_own_secret_account() {
        let profile = NetworkProfile::managed(id(), "Office");
        assert!(profile.is_managed());
        assert_eq!(profile.privilege(), Privilege::User);
        assert_eq!(profile.secret_account(), id().config_server_account());
    }

    #[test]
    fn names_are_cleaned_by_characters() {
        assert_eq!(clean_name("  家庭网络\n", 64), "家庭网络");
        assert_eq!(clean_name("家庭网络家庭网络", 4), "家庭网络");
        assert_eq!(clean_name("a\u{7}b", 64), "ab");
        assert_eq!(clean_name("😀😀😀", 2), "😀😀");
    }

    #[test]
    fn exposures_list_every_widening_choice() {
        let mut profile = NetworkProfile::new(id(), "Home", "home-net");
        profile.listeners = vec![
            "tcp://0.0.0.0:11010".parse().unwrap(),
            "udp://127.0.0.1:11010".parse().unwrap(),
        ];
        profile.offers_exit_node = true;
        profile.exit_nodes = vec!["10.144.144.9".parse().unwrap()];
        profile.subnet_proxy = vec![SubnetProxy::plain("192.168.50.0/24".parse().unwrap())];
        profile.vpn_portal = Some(VpnPortal {
            client_cidr: "10.14.14.0/24".parse().unwrap(),
            listen_port: 22022,
        });
        profile.flags.proxy_forward_by_system = true;
        profile.port_forwards = vec![PortForward {
            proto: Proto::Tcp,
            bind: "0.0.0.0:8080".parse().unwrap(),
            dst: "10.144.144.2:80".parse().unwrap(),
        }];
        profile.socks5 = Some(Socks5::local(1080));
        profile.encryption = Encryption::Xor;
        profile.private_mode = false;
        profile.relay_all_peer_rpc = true;
        let found = profile.exposures();
        assert!(
            matches!(found.first(), Some(Exposure::ListenerOnAllInterfaces(l)) if l.as_str() == "tcp://0.0.0.0:11010")
        );
        assert!(found.contains(&Exposure::OffersExitNode));
        assert!(found.contains(&Exposure::UsesExitNode("10.144.144.9".parse().unwrap())));
        assert!(found.contains(&Exposure::SharesSubnet("192.168.50.0/24".parse().unwrap())));
        assert!(
            found
                .iter()
                .any(|e| matches!(e, Exposure::ForwardsPort { public: true, .. }))
        );
        assert!(found.contains(&Exposure::Socks5 { public: false }));
        assert!(found.contains(&Exposure::ObfuscationOnly));
        assert!(found.contains(&Exposure::PrivateModeOff));
        assert!(found.contains(&Exposure::RelaysAllPeerRpc));
        assert!(found.contains(&Exposure::WideRelayWhitelist));
        assert!(found.contains(&Exposure::ForwardBySystem));
        assert!(
            found
                .iter()
                .any(|e| matches!(e, Exposure::VpnPortal(p) if p.listen_port == 22022))
        );
        assert_eq!(
            found
                .iter()
                .filter(|e| matches!(e, Exposure::ListenerOnAllInterfaces(_)))
                .count(),
            1
        );
    }

    #[test]
    fn encryption_names_round_trip() {
        for choice in Encryption::ALL {
            if let Some(name) = choice.algorithm() {
                assert_eq!(Encryption::from_algorithm(name), Some(choice));
            }
        }
        assert_eq!(Encryption::from_algorithm("rot13"), None);
        assert!(Encryption::AesGcm.is_encryption());
        assert!(!Encryption::Xor.is_encryption());
        assert!(!Encryption::Off.is_encryption());
    }

    #[test]
    fn profiles_round_trip_through_json_and_tolerate_unknown_and_missing_fields() {
        let mut profile = NetworkProfile::new(id(), "Home", "home-net");
        profile.ip = IpMode::Static("10.144.144.1/24".parse().unwrap());
        profile.peers = vec!["tcp://203.0.113.7:11010".parse().unwrap()];
        profile.encryption = Encryption::Aes256Gcm;
        profile.relay_whitelist = RelayWhitelist::Only(vec!["team-*".to_owned()]);
        let json = serde_json::to_string(&profile).unwrap();
        assert_eq!(
            serde_json::from_str::<NetworkProfile>(&json).unwrap(),
            profile
        );

        let minimal = r#"{"id":"0123456789abcdef0123456789abcdef","name":"Home","network_name":"home-net","future_field":[1,2]}"#;
        let loaded: NetworkProfile = serde_json::from_str(minimal).unwrap();
        assert_eq!(loaded, NetworkProfile::new(id(), "Home", "home-net"));
    }

    #[test]
    fn a_profile_with_every_field_round_trips_through_json() {
        let (profile, _) = crate::profile::testing::full_profile().unwrap();
        let json = serde_json::to_string(&profile).unwrap();
        assert_eq!(
            serde_json::from_str::<NetworkProfile>(&json).unwrap(),
            profile
        );
        // Group secrets are never part of the stored profile.
        assert!(!json.contains("example-group-secret"));
        assert!(json.contains("192.168.50.0/24->10.50.0.0/24"));
    }

    #[test]
    fn subnet_lines_read_and_write_the_arrow_form() {
        let plain = SubnetProxy::parse_line("s", " 192.168.50.0/24 ").unwrap();
        assert_eq!(
            plain,
            SubnetProxy::plain("192.168.50.0/24".parse().unwrap())
        );
        assert_eq!(plain.to_string(), "192.168.50.0/24");
        let mapped = SubnetProxy::parse_line("s", "192.168.50.0/24 -> 10.50.0.0/24").unwrap();
        assert_eq!(mapped.to_string(), "192.168.50.0/24->10.50.0.0/24");
        assert!(SubnetProxy::parse_line("s", "192.168.50.7/24").is_err());
        assert!(SubnetProxy::parse_line("s", "192.168.50.0/24->nope").is_err());
        assert!(serde_json::from_str::<SubnetProxy>("\"nope\"").is_err());
    }
}
