//! Importing an EasyTier TOML config: untrusted text in, a typed profile plus a preview out. Nothing is started,
//! stored or applied here; the person sees the [`ImportPreview`] first (`rule://easytier-config-security`).
//!
//! Absent keys mean what they mean to EasyTier (its defaults, not this app's): a config without `private_mode` is
//! one with private mode off, and the preview says so. Keys this model does not carry are not applied and are listed
//! in [`Imported::ignored_keys`] so the person knows the config was only partly understood (an ignored `acl` or
//! `peer_public_key` matters).

use std::net::IpAddr;

use ipnet::Ipv4Net;
use toml::{Table, Value};

use super::{
    error::ProfileError,
    model::{
        ConfigSource, Encryption, Exposure, IpMode, MAX_NAME_CHARS, MTU_RANGE, NetworkProfile,
        PortForward, ProfileId, Proto, RelayWhitelist, Socks5, clean_name,
    },
};
use crate::core::{
    net::{self, ListenerUrl, NetError, PeerUrl},
    secret::Secret,
};

/// The largest config accepted: real ones are a few hundred bytes.
pub const MAX_IMPORT_BYTES: usize = 256 * 1024;

/// Why a config was rejected. Syntax errors carry the line, never the text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    /// More than [`MAX_IMPORT_BYTES`].
    #[error("the config is larger than {max} bytes")]
    TooLarge {
        /// The limit.
        max: usize,
    },
    /// Not valid TOML.
    #[error("the config is not valid TOML{}", .line.map(|n| format!(" (line {n})")).unwrap_or_default())]
    Syntax {
        /// The line, if known.
        line: Option<usize>,
        /// The parser's description (it never quotes the input).
        message: String,
    },
    /// A key holds the wrong kind of value.
    #[error("{field}: expected {expected}")]
    WrongType {
        /// The key.
        field: String,
        /// What it should hold.
        expected: &'static str,
    },
    /// A required key is missing.
    #[error("{field}: missing")]
    Missing {
        /// The key.
        field: String,
    },
    /// A value EasyTier or this app does not know (an unknown algorithm or protocol).
    #[error("{field}: unsupported value")]
    UnknownValue {
        /// The key.
        field: String,
    },
    /// The values parsed but do not make a valid profile; the inner error names the field.
    #[error(transparent)]
    Profile(#[from] ProfileError),
}

impl From<NetError> for ImportError {
    fn from(error: NetError) -> Self {
        Self::Profile(ProfileError::Net(error))
    }
}

impl ImportError {
    /// The key the problem is in, if it has one.
    pub fn field(&self) -> Option<&str> {
        match self {
            Self::WrongType { field, .. }
            | Self::Missing { field }
            | Self::UnknownValue { field } => Some(field),
            Self::Profile(error) => error.field(),
            Self::TooLarge { .. } | Self::Syntax { .. } => None,
        }
    }
}

/// A listener in the preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewListener {
    /// The listener.
    pub url: ListenerUrl,
    /// The port it will bind (shown even when the URL has none).
    pub port: u16,
    /// Whether it binds every interface.
    pub all_interfaces: bool,
}

/// A port forward in the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewForward {
    /// The forward.
    pub forward: PortForward,
    /// Whether it listens beyond this device.
    pub public: bool,
}

/// A SOCKS5 proxy in the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewSocks5 {
    /// The proxy.
    pub proxy: Socks5,
    /// Whether it listens beyond this device.
    pub public: bool,
}

/// What applying an imported config would do, for the person to read before agreeing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreview {
    /// The network it joins.
    pub network_name: String,
    /// Whether the config carried a network secret.
    pub has_secret: bool,
    /// How the node gets its address.
    pub ip: IpMode,
    /// The hostname it reports.
    pub hostname: Option<String>,
    /// Peers it connects to.
    pub peers: Vec<PeerUrl>,
    /// Addresses it listens on.
    pub listeners: Vec<PreviewListener>,
    /// The config had no `listeners` key, so EasyTier would have opened its default listeners on every interface;
    /// the import has none instead.
    pub default_listeners_dropped: bool,
    /// Peers may route through this node to the internet.
    pub offers_exit_node: bool,
    /// Exit nodes this node sends its traffic through.
    pub exit_nodes: Vec<IpAddr>,
    /// Subnets it shares with the network.
    pub proxied_subnets: Vec<Ipv4Net>,
    /// Ports it forwards.
    pub port_forwards: Vec<PreviewForward>,
    /// Its SOCKS5 proxy.
    pub socks5: Option<PreviewSocks5>,
    /// How peer traffic is protected.
    pub encryption: Encryption,
    /// Whether peers of other networks need the secret.
    pub private_mode: bool,
    /// Whether it creates a network interface (administrator rights).
    pub tun: bool,
    /// Every exposure-widening item, for warnings.
    pub exposures: Vec<Exposure>,
}

impl ImportPreview {
    /// The preview of `profile`.
    pub fn of(profile: &NetworkProfile, has_secret: bool, default_listeners_dropped: bool) -> Self {
        Self {
            network_name: profile.network_name.clone(),
            has_secret,
            ip: profile.ip,
            hostname: profile.hostname.clone(),
            peers: profile.peers.clone(),
            listeners: profile
                .listeners
                .iter()
                .map(|url| PreviewListener {
                    url: url.clone(),
                    port: url.effective_port(),
                    all_interfaces: url.exposes_all_interfaces(),
                })
                .collect(),
            default_listeners_dropped,
            offers_exit_node: profile.offers_exit_node,
            exit_nodes: profile.exit_nodes.clone(),
            proxied_subnets: profile.subnet_proxy.clone(),
            port_forwards: profile
                .port_forwards
                .iter()
                .map(|forward| PreviewForward {
                    forward: *forward,
                    public: !forward.bind.ip().is_loopback(),
                })
                .collect(),
            socks5: profile.socks5.map(|proxy| PreviewSocks5 {
                proxy,
                public: !proxy.host.is_loopback(),
            }),
            encryption: profile.encryption,
            private_mode: profile.private_mode,
            tun: profile.tun,
            exposures: profile.exposures(),
        }
    }
}

/// A parsed import, not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// The profile, with a fresh id and the network name as its display name.
    pub profile: NetworkProfile,
    /// The network secret, if the config had a non-empty one.
    pub secret: Option<Secret>,
    /// What applying it would do.
    pub preview: ImportPreview,
    /// Keys the config had that this app did not apply (`acl`, `secure_mode`, `peer[0].peer_public_key`,
    /// `flags.enable_kcp_proxy`, ...), in file order within each table.
    pub ignored_keys: Vec<String>,
}

/// Imports `text` as a new profile with a fresh id.
///
/// # Errors
///
/// [`ImportError`] naming the key; [`ProfileError::NoRandomSource`] (as [`ImportError::Profile`]) on the web build.
pub fn import_toml(text: &str) -> Result<Imported, ImportError> {
    import_toml_with_id(text, ProfileId::generate()?)
}

/// Imports `text` as a profile with the id `id`.
///
/// # Errors
///
/// [`ImportError::TooLarge`], [`ImportError::Syntax`], [`ImportError::WrongType`], [`ImportError::Missing`],
/// [`ImportError::UnknownValue`], or [`ImportError::Profile`] for a value that is not valid (bad scheme, CIDR, port,
/// over-long name, ...), each naming the key.
pub fn import_toml_with_id(text: &str, id: ProfileId) -> Result<Imported, ImportError> {
    if text.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge {
            max: MAX_IMPORT_BYTES,
        });
    }
    let root: Table = toml::from_str(text).map_err(|error| ImportError::Syntax {
        line: error
            .span()
            .and_then(|span| text.get(..span.start))
            .map(|before| before.matches('\n').count().saturating_add(1)),
        message: error.message().chars().take(200).collect(),
    })?;
    let mut ignored = Vec::new();
    let mut profile = NetworkProfile::new(id, "", "");
    profile.source = ConfigSource::Local;

    report_unknown(&root, "", &TOP_LEVEL_KEYS, &mut ignored);

    profile.hostname = str_of(&root, "", "hostname")?.map(str::to_owned);

    let dhcp = bool_of(&root, "", "dhcp")?.unwrap_or(false);
    let ipv4 = str_of(&root, "", "ipv4")?;
    profile.ip = match (dhcp, ipv4) {
        (true, Some(_)) => {
            ignored.push("ipv4".to_owned());
            IpMode::Dhcp
        }
        (true, None) => IpMode::Dhcp,
        (false, Some(text)) => IpMode::Static(net::parse_ipv4_inet("ipv4", text)?),
        (false, None) => IpMode::None,
    };

    let mut default_listeners_dropped = false;
    match strings_of(&root, "", "listeners")? {
        Some(listeners) => {
            for (index, text) in listeners.into_iter().enumerate() {
                profile.listeners.push(ListenerUrl::parse_config(
                    &format!("listeners[{index}]"),
                    text,
                )?);
            }
        }
        None => default_listeners_dropped = true,
    }

    if let Some(nodes) = strings_of(&root, "", "exit_nodes")? {
        for (index, text) in nodes.into_iter().enumerate() {
            profile
                .exit_nodes
                .push(net::parse_ip_addr(&format!("exit_nodes[{index}]"), text)?);
        }
    }

    if let Some(text) = str_of(&root, "", "socks5_proxy")? {
        profile.socks5 = Some(parse_socks5(text)?);
    }

    let mut secret = None;
    if let Some(identity) = table_of(&root, "", "network_identity")? {
        report_unknown(
            identity,
            "network_identity",
            &["network_name", "network_secret"],
            &mut ignored,
        );
        let name = str_of(identity, "network_identity", "network_name")?;
        profile.network_name = name.unwrap_or("default").to_owned();
        secret = str_of(identity, "network_identity", "network_secret")?
            .filter(|value| !value.is_empty())
            .map(Secret::new);
    } else {
        // EasyTier's own default network.
        "default".clone_into(&mut profile.network_name);
    }

    for (index, entry) in tables_of(&root, "", "peer")?.into_iter().enumerate() {
        let prefix = format!("peer[{index}]");
        report_unknown(entry, &prefix, &["uri"], &mut ignored);
        let uri = str_of(entry, &prefix, "uri")?.ok_or_else(|| ImportError::Missing {
            field: format!("{prefix}.uri"),
        })?;
        profile
            .peers
            .push(PeerUrl::parse(&format!("{prefix}.uri"), uri)?);
    }

    for (index, entry) in tables_of(&root, "", "proxy_network")?
        .into_iter()
        .enumerate()
    {
        let prefix = format!("proxy_network[{index}]");
        report_unknown(entry, &prefix, &["cidr"], &mut ignored);
        let cidr = str_of(entry, &prefix, "cidr")?.ok_or_else(|| ImportError::Missing {
            field: format!("{prefix}.cidr"),
        })?;
        profile
            .subnet_proxy
            .push(net::parse_ipv4_subnet(&format!("{prefix}.cidr"), cidr)?);
    }

    for (index, entry) in tables_of(&root, "", "port_forward")?
        .into_iter()
        .enumerate()
    {
        let prefix = format!("port_forward[{index}]");
        report_unknown(
            entry,
            &prefix,
            &["bind_addr", "dst_addr", "proto"],
            &mut ignored,
        );
        let required = |key: &str| {
            str_of(entry, &prefix, key)?.ok_or_else(|| ImportError::Missing {
                field: format!("{prefix}.{key}"),
            })
        };
        let bind = net::parse_socket_addr(&format!("{prefix}.bind_addr"), required("bind_addr")?)?;
        let dst = net::parse_socket_addr(&format!("{prefix}.dst_addr"), required("dst_addr")?)?;
        let proto = match required("proto")?.to_ascii_lowercase().as_str() {
            "tcp" => Proto::Tcp,
            "udp" => Proto::Udp,
            _ => {
                return Err(ImportError::UnknownValue {
                    field: format!("{prefix}.proto"),
                });
            }
        };
        profile.port_forwards.push(PortForward { proto, bind, dst });
    }

    apply_flags(&root, &mut profile, &mut ignored)?;

    // The display name starts as the network name; the person renames it. A blank or over-long network name is
    // rejected below under its own field, not under "name".
    let name = clean_name(&profile.network_name, MAX_NAME_CHARS);
    profile.name = if name.is_empty() {
        "?".to_owned()
    } else {
        name
    };
    profile.validate()?;

    let preview = ImportPreview::of(&profile, secret.is_some(), default_listeners_dropped);
    Ok(Imported {
        profile,
        secret,
        preview,
        ignored_keys: ignored,
    })
}

/// The top-level keys this importer reads. `instance_name` and `instance_id` are read and dropped on purpose: the
/// import gets its own identity.
const TOP_LEVEL_KEYS: [&str; 13] = [
    "instance_name",
    "instance_id",
    "hostname",
    "ipv4",
    "dhcp",
    "listeners",
    "exit_nodes",
    "socks5_proxy",
    "network_identity",
    "peer",
    "proxy_network",
    "port_forward",
    "flags",
];

/// The `[flags]` keys this importer reads.
const FLAG_KEYS: [&str; 13] = [
    "no_tun",
    "enable_encryption",
    "encryption_algorithm",
    "private_mode",
    "enable_exit_node",
    "relay_all_peer_rpc",
    "relay_network_whitelist",
    "mtu",
    "latency_first",
    "disable_p2p",
    "accept_dns",
    "multi_thread",
    "dev_name",
];

fn apply_flags(
    root: &Table,
    profile: &mut NetworkProfile,
    ignored: &mut Vec<String>,
) -> Result<(), ImportError> {
    let Some(flags) = table_of(root, "", "flags")? else {
        // No `[flags]`: every flag has EasyTier's default.
        profile.private_mode = false;
        return Ok(());
    };
    report_unknown(flags, "flags", &FLAG_KEYS, ignored);
    let flag = |key: &str, default: bool| -> Result<bool, ImportError> {
        Ok(bool_of(flags, "flags", key)?.unwrap_or(default))
    };
    profile.tun = !flag("no_tun", false)?;
    profile.private_mode = flag("private_mode", false)?;
    profile.offers_exit_node = flag("enable_exit_node", false)?;
    profile.relay_all_peer_rpc = flag("relay_all_peer_rpc", false)?;
    profile.latency_first = flag("latency_first", false)?;
    profile.disable_p2p = flag("disable_p2p", false)?;
    profile.accept_dns = flag("accept_dns", false)?;
    profile.multi_thread = flag("multi_thread", true)?;

    profile.encryption = if flag("enable_encryption", true)? {
        match str_of(flags, "flags", "encryption_algorithm")? {
            None | Some("") => Encryption::AesGcm,
            Some(name) => Encryption::from_algorithm(name)
                .filter(|choice| choice.algorithm().is_some())
                .ok_or_else(|| ImportError::UnknownValue {
                    field: "flags.encryption_algorithm".to_owned(),
                })?,
        }
    } else {
        Encryption::Off
    };

    if let Some(mtu) = int_of(flags, "flags", "mtu")? {
        profile.mtu = Some(u16::try_from(mtu).map_err(|_| {
            ImportError::Profile(ProfileError::OutOfRange {
                field: "flags.mtu".to_owned(),
                min: u32::from(MTU_RANGE.0),
                max: u32::from(MTU_RANGE.1),
            })
        })?);
    }

    profile.relay_whitelist = match str_of(flags, "flags", "relay_network_whitelist")? {
        None | Some("*") => RelayWhitelist::Any,
        Some(list) => RelayWhitelist::Only(list.split_whitespace().map(str::to_owned).collect()),
    };

    profile.dev_name = str_of(flags, "flags", "dev_name")?
        .filter(|name| !name.is_empty())
        .map(str::to_owned);
    Ok(())
}

/// `socks5://ip:port`.
fn parse_socks5(text: &str) -> Result<Socks5, NetError> {
    let field = "socks5_proxy";
    let Some((scheme, rest)) = text.trim().split_once("://") else {
        return Err(NetError::Malformed {
            field: field.to_owned(),
        });
    };
    if !scheme.eq_ignore_ascii_case("socks5") {
        return Err(NetError::UnknownScheme {
            field: field.to_owned(),
            scheme: scheme.chars().take(16).collect(),
        });
    }
    let addr = net::parse_socket_addr(field, rest.trim_end_matches('/'))?;
    Ok(Socks5 {
        host: addr.ip(),
        port: addr.port(),
    })
}

fn join(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

fn wrong(prefix: &str, key: &str, expected: &'static str) -> ImportError {
    ImportError::WrongType {
        field: join(prefix, key),
        expected,
    }
}

/// Notes every key of `table` that is not in `known`.
fn report_unknown(table: &Table, prefix: &str, known: &[&str], ignored: &mut Vec<String>) {
    for key in table.keys() {
        if !known.contains(&key.as_str()) {
            ignored.push(join(prefix, key));
        }
    }
}

fn str_of<'a>(table: &'a Table, prefix: &str, key: &str) -> Result<Option<&'a str>, ImportError> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(wrong(prefix, key, "text")),
    }
}

fn bool_of(table: &Table, prefix: &str, key: &str) -> Result<Option<bool>, ImportError> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::Boolean(value)) => Ok(Some(*value)),
        Some(_) => Err(wrong(prefix, key, "true or false")),
    }
}

fn int_of(table: &Table, prefix: &str, key: &str) -> Result<Option<i64>, ImportError> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::Integer(value)) => Ok(Some(*value)),
        Some(_) => Err(wrong(prefix, key, "a whole number")),
    }
}

fn table_of<'a>(
    table: &'a Table,
    prefix: &str,
    key: &str,
) -> Result<Option<&'a Table>, ImportError> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::Table(inner)) => Ok(Some(inner)),
        Some(_) => Err(wrong(prefix, key, "a table")),
    }
}

/// A list of tables (`[[peer]]`); absent is an empty list.
fn tables_of<'a>(table: &'a Table, prefix: &str, key: &str) -> Result<Vec<&'a Table>, ImportError> {
    match table.get(key) {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, item)| match item {
                Value::Table(inner) => Ok(inner),
                _ => Err(ImportError::WrongType {
                    field: format!("{}[{index}]", join(prefix, key)),
                    expected: "a table",
                }),
            })
            .collect(),
        Some(_) => Err(wrong(prefix, key, "a list of tables")),
    }
}

/// A list of strings; `None` when the key is absent (an empty list is `Some`).
fn strings_of<'a>(
    table: &'a Table,
    prefix: &str,
    key: &str,
) -> Result<Option<Vec<&'a str>>, ImportError> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, item)| match item {
                Value::String(text) => Ok(text.as_str()),
                _ => Err(ImportError::WrongType {
                    field: format!("{}[{index}]", join(prefix, key)),
                    expected: "text",
                }),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(wrong(prefix, key, "a list of text")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::model::ProfileId;

    const ID: &str = "0123456789abcdef0123456789abcdef";

    fn id() -> ProfileId {
        ProfileId::parse(ID).unwrap()
    }

    fn secret() -> Secret {
        Secret::new("example-secret-do-not-use")
    }

    fn base() -> NetworkProfile {
        NetworkProfile::new(id(), "example-net", "example-net")
    }

    /// Every shape the renderer produces must come back equal.
    fn round_trip(profile: &NetworkProfile) {
        let text = profile.render(&secret()).unwrap();
        let imported = import_toml_with_id(text.expose(), id()).unwrap();
        assert_eq!(&imported.profile, profile);
        assert_eq!(imported.secret, Some(secret()));
        assert!(
            imported.ignored_keys.is_empty(),
            "{:?}",
            imported.ignored_keys
        );
        assert!(!imported.preview.default_listeners_dropped);
    }

    #[test]
    fn rendered_profiles_import_equal() {
        round_trip(&base());

        let mut p = base();
        p.ip = IpMode::Static("10.144.144.1/24".parse().unwrap());
        p.hostname = Some("家用路由器".to_owned());
        p.tun = false;
        round_trip(&p);

        let mut p = base();
        p.ip = IpMode::None;
        p.peers = vec![
            "tcp://203.0.113.7:11010".parse().unwrap(),
            "wss://relay.example.com".parse().unwrap(),
            "txt://peers.example.com".parse().unwrap(),
        ];
        p.listeners = vec![
            "tcp://0.0.0.0:11010".parse().unwrap(),
            "udp://[::]:11010".parse().unwrap(),
            "ws://127.0.0.1".parse().unwrap(),
        ];
        round_trip(&p);

        let mut p = base();
        p.subnet_proxy = vec!["192.168.50.0/24".parse().unwrap()];
        p.offers_exit_node = true;
        p.exit_nodes = vec!["10.144.144.9".parse().unwrap()];
        p.socks5 = Some(Socks5::local(1080));
        p.port_forwards = vec![
            PortForward {
                proto: Proto::Tcp,
                bind: "127.0.0.1:41200".parse().unwrap(),
                dst: "10.144.144.1:41201".parse().unwrap(),
            },
            PortForward {
                proto: Proto::Udp,
                bind: "[::1]:5353".parse().unwrap(),
                dst: "10.144.144.1:53".parse().unwrap(),
            },
        ];
        round_trip(&p);

        for encryption in Encryption::ALL {
            let mut p = base();
            p.encryption = encryption;
            round_trip(&p);
        }

        let mut p = base();
        p.mtu = Some(1400);
        p.latency_first = true;
        p.disable_p2p = true;
        p.accept_dns = true;
        p.multi_thread = false;
        p.private_mode = false;
        p.relay_all_peer_rpc = true;
        p.relay_whitelist = RelayWhitelist::Only(vec!["team-*".to_owned(), "lab".to_owned()]);
        p.dev_name = Some("et0".to_owned());
        round_trip(&p);

        let mut p = base();
        p.relay_whitelist = RelayWhitelist::Only(Vec::new());
        round_trip(&p);
    }

    #[test]
    fn imports_get_their_own_identity_and_a_display_name() {
        let text = base().render(&secret()).unwrap();
        let a = import_toml(text.expose()).unwrap();
        let b = import_toml(text.expose()).unwrap();
        assert_ne!(a.profile.id, b.profile.id);
        assert_ne!(a.profile.id, id());
        assert_eq!(a.profile.name, "example-net");
    }

    #[test]
    fn the_captured_node_config_previews_what_it_would_do() {
        let text = include_str!("../../tests/fixtures/easytier-2.6.4/node-a.toml");
        let imported = import_toml_with_id(text, id()).unwrap();
        let preview = &imported.preview;
        assert_eq!(preview.network_name, "example-net");
        assert!(preview.has_secret);
        assert_eq!(
            preview.ip,
            IpMode::Static("10.144.144.1/24".parse().unwrap())
        );
        assert_eq!(preview.hostname.as_deref(), Some("example-host-a"));
        assert_eq!(preview.listeners.len(), 1);
        assert_eq!(preview.listeners[0].port, 41010);
        assert!(!preview.listeners[0].all_interfaces);
        assert_eq!(
            preview.proxied_subnets,
            vec!["192.168.50.0/24".parse::<Ipv4Net>().unwrap()]
        );
        assert_eq!(preview.port_forwards.len(), 1);
        assert!(!preview.port_forwards[0].public);
        assert_eq!(preview.encryption, Encryption::AesGcm);
        // `[flags]` is empty, so EasyTier's defaults apply: private mode is off and a TUN is created.
        assert!(!preview.private_mode);
        assert!(preview.tun);
        assert!(preview.exposures.contains(&Exposure::PrivateModeOff));
        assert!(
            preview
                .exposures
                .contains(&Exposure::SharesSubnet("192.168.50.0/24".parse().unwrap()))
        );
        assert!(imported.ignored_keys.is_empty());
    }

    #[test]
    fn a_config_without_listeners_imports_with_none_and_says_so() {
        let imported =
            import_toml_with_id("[network_identity]\nnetwork_name = \"n\"\n", id()).unwrap();
        assert!(imported.profile.listeners.is_empty());
        assert!(imported.preview.default_listeners_dropped);
        assert_eq!(imported.secret, None);
        assert!(!imported.preview.has_secret);
        // Neither dhcp nor ipv4: no virtual address.
        assert_eq!(imported.profile.ip, IpMode::None);
    }

    #[test]
    fn exposure_widening_items_are_marked() {
        let text = r#"
dhcp = true
listeners = ["tcp://0.0.0.0:11010", "udp://[::]:11010", "tcp://127.0.0.1:11011"]
exit_nodes = ["10.144.144.9"]
socks5_proxy = "socks5://0.0.0.0:1080"

[network_identity]
network_name = "n"

[[proxy_network]]
cidr = "192.168.50.0/24"

[[port_forward]]
bind_addr = "0.0.0.0:8080"
dst_addr = "10.144.144.2:80"
proto = "tcp"

[flags]
enable_exit_node = true
enable_encryption = false
relay_all_peer_rpc = true
"#;
        let imported = import_toml_with_id(text, id()).unwrap();
        let preview = &imported.preview;
        assert_eq!(
            preview
                .listeners
                .iter()
                .filter(|l| l.all_interfaces)
                .count(),
            2
        );
        assert!(preview.offers_exit_node);
        assert_eq!(preview.exit_nodes.len(), 1);
        assert!(preview.port_forwards[0].public);
        assert!(preview.socks5.is_some_and(|s| s.public));
        assert_eq!(preview.encryption, Encryption::Off);
        for exposure in [
            Exposure::OffersExitNode,
            Exposure::EncryptionOff,
            Exposure::PrivateModeOff,
            Exposure::RelaysAllPeerRpc,
            Exposure::Socks5 { public: true },
            Exposure::SharesSubnet("192.168.50.0/24".parse().unwrap()),
            Exposure::UsesExitNode("10.144.144.9".parse().unwrap()),
        ] {
            assert!(preview.exposures.contains(&exposure), "{exposure:?}");
        }
        assert_eq!(
            preview
                .exposures
                .iter()
                .filter(|e| matches!(e, Exposure::ListenerOnAllInterfaces(_)))
                .count(),
            2
        );
        assert!(
            preview
                .exposures
                .iter()
                .any(|e| matches!(e, Exposure::ForwardsPort { public: true, .. }))
        );
    }

    #[test]
    fn unapplied_keys_are_listed() {
        let text = r#"
instance_name = "x"
dhcp = true
listeners = []
secure_mode = { enabled = true }
routes = ["10.0.0.0/8"]

[network_identity]
network_name = "n"
extra = 1

[[peer]]
uri = "tcp://203.0.113.7:11010"
peer_public_key = "AAAA"

[[proxy_network]]
cidr = "192.168.50.0/24"
mapped_cidr = "10.9.0.0/24"

[flags]
enable_kcp_proxy = true
mtu = 1380

[acl]
enabled = true
"#;
        let imported = import_toml_with_id(text, id()).unwrap();
        let mut ignored = imported.ignored_keys.clone();
        ignored.sort();
        assert_eq!(
            ignored,
            [
                "acl",
                "flags.enable_kcp_proxy",
                "network_identity.extra",
                "peer[0].peer_public_key",
                "proxy_network[0].mapped_cidr",
                "routes",
                "secure_mode",
            ]
        );
        assert_eq!(imported.profile.mtu, Some(1380));
    }

    fn reject(text: &str) -> ImportError {
        import_toml_with_id(text, id()).unwrap_err()
    }

    #[test]
    fn rejections_name_the_field() {
        let wrap = |top: &str, body: &str| {
            format!(
                "dhcp = true\nlisteners = []\n{top}\n[network_identity]\nnetwork_name = \"n\"\n{body}"
            )
        };
        let field_of = |body: &str| reject(&wrap("", body)).field().map(str::to_owned);
        let top_field_of = |top: &str| reject(&wrap(top, "")).field().map(str::to_owned);

        assert_eq!(
            field_of("[[peer]]\nuri = \"ftp://x.example.com\""),
            Some("peer[0].uri".into())
        );
        assert_eq!(
            field_of("[[peer]]\nuri = \"tcp://x.example.com:70000\""),
            Some("peer[0].uri".into())
        );
        assert_eq!(
            field_of("[[peer]]\nuri = \"tcp://x.example.com\"\n[[peer]]\nname = 1"),
            Some("peer[1].uri".into())
        );
        assert_eq!(
            field_of("[[proxy_network]]\ncidr = \"192.168.50.7/24\""),
            Some("proxy_network[0].cidr".into())
        );
        assert_eq!(
            field_of("[[proxy_network]]\ncidr = \"0.0.0.0/0\""),
            Some("proxy_network[0].cidr".into())
        );
        assert_eq!(
            field_of("[[proxy_network]]\ncidr = \"10.0.0.0/33\""),
            Some("proxy_network[0].cidr".into())
        );
        assert_eq!(
            field_of(
                "[[port_forward]]\nbind_addr = \"127.0.0.1:99999\"\ndst_addr = \"10.0.0.1:80\"\nproto = \"tcp\""
            ),
            Some("port_forward[0].bind_addr".into())
        );
        assert_eq!(
            field_of(
                "[[port_forward]]\nbind_addr = \"127.0.0.1:80\"\ndst_addr = \"10.0.0.1:80\"\nproto = \"sctp\""
            ),
            Some("port_forward[0].proto".into())
        );
        assert_eq!(field_of("[flags]\nmtu = 99999"), Some("flags.mtu".into()));
        assert_eq!(field_of("[flags]\nmtu = 100"), Some("mtu".into()));
        assert_eq!(field_of("[flags]\nmtu = \"big\""), Some("flags.mtu".into()));
        assert_eq!(
            field_of("[flags]\nencryption_algorithm = \"rot13\""),
            Some("flags.encryption_algorithm".into())
        );
        assert_eq!(
            field_of("[flags]\nno_tun = \"yes\""),
            Some("flags.no_tun".into())
        );
        assert_eq!(
            top_field_of("socks5_proxy = \"http://127.0.0.1:1080\""),
            Some("socks5_proxy".into())
        );
        assert_eq!(
            top_field_of("exit_nodes = [\"not-an-ip\"]"),
            Some("exit_nodes[0]".into())
        );

        let long = "n".repeat(65);
        assert_eq!(
            reject(&format!(
                "dhcp = true\nlisteners = []\n[network_identity]\nnetwork_name = \"{long}\""
            ))
            .field(),
            Some("network_name")
        );
        assert_eq!(
            reject("dhcp = true\nlisteners = []\n[network_identity]\nnetwork_name = \"\"").field(),
            Some("network_name")
        );
        assert_eq!(
            reject(&format!(
                "hostname = \"{}\"\ndhcp = true\nlisteners = []",
                "主机".repeat(17)
            ))
            .field(),
            Some("hostname")
        );
        assert_eq!(
            reject("ipv4 = \"10.144.144.1/31\"\nlisteners = []").field(),
            Some("ipv4")
        );
        assert_eq!(
            reject("dhcp = true\nlisteners = [\"txt://example.com\"]").field(),
            Some("listeners[0]")
        );
        assert_eq!(
            reject("dhcp = true\nlisteners = [5]").field(),
            Some("listeners[0]")
        );
    }

    #[test]
    fn syntax_errors_and_oversized_input_are_reported_without_the_text() {
        match reject("dhcp = true\ngarbage = [") {
            ImportError::Syntax { line, message } => {
                assert!(line.is_some());
                assert!(!message.contains("garbage"));
            }
            other => panic!("expected a syntax error, got {other:?}"),
        }
        let secret_in_bad_file = "network_secret = 'example-secret-do-not-use' = =";
        assert!(
            !reject(secret_in_bad_file)
                .to_string()
                .contains("example-secret")
        );
        assert_eq!(
            reject(&"#".repeat(MAX_IMPORT_BYTES + 1)),
            ImportError::TooLarge {
                max: MAX_IMPORT_BYTES
            }
        );
    }
}
