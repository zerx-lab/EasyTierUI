//! Rendering a profile to EasyTier's TOML config file, and turning it into a [`NodeLaunch`].
//!
//! The file layout and key names follow EasyTier 2.6.4's `Config` struct (`easytier/src/common/config.rs`); the
//! rendered text is checked against the real `easytier-core --check-config`. Unknown keys and keys in the wrong
//! table are silently ignored by EasyTier, so the typed structs below are the only place the names are spelled.
//! None of the structs derives `Debug`: they hold the network secret while the text is built.

use serde::Serialize;
use url::Url;

use super::{
    error::ProfileError,
    model::{ConfigSource, Encryption, IpMode, NetworkProfile, RelayWhitelist},
};
use crate::core::{
    acl::{AclConfig, GroupSecrets, wire},
    net::{MAX_URL_LEN, Scheme},
    node::{LaunchSource, NodeLaunch, Privilege},
    secret::Secret,
};

/// The whole config file. Plain values come first, then the tables: TOML requires it, and the serializer keeps the
/// order.
#[derive(Serialize)]
struct ConfigFile<'a> {
    instance_name: &'a str,
    instance_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ipv4: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dhcp: Option<bool>,
    #[serde(skip_serializing_if = "is_false")]
    ipv6_public_addr_auto: bool,
    // Written even when empty: an absent key makes the core open its default listeners on every interface.
    listeners: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mapped_listeners: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    exit_nodes: Vec<String>,
    // Written even when empty once manual routes are on: an empty list means no routes, an absent key means
    // EasyTier's own routing.
    #[serde(skip_serializing_if = "Option::is_none")]
    routes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    socks5_proxy: Option<String>,
    network_identity: NetworkIdentity<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    peer: Vec<PeerEntry<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    proxy_network: Vec<ProxyNetworkEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vpn_portal_config: Option<VpnPortalEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    port_forward: Vec<PortForwardEntry>,
    flags: Flags<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    acl: Option<wire::Acl>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Serialize)]
struct NetworkIdentity<'a> {
    network_name: &'a str,
    // An empty secret is the same network as no secret, and a shared link without the secret must not name one.
    #[serde(skip_serializing_if = "Option::is_none")]
    network_secret: Option<&'a str>,
}

#[derive(Serialize)]
struct PeerEntry<'a> {
    uri: &'a str,
}

#[derive(Serialize)]
struct ProxyNetworkEntry {
    cidr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mapped_cidr: Option<String>,
}

#[derive(Serialize)]
struct VpnPortalEntry {
    client_cidr: String,
    wireguard_listen: String,
}

#[derive(Serialize)]
struct PortForwardEntry {
    bind_addr: String,
    dst_addr: String,
    proto: &'static str,
}

#[derive(Serialize)]
struct Flags<'a> {
    latency_first: bool,
    use_smoltcp: bool,
    enable_ipv6: bool,
    enable_kcp_proxy: bool,
    disable_kcp_input: bool,
    enable_quic_proxy: bool,
    disable_quic_input: bool,
    disable_p2p: bool,
    p2p_only: bool,
    lazy_p2p: bool,
    need_p2p: bool,
    bind_device: bool,
    no_tun: bool,
    enable_exit_node: bool,
    relay_all_peer_rpc: bool,
    multi_thread: bool,
    proxy_forward_by_system: bool,
    enable_encryption: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    encryption_algorithm: Option<&'static str>,
    disable_tcp_hole_punching: bool,
    disable_udp_hole_punching: bool,
    disable_sym_hole_punching: bool,
    enable_udp_broadcast_relay: bool,
    disable_upnp: bool,
    accept_dns: bool,
    private_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtu: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance_recv_bps_limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relay_network_whitelist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_name: Option<&'a str>,
}

impl NetworkProfile {
    /// Renders the profile as EasyTier's TOML config. `secret` is the network secret; an empty one leaves the key
    /// out (a network without a secret). `groups` holds the secrets of the ACL groups this profile declares.
    ///
    /// The text holds the secrets: it is returned as a [`Secret`] and belongs in an owner-only file or a share link
    /// the person asked for, nowhere else.
    ///
    /// # Errors
    ///
    /// [`ProfileError::Managed`] for a managed profile; the first problem [`NetworkProfile::validate`] finds;
    /// [`ProfileError::Acl`] for a declared group without a secret in `groups`; [`ProfileError::Render`] if the
    /// serializer refuses (a bug).
    pub fn render(&self, secret: &Secret, groups: &GroupSecrets) -> Result<Secret, ProfileError> {
        self.render_with(secret, groups, true)
    }

    /// [`NetworkProfile::render`], optionally without insisting that every declared ACL group has a secret (an
    /// export without secrets names the groups only).
    pub fn render_with(
        &self,
        secret: &Secret,
        groups: &GroupSecrets,
        groups_required: bool,
    ) -> Result<Secret, ProfileError> {
        if self.is_managed() {
            return Err(ProfileError::Managed);
        }
        self.validate()?;
        let acl = match &self.acl {
            Some(acl) => {
                let config = AclConfig {
                    acl: acl.clone(),
                    group_secrets: groups.clone(),
                };
                if groups_required {
                    config.check().map_err(ProfileError::Acl)?;
                }
                Some(config.to_wire())
            }
            None => None,
        };
        let key = self.id.instance_key()?;
        let (ipv4, dhcp) = match self.ip {
            IpMode::Dhcp => (None, Some(true)),
            IpMode::Static(net) => (Some(net.to_string()), None),
            IpMode::None => (None, None),
        };
        let file = ConfigFile {
            instance_name: key.as_str(),
            instance_id: self.id.instance_uuid()?,
            hostname: self.hostname.as_deref(),
            ipv4,
            dhcp,
            ipv6_public_addr_auto: self.flags.ipv6_public_addr_auto,
            listeners: self.listeners.iter().map(|l| l.as_str()).collect(),
            mapped_listeners: self.mapped_listeners.iter().map(|l| l.as_str()).collect(),
            exit_nodes: self.exit_nodes.iter().map(ToString::to_string).collect(),
            routes: self
                .manual_routes
                .as_ref()
                .map(|routes| routes.iter().map(ToString::to_string).collect()),
            socks5_proxy: self
                .socks5
                .map(|s| format!("socks5://{}", std::net::SocketAddr::new(s.host, s.port))),
            network_identity: NetworkIdentity {
                network_name: &self.network_name,
                network_secret: (!secret.is_empty()).then_some(secret.expose()),
            },
            peer: self
                .peers
                .iter()
                .map(|p| PeerEntry { uri: p.as_str() })
                .collect(),
            proxy_network: self
                .subnet_proxy
                .iter()
                .map(|subnet| ProxyNetworkEntry {
                    cidr: subnet.cidr.to_string(),
                    mapped_cidr: subnet.mapped.map(|mapped| mapped.to_string()),
                })
                .collect(),
            vpn_portal_config: self.vpn_portal.map(|portal| VpnPortalEntry {
                client_cidr: portal.client_cidr.to_string(),
                wireguard_listen: format!("0.0.0.0:{}", portal.listen_port),
            }),
            port_forward: self
                .port_forwards
                .iter()
                .map(|f| PortForwardEntry {
                    bind_addr: f.bind.to_string(),
                    dst_addr: f.dst.to_string(),
                    proto: f.proto.name(),
                })
                .collect(),
            flags: Flags {
                latency_first: self.flags.latency_first,
                use_smoltcp: self.flags.use_smoltcp,
                enable_ipv6: !self.flags.disable_ipv6,
                enable_kcp_proxy: self.flags.enable_kcp_proxy,
                disable_kcp_input: self.flags.disable_kcp_input,
                enable_quic_proxy: self.flags.enable_quic_proxy,
                disable_quic_input: self.flags.disable_quic_input,
                disable_p2p: self.flags.disable_p2p,
                p2p_only: self.flags.p2p_only,
                lazy_p2p: self.flags.lazy_p2p,
                need_p2p: self.flags.need_p2p,
                bind_device: self.flags.bind_device,
                no_tun: !self.tun,
                enable_exit_node: self.offers_exit_node,
                relay_all_peer_rpc: self.relay_all_peer_rpc,
                multi_thread: self.flags.multi_thread,
                proxy_forward_by_system: self.flags.proxy_forward_by_system,
                enable_encryption: self.encryption != Encryption::Off,
                encryption_algorithm: self.encryption.algorithm(),
                disable_tcp_hole_punching: self.flags.disable_tcp_hole_punching,
                disable_udp_hole_punching: self.flags.disable_udp_hole_punching,
                disable_sym_hole_punching: self.flags.disable_sym_hole_punching,
                enable_udp_broadcast_relay: self.flags.enable_udp_broadcast_relay,
                disable_upnp: self.flags.disable_upnp,
                accept_dns: self.flags.accept_dns,
                private_mode: self.private_mode,
                mtu: self.mtu,
                instance_recv_bps_limit: self.instance_recv_bps_limit,
                relay_network_whitelist: match &self.relay_whitelist {
                    RelayWhitelist::Any => None,
                    RelayWhitelist::Only(names) => Some(names.join(" ")),
                },
                dev_name: self.dev_name.as_deref(),
            },
            acl,
        };
        toml::to_string(&file)
            .map(Secret::new)
            .map_err(|_| ProfileError::Render)
    }

    /// Everything a node backend needs to start this profile: a local profile's rendered config (a TUN profile is
    /// [`Privilege::System`], the rest [`Privilege::User`]), or a managed profile's enrollment.
    ///
    /// `secret` is the profile's keychain entry: the network secret of a local profile, the control-plane address
    /// of a managed one. `groups` holds the ACL group secrets (empty for a managed profile). `machine_id` is this
    /// installation's id (see [`crate::profile::repo::machine_id`]).
    ///
    /// # Errors
    ///
    /// The first validation problem; [`ProfileError::BadConfigServer`] for a managed profile whose address is not
    /// `tcp|udp|ws|wss://host[:port]/<account>`; [`ProfileError::BadMachineId`] for a managed profile and a machine
    /// id that is not a UUID; whatever [`NetworkProfile::render`] reports.
    pub fn launch(
        &self,
        secret: &Secret,
        groups: &GroupSecrets,
        machine_id: &str,
    ) -> Result<NodeLaunch, ProfileError> {
        self.validate()?;
        let key = self.id.instance_key()?;
        match self.source {
            ConfigSource::Local => Ok(NodeLaunch {
                key,
                privilege: self.privilege(),
                source: LaunchSource::Local {
                    config_toml: self.render(secret, groups)?,
                },
            }),
            ConfigSource::Managed => {
                check_config_server(secret)?;
                check_machine_id(machine_id)?;
                Ok(NodeLaunch {
                    key,
                    // A managed node is the user's child process, never an OS service.
                    privilege: Privilege::User,
                    source: LaunchSource::Managed {
                        config_server: secret.clone(),
                        machine_id: machine_id.to_owned(),
                        hostname: self.hostname.clone(),
                    },
                })
            }
        }
    }
}

/// Checks a control-plane address: `tcp|udp|ws|wss://host[:port]/<account>`, the account being the easytier-web
/// user the machine enrolls into. The message never repeats the address.
///
/// # Errors
///
/// [`ProfileError::BadConfigServer`].
pub fn check_config_server(address: &Secret) -> Result<(), ProfileError> {
    let text = address.expose().trim();
    if text.is_empty() || text.chars().count() > MAX_URL_LEN {
        return Err(ProfileError::BadConfigServer);
    }
    let url = Url::parse(text).map_err(|_| ProfileError::BadConfigServer)?;
    let scheme_ok = Scheme::parse(url.scheme()).is_some_and(|scheme| {
        matches!(scheme, Scheme::Tcp | Scheme::Udp | Scheme::Ws | Scheme::Wss)
    });
    let account = url.path().trim_matches('/');
    let ok = scheme_ok
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.port() != Some(0)
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && !account.is_empty()
        && !account.contains('/')
        && !text.chars().any(|c| c.is_whitespace() || c.is_control());
    if ok {
        Ok(())
    } else {
        Err(ProfileError::BadConfigServer)
    }
}

/// Checks the machine id is a UUID in its hyphenated text form, as the core's `--machine-id` expects.
fn check_machine_id(machine_id: &str) -> Result<(), ProfileError> {
    match uuid::Uuid::try_parse(machine_id) {
        Ok(_) if machine_id.len() == 36 => Ok(()),
        _ => Err(ProfileError::BadMachineId),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::settings::{PortForward, Proto},
        profile::{
            model::{ProfileId, Socks5, SubnetProxy},
            testing,
        },
    };

    const ID: &str = "0123456789abcdef0123456789abcdef";
    const HEAD: &str = "instance_name = \"p-0123456789abcdef0123456789abcdef\"\ninstance_id = \"01234567-89ab-cdef-0123-456789abcdef\"\n";

    fn secret() -> Secret {
        Secret::new("example-secret-do-not-use")
    }

    fn profile() -> NetworkProfile {
        NetworkProfile::new(ProfileId::parse(ID).unwrap(), "Home", "example-net")
    }

    fn render(profile: &NetworkProfile) -> String {
        profile
            .render(&secret(), &GroupSecrets::default())
            .unwrap()
            .expose()
            .to_owned()
    }

    const IDENTITY: &str = "\n[network_identity]\nnetwork_name = \"example-net\"\nnetwork_secret = \"example-secret-do-not-use\"\n";

    /// The `[flags]` table of a default profile (EasyTier's defaults for every switch the app does not own).
    fn flags(no_tun: bool, enc: &str, private: bool) -> String {
        format!(
            "\n[flags]\nlatency_first = false\nuse_smoltcp = false\nenable_ipv6 = true\nenable_kcp_proxy = false\ndisable_kcp_input = false\nenable_quic_proxy = false\ndisable_quic_input = false\ndisable_p2p = false\np2p_only = false\nlazy_p2p = false\nneed_p2p = false\nbind_device = true\nno_tun = {no_tun}\nenable_exit_node = false\nrelay_all_peer_rpc = false\nmulti_thread = true\nproxy_forward_by_system = false\n{enc}disable_tcp_hole_punching = false\ndisable_udp_hole_punching = false\ndisable_sym_hole_punching = false\nenable_udp_broadcast_relay = false\ndisable_upnp = false\naccept_dns = false\nprivate_mode = {private}\n"
        )
    }

    const AES: &str = "enable_encryption = true\nencryption_algorithm = \"aes-gcm\"\n";

    #[test]
    fn default_joiner() {
        let expected = format!(
            "{HEAD}dhcp = true\nlisteners = []\n{IDENTITY}{}",
            flags(false, AES, true)
        );
        assert_eq!(render(&profile()), expected);
    }

    #[test]
    fn static_ip_hostname_and_no_tun() {
        let mut p = profile();
        p.ip = IpMode::Static("10.144.144.1/24".parse().unwrap());
        p.hostname = Some("家用路由器".to_owned());
        p.tun = false;
        let expected = format!(
            "{HEAD}hostname = \"家用路由器\"\nipv4 = \"10.144.144.1/24\"\nlisteners = []\n{IDENTITY}{}",
            flags(true, AES, true)
        );
        assert_eq!(render(&p), expected);
    }

    #[test]
    fn relay_only_node_has_no_address_keys() {
        let mut p = profile();
        p.ip = IpMode::None;
        let expected = format!(
            "{HEAD}listeners = []\n{IDENTITY}{}",
            flags(false, AES, true)
        );
        assert_eq!(render(&p), expected);
    }

    #[test]
    fn peers_and_listeners() {
        let mut p = profile();
        p.peers = vec![
            "tcp://203.0.113.7:11010".parse().unwrap(),
            "wss://relay.example.com".parse().unwrap(),
        ];
        p.listeners = vec![
            "tcp://0.0.0.0:11010".parse().unwrap(),
            "udp://[::]:11010".parse().unwrap(),
        ];
        let expected = format!(
            "{HEAD}dhcp = true\nlisteners = [\"tcp://0.0.0.0:11010\", \"udp://[::]:11010\"]\n{IDENTITY}\n[[peer]]\nuri = \"tcp://203.0.113.7:11010\"\n\n[[peer]]\nuri = \"wss://relay.example.com/\"\n{}",
            flags(false, AES, true)
        );
        assert_eq!(render(&p), expected);
    }

    #[test]
    fn subnet_proxy_exit_node_forward_and_socks5() {
        let mut p = profile();
        p.subnet_proxy = vec![SubnetProxy::plain("192.168.50.0/24".parse().unwrap())];
        p.offers_exit_node = true;
        p.exit_nodes = vec!["10.144.144.9".parse().unwrap()];
        p.socks5 = Some(Socks5::local(1080));
        p.port_forwards = vec![PortForward {
            proto: Proto::Tcp,
            bind: "127.0.0.1:41200".parse().unwrap(),
            dst: "10.144.144.1:41201".parse().unwrap(),
        }];
        let expected = format!(
            "{HEAD}dhcp = true\nlisteners = []\nexit_nodes = [\"10.144.144.9\"]\nsocks5_proxy = \"socks5://127.0.0.1:1080\"\n{IDENTITY}\n[[proxy_network]]\ncidr = \"192.168.50.0/24\"\n\n[[port_forward]]\nbind_addr = \"127.0.0.1:41200\"\ndst_addr = \"10.144.144.1:41201\"\nproto = \"tcp\"\n{}",
            flags(false, AES, true).replace("enable_exit_node = false", "enable_exit_node = true")
        );
        assert_eq!(render(&p), expected);
    }

    #[test]
    fn encryption_variants() {
        let cases = [
            (
                Encryption::Aes256Gcm,
                "enable_encryption = true\nencryption_algorithm = \"aes-256-gcm\"\n",
            ),
            (
                Encryption::ChaCha20,
                "enable_encryption = true\nencryption_algorithm = \"chacha20\"\n",
            ),
            (
                Encryption::Xor,
                "enable_encryption = true\nencryption_algorithm = \"xor\"\n",
            ),
            (Encryption::Off, "enable_encryption = false\n"),
        ];
        for (encryption, lines) in cases {
            let mut p = profile();
            p.encryption = encryption;
            let expected = format!(
                "{HEAD}dhcp = true\nlisteners = []\n{IDENTITY}{}",
                flags(false, lines, true)
            );
            assert_eq!(render(&p), expected, "{encryption:?}");
        }
    }

    #[test]
    fn tuning_flags_and_whitelist() {
        let mut p = profile();
        p.mtu = Some(1400);
        p.flags.latency_first = true;
        p.flags.disable_p2p = true;
        p.flags.accept_dns = true;
        p.flags.multi_thread = false;
        p.private_mode = false;
        p.relay_all_peer_rpc = true;
        p.relay_whitelist = RelayWhitelist::Only(vec!["team-*".to_owned(), "lab".to_owned()]);
        p.dev_name = Some("et0".to_owned());
        let expected = format!(
            "{HEAD}dhcp = true\nlisteners = []\n{IDENTITY}{}mtu = 1400\nrelay_network_whitelist = \"team-* lab\"\ndev_name = \"et0\"\n",
            flags(false, AES, false)
                .replace("latency_first = false", "latency_first = true")
                .replace("disable_p2p = false", "disable_p2p = true")
                .replace("accept_dns = false", "accept_dns = true")
                .replace("multi_thread = true", "multi_thread = false")
                .replace("relay_all_peer_rpc = false", "relay_all_peer_rpc = true")
        );
        assert_eq!(render(&p), expected);
    }

    #[test]
    fn every_field_renders_to_the_keys_easytier_parsed() {
        let (profile, groups) = testing::full_profile().unwrap();
        let ours = profile.render(&secret(), &groups).unwrap();
        let theirs: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/easytier-2.6.4/web-parse-config-full-request.json"
        ))
        .unwrap();
        let theirs = theirs["toml_config"].as_str().unwrap();
        let mut ours: toml::Table = toml::from_str(ours.expose()).unwrap();
        let mut theirs: toml::Table = toml::from_str(theirs).unwrap();
        // The identity of the instance is the app's own.
        for table in [&mut ours, &mut theirs] {
            table.remove("instance_name");
            table.remove("instance_id");
        }
        assert_eq!(ours, theirs);
    }

    #[test]
    fn manual_routes_are_written_only_when_on_and_empty_means_none() {
        let mut p = profile();
        assert!(!render(&p).contains("routes"));
        p.manual_routes = Some(Vec::new());
        assert!(render(&p).contains("\nroutes = []\n"));
    }

    #[test]
    fn a_declared_group_without_a_secret_does_not_render() {
        let (profile, _) = testing::full_profile().unwrap();
        assert!(matches!(
            profile.render(&secret(), &GroupSecrets::default()),
            Err(ProfileError::Acl(_))
        ));
    }

    #[test]
    fn group_secrets_stay_out_of_debug_output() {
        let (_, groups) = testing::full_profile().unwrap();
        assert!(!format!("{groups:?}").contains("example-group-secret"));
    }

    #[test]
    fn an_empty_secret_leaves_the_key_out() {
        let p = profile();
        let text = p
            .render(&Secret::new(""), &GroupSecrets::default())
            .unwrap();
        assert!(
            text.expose()
                .contains("network_name = \"example-net\"\n\n[flags]")
        );
        assert!(!text.expose().contains("network_secret"));
    }

    #[test]
    fn managed_and_invalid_profiles_do_not_render() {
        let managed = NetworkProfile::managed(ProfileId::parse(ID).unwrap(), "Office");
        assert_eq!(
            managed.render(&secret(), &GroupSecrets::default()),
            Err(ProfileError::Managed)
        );
        let mut bad = profile();
        bad.network_name = String::new();
        assert!(matches!(
            bad.render(&secret(), &GroupSecrets::default()),
            Err(ProfileError::Empty { .. })
        ));
    }

    #[test]
    fn local_launches_follow_the_tun_choice() {
        let p = profile();
        let launch = p
            .launch(
                &secret(),
                &GroupSecrets::default(),
                "6f9619ff-8b86-d011-b42d-00c04fc964ff",
            )
            .unwrap();
        assert_eq!(launch.key.as_str(), "p-0123456789abcdef0123456789abcdef");
        assert_eq!(launch.privilege, Privilege::System);
        match launch.source {
            LaunchSource::Local { config_toml } => assert_eq!(config_toml.expose(), render(&p)),
            LaunchSource::Managed { .. } => panic!("expected a local launch"),
        }
        let mut no_tun = profile();
        no_tun.tun = false;
        assert_eq!(
            no_tun
                .launch(
                    &secret(),
                    &GroupSecrets::default(),
                    "6f9619ff-8b86-d011-b42d-00c04fc964ff"
                )
                .unwrap()
                .privilege,
            Privilege::User
        );
    }

    #[test]
    fn managed_launches_run_as_the_user_with_the_enrollment() {
        let mut p = NetworkProfile::managed(ProfileId::parse(ID).unwrap(), "Office");
        p.hostname = Some("laptop".to_owned());
        let address = Secret::new("udp://control.example.com:22020/team-7f3k9q");
        let machine = "6f9619ff-8b86-d011-b42d-00c04fc964ff";
        let launch = p
            .launch(&address, &GroupSecrets::default(), machine)
            .unwrap();
        assert_eq!(launch.privilege, Privilege::User);
        assert_eq!(
            launch.source,
            LaunchSource::Managed {
                config_server: address.clone(),
                machine_id: machine.to_owned(),
                hostname: Some("laptop".to_owned()),
            }
        );
        // The address is an enrollment credential: it never appears in errors.
        let error = p
            .launch(
                &Secret::new("http://control.example.com/team-7f3k9q"),
                &GroupSecrets::default(),
                machine,
            )
            .unwrap_err();
        assert_eq!(error, ProfileError::BadConfigServer);
        assert!(!error.to_string().contains("team-7f3k9q"));
        assert_eq!(
            p.launch(&address, &GroupSecrets::default(), "not-a-uuid"),
            Err(ProfileError::BadMachineId)
        );
    }

    #[test]
    fn control_plane_addresses_are_checked() {
        for good in [
            "udp://control.example.com:22020/team-7f3k9q",
            "tcp://10.0.0.5/team",
            "wss://control.example.com/team",
            "ws://[::1]:22020/team",
        ] {
            assert_eq!(check_config_server(&Secret::new(good)), Ok(()), "{good}");
        }
        for bad in [
            "",
            "team",
            "udp://control.example.com",
            "udp://control.example.com/",
            "udp://control.example.com/a/b",
            "udp://user:pw@control.example.com/team",
            "udp://control.example.com:0/team",
            "udp://control.example.com/team?x=1",
            "quic://control.example.com/team",
            "udp://control.example.com/te am",
        ] {
            assert_eq!(
                check_config_server(&Secret::new(bad)),
                Err(ProfileError::BadConfigServer),
                "{bad}"
            );
        }
    }
}
