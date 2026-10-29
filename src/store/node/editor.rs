//! The network editor's draft: a profile as form text, parsed back into a profile on save.
//!
//! Lists are one entry per line; a feature that widens exposure has a switch and its text, and the text only counts
//! while the switch is on. `from_profile` followed by `build` gives back the same profile (tested).

use std::net::SocketAddr;

use super::{
    flow::parse_peer_at,
    problem::{Field, FieldProblem, ProblemKind, SaveFailure},
};
use crate::{
    core::{
        acl::{AclConfig, GroupSecrets},
        net::{self, ListenerUrl},
        secret::Secret,
        settings::{
            DEFAULT_VPN_PORTAL_PORT, ForwardLineError, MappedListener, NodeFlags, PortForward,
            VpnPortal,
        },
    },
    profile::{
        Encryption, Exposure, IpMode, MTU_RANGE, NetworkProfile, ProfileId, RelayWhitelist, Socks5,
        SubnetProxy, parse_route,
    },
};

/// How the node gets its virtual address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpChoice {
    /// Picked automatically.
    Dhcp,
    /// The address typed in the form.
    Static,
    /// None (a relay-only node; an import can produce it).
    None,
}

/// A tab of the network drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailTab {
    /// The running node: its throughput and latency, virtual IP, listeners, NAT type, version.
    Overview,
    /// Every setting of the profile.
    Settings,
    /// The core's recent output.
    Logs,
    /// The config as TOML, a share link and a QR code.
    Export,
}

/// The editor's form state for one profile.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorDraft {
    /// The profile as stored; `build` starts from it so fields the form does not carry survive.
    pub base: NetworkProfile,
    /// The name shown in the app.
    pub name: String,
    /// The hostname peers see (empty: the device's own).
    pub hostname: String,
    /// The EasyTier network name.
    pub network_name: String,
    /// The network secret (read from the keychain when the dialog opens).
    pub secret: Secret,
    /// The keychain answered (found or not).
    pub secret_loaded: bool,
    /// The keychain had no secret for this profile.
    pub secret_missing: bool,
    /// The person changed the secret, so saving writes it.
    pub secret_changed: bool,
    /// How the address is chosen.
    pub ip: IpChoice,
    /// The static address as typed (`10.144.144.1/24`).
    pub ip_text: String,
    /// Peers and shared nodes, one per line.
    pub peers: String,
    /// Accept incoming connections.
    pub listeners_on: bool,
    /// Listeners, one per line.
    pub listeners: String,
    /// Create a network interface (administrator rights).
    pub tun: bool,
    /// How peer traffic is protected.
    pub encryption: Encryption,
    /// The MTU as typed (empty: EasyTier's default).
    pub mtu: String,
    /// The interface name as typed (empty: the system's choice).
    pub dev_name: String,
    /// The core's behavior switches.
    pub flags: NodeFlags,
    /// The most bytes per second the instance receives, as typed (empty: unlimited).
    pub recv_limit: String,
    /// Public addresses of this node's listeners to announce to peers, one per line.
    pub mapped_listeners: String,
    /// Peers may route their internet traffic through this node.
    pub offers_exit_node: bool,
    /// Send this node's traffic through exit nodes.
    pub exit_nodes_on: bool,
    /// Exit nodes' virtual addresses, one per line.
    pub exit_nodes: String,
    /// Share subnets with the network.
    pub subnet_on: bool,
    /// Subnets, one per line: `192.168.50.0/24` or `192.168.50.0/24->10.50.0.0/24` (shown to peers as the second).
    pub subnets: String,
    /// Use the routes below instead of EasyTier's own (an empty list then means no routes at all).
    pub routes_on: bool,
    /// Manual routes, one per line (`192.168.0.0/16`).
    pub routes: String,
    /// Open the WireGuard VPN portal (listens on every interface).
    pub vpn_portal_on: bool,
    /// The addresses handed to WireGuard clients, as typed (`10.14.14.0/24`).
    pub vpn_portal_cidr: String,
    /// The portal's UDP port, as typed.
    pub vpn_portal_port: String,
    /// Serve a SOCKS5 proxy.
    pub socks5_on: bool,
    /// The proxy's `address:port` (or just a port, on this device only).
    pub socks5: String,
    /// Forward local ports into the network.
    pub forwards_on: bool,
    /// Forwards, one per line: `tcp 127.0.0.1:8080 10.144.144.2:80`.
    pub forwards: String,
    /// Relay every peer's control traffic.
    pub relay_all_peer_rpc: bool,
    /// Refuse peers of other networks unless they know the secret.
    pub private_mode: bool,
    /// Relay only for the listed networks.
    pub whitelist_only: bool,
    /// The networks to relay for, one pattern per line.
    pub whitelist: String,
    /// The access-control section with the secrets of its declared groups; `None` is no access control. The secrets
    /// are read from the keychain with the network secret ([`EditorDraft::apply_group_secrets`]).
    pub acl: Option<AclConfig>,
    /// The group secrets as the keychain gave them, to save only what changed.
    pub base_group_secrets: GroupSecrets,
    /// Whether empty or invalid fields show their errors (set when a save was refused).
    pub show_errors: bool,
    /// The profile is being saved.
    pub saving: bool,
    /// Why the last save failed.
    pub failure: Option<SaveFailure>,
}

/// The non-empty trimmed lines of `text`.
fn entries(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|line| !line.is_empty())
}

/// One entry per line.
fn join_lines<T: ToString>(items: impl IntoIterator<Item = T>) -> String {
    items
        .into_iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

impl EditorDraft {
    /// Takes the ACL group secrets the keychain answered with: they become the draft's and the baseline `save`
    /// compares against. Group secrets typed since the draft opened are kept.
    pub fn apply_group_secrets(&mut self, groups: GroupSecrets) {
        self.base_group_secrets = groups.clone();
        if let Some(config) = &mut self.acl
            && config.group_secrets.is_empty()
        {
            config.group_secrets = groups;
        }
    }

    /// The group secrets saving must write to the keychain: the declared groups' secrets when they differ from what
    /// the keychain gave (an empty set deletes the entry), `None` when nothing changed or the keychain has not
    /// answered yet.
    pub fn group_secrets_to_save(&self) -> Option<GroupSecrets> {
        if !self.secret_loaded || self.is_managed() {
            return None;
        }
        let mut current = self
            .acl
            .as_ref()
            .map(|config| config.group_secrets.clone())
            .unwrap_or_default();
        let declared = self
            .acl
            .as_ref()
            .map(|config| config.acl.declared_groups.clone())
            .unwrap_or_default();
        current.retain_names(&declared);
        (current != self.base_group_secrets).then_some(current)
    }

    /// The form for `profile`; the secret is filled in when the keychain has answered.
    pub fn from_profile(profile: &NetworkProfile) -> Self {
        let (ip, ip_text) = match profile.ip {
            IpMode::Dhcp => (IpChoice::Dhcp, String::new()),
            IpMode::Static(net) => (IpChoice::Static, net.to_string()),
            IpMode::None => (IpChoice::None, String::new()),
        };
        let (whitelist_only, whitelist) = match &profile.relay_whitelist {
            RelayWhitelist::Any => (false, String::new()),
            RelayWhitelist::Only(names) => (true, join_lines(names)),
        };
        Self {
            base: profile.clone(),
            name: profile.name.clone(),
            hostname: profile.hostname.clone().unwrap_or_default(),
            network_name: profile.network_name.clone(),
            secret: Secret::default(),
            secret_loaded: false,
            secret_missing: false,
            secret_changed: false,
            ip,
            ip_text,
            peers: join_lines(profile.peers.iter().map(|peer| peer.as_str())),
            listeners_on: !profile.listeners.is_empty(),
            listeners: join_lines(profile.listeners.iter().map(|l| l.as_str())),
            tun: profile.tun,
            encryption: profile.encryption,
            mtu: profile.mtu.map(|mtu| mtu.to_string()).unwrap_or_default(),
            dev_name: profile.dev_name.clone().unwrap_or_default(),
            flags: profile.flags,
            recv_limit: profile
                .instance_recv_bps_limit
                .map(|limit| limit.to_string())
                .unwrap_or_default(),
            mapped_listeners: join_lines(profile.mapped_listeners.iter().map(|l| l.as_str())),
            offers_exit_node: profile.offers_exit_node,
            exit_nodes_on: !profile.exit_nodes.is_empty(),
            exit_nodes: join_lines(&profile.exit_nodes),
            subnet_on: !profile.subnet_proxy.is_empty(),
            subnets: join_lines(&profile.subnet_proxy),
            routes_on: profile.manual_routes.is_some(),
            routes: join_lines(profile.manual_routes.iter().flatten()),
            vpn_portal_on: profile.vpn_portal.is_some(),
            vpn_portal_cidr: profile
                .vpn_portal
                .map(|portal| portal.client_cidr.to_string())
                .unwrap_or_default(),
            vpn_portal_port: profile
                .vpn_portal
                .map_or(DEFAULT_VPN_PORTAL_PORT, |portal| portal.listen_port)
                .to_string(),
            socks5_on: profile.socks5.is_some(),
            socks5: profile
                .socks5
                .map(|proxy| SocketAddr::new(proxy.host, proxy.port).to_string())
                .unwrap_or_default(),
            forwards_on: !profile.port_forwards.is_empty(),
            forwards: join_lines(&profile.port_forwards),
            relay_all_peer_rpc: profile.relay_all_peer_rpc,
            private_mode: profile.private_mode,
            whitelist_only,
            whitelist,
            acl: profile.acl.clone().map(|acl| AclConfig {
                acl,
                group_secrets: GroupSecrets::default(),
            }),
            base_group_secrets: GroupSecrets::default(),
            show_errors: false,
            saving: false,
            failure: None,
        }
    }

    /// Whether a control plane owns the network (only its name and hostname are edited here).
    pub fn is_managed(&self) -> bool {
        self.base.is_managed()
    }

    /// The profile's id.
    pub fn id(&self) -> &ProfileId {
        &self.base.id
    }

    /// The form as a profile, collecting what could not be parsed into `problems`. Entries that fail are left out.
    fn assemble(&self, problems: &mut Vec<FieldProblem>) -> NetworkProfile {
        let mut profile = self.base.clone();
        profile.name = self.name.trim().to_owned();
        let hostname = self.hostname.trim();
        profile.hostname = (!hostname.is_empty()).then(|| hostname.to_owned());
        if self.is_managed() {
            return profile;
        }
        profile.network_name = self.network_name.trim().to_owned();
        profile.ip = match self.ip {
            IpChoice::Dhcp => IpMode::Dhcp,
            IpChoice::None => IpMode::None,
            IpChoice::Static => match net::parse_ipv4_inet("ip", self.ip_text.trim()) {
                Ok(address) => IpMode::Static(address),
                Err(error) => {
                    problems.push(FieldProblem::from(&error));
                    IpMode::Dhcp
                }
            },
        };
        profile.peers = Vec::new();
        for (index, line) in entries(&self.peers).enumerate() {
            match parse_peer_at(&format!("peers[{index}]"), line) {
                Ok(peer) => profile.peers.push(peer),
                Err(problem) => problems.push(problem),
            }
        }
        profile.listeners = Vec::new();
        if self.listeners_on {
            for (index, line) in entries(&self.listeners).enumerate() {
                match ListenerUrl::parse(&format!("listeners[{index}]"), line) {
                    Ok(listener) => profile.listeners.push(listener),
                    Err(error) => problems.push(FieldProblem::from(&error)),
                }
            }
        }
        profile.tun = self.tun;
        profile.encryption = self.encryption;
        let mtu = self.mtu.trim();
        profile.mtu = if mtu.is_empty() {
            None
        } else if let Ok(value) = mtu.parse::<u16>() {
            Some(value)
        } else {
            let (min, max) = MTU_RANGE;
            problems.push(FieldProblem::of(
                Field::Mtu,
                ProblemKind::OutOfRange {
                    min: u32::from(min),
                    max: u32::from(max),
                },
            ));
            None
        };
        let dev_name = self.dev_name.trim();
        profile.dev_name = (!dev_name.is_empty()).then(|| dev_name.to_owned());
        profile.flags = self.flags;
        let recv_limit = self.recv_limit.trim();
        profile.instance_recv_bps_limit = if recv_limit.is_empty() {
            None
        } else if let Ok(limit) = recv_limit.parse::<u64>() {
            Some(limit)
        } else {
            problems.push(FieldProblem::of(
                Field::RecvLimit,
                ProblemKind::MustBePositive,
            ));
            None
        };
        profile.mapped_listeners = Vec::new();
        for (index, line) in entries(&self.mapped_listeners).enumerate() {
            match MappedListener::parse(&format!("mapped_listeners[{index}]"), line) {
                Ok(listener) => profile.mapped_listeners.push(listener),
                Err(error) => problems.push(FieldProblem::from(&error)),
            }
        }
        profile.offers_exit_node = self.offers_exit_node;
        profile.exit_nodes = Vec::new();
        if self.exit_nodes_on {
            for (index, line) in entries(&self.exit_nodes).enumerate() {
                match net::parse_ip_addr(&format!("exit_nodes[{index}]"), line) {
                    Ok(address) => profile.exit_nodes.push(address),
                    Err(error) => problems.push(FieldProblem::from(&error)),
                }
            }
        }
        profile.subnet_proxy = Vec::new();
        if self.subnet_on {
            for (index, line) in entries(&self.subnets).enumerate() {
                match SubnetProxy::parse_line(&format!("subnet_proxy[{index}]"), line) {
                    Ok(subnet) => profile.subnet_proxy.push(subnet),
                    Err(error) => problems.push(FieldProblem::from(&error)),
                }
            }
        }
        profile.manual_routes = None;
        if self.routes_on {
            let mut routes = Vec::new();
            for (index, line) in entries(&self.routes).enumerate() {
                match parse_route(&format!("routes[{index}]"), line) {
                    Ok(route) => routes.push(route),
                    Err(error) => problems.push(FieldProblem::from(&error)),
                }
            }
            profile.manual_routes = Some(routes);
        }
        profile.vpn_portal = None;
        if self.vpn_portal_on {
            let cidr =
                net::parse_ipv4_subnet("vpn_portal.client_cidr", self.vpn_portal_cidr.trim());
            let port = net::parse_port("vpn_portal.listen_port", &self.vpn_portal_port);
            match (cidr, port) {
                (Ok(client_cidr), Ok(listen_port)) => {
                    profile.vpn_portal = Some(VpnPortal {
                        client_cidr,
                        listen_port,
                    });
                }
                (cidr, port) => {
                    problems.extend(cidr.err().iter().map(FieldProblem::from));
                    problems.extend(port.err().iter().map(FieldProblem::from));
                }
            }
        }
        profile.socks5 = None;
        if self.socks5_on {
            match parse_socks5(&self.socks5) {
                Ok(proxy) => profile.socks5 = Some(proxy),
                Err(problem) => problems.push(problem),
            }
        }
        profile.port_forwards = Vec::new();
        if self.forwards_on {
            for (index, line) in entries(&self.forwards).enumerate() {
                match PortForward::parse_line(&format!("port_forwards[{index}]"), line) {
                    Ok(forward) => profile.port_forwards.push(forward),
                    Err(ForwardLineError::Shape) => problems.push(FieldProblem::in_entry(
                        Field::PortForwards,
                        index.saturating_add(1),
                        ProblemKind::ForwardShape,
                    )),
                    Err(ForwardLineError::Address(error)) => {
                        problems.push(FieldProblem::from(&error));
                    }
                }
            }
        }
        profile.relay_all_peer_rpc = self.relay_all_peer_rpc;
        profile.private_mode = self.private_mode;
        profile.relay_whitelist = if self.whitelist_only {
            RelayWhitelist::Only(entries(&self.whitelist).map(str::to_owned).collect())
        } else {
            RelayWhitelist::Any
        };
        profile.acl = self.acl.as_ref().map(|config| config.acl.clone());
        profile
    }

    /// The form as a valid profile.
    ///
    /// # Errors
    ///
    /// Every problem found, in field order, each naming its field and entry.
    pub fn build(&self) -> Result<NetworkProfile, Vec<FieldProblem>> {
        let mut problems = Vec::new();
        let profile = self.assemble(&mut problems);
        for error in profile.problems() {
            let problem = FieldProblem::from(&error);
            if !problems.contains(&problem) {
                problems.push(problem);
            }
        }
        if self.secret_loaded
            && !self.is_managed()
            && let Some(config) = &self.acl
            && let Err(error) = config.check()
        {
            let problem = FieldProblem::of(Field::Acl, ProblemKind::Acl(error));
            if !problems.contains(&problem) {
                problems.push(problem);
            }
        }
        if !self.is_managed() && self.secret_changed && self.secret.is_empty() {
            problems.push(FieldProblem::of(Field::Secret, ProblemKind::Required));
        }
        if problems.is_empty() {
            Ok(profile)
        } else {
            Err(problems)
        }
    }

    /// Every problem of the form now (empty when it can be saved).
    pub fn problems(&self) -> Vec<FieldProblem> {
        self.build().err().unwrap_or_default()
    }

    /// What the form would widen, from the entries that parse (for the warnings beside the switches).
    pub fn exposures(&self) -> Vec<Exposure> {
        self.assemble(&mut Vec::new()).exposures()
    }

    /// The problems about `field`.
    pub fn problems_of(&self, field: Field) -> Vec<FieldProblem> {
        self.problems()
            .into_iter()
            .filter(|problem| problem.field == field)
            .collect()
    }
}

/// `1080` (this device only) or `address:port`.
fn parse_socks5(text: &str) -> Result<Socks5, FieldProblem> {
    let text = text.trim();
    if !text.is_empty() && text.chars().all(|c| c.is_ascii_digit()) {
        return net::parse_port("socks5", text)
            .map(Socks5::local)
            .map_err(|error| FieldProblem::from(&error));
    }
    net::parse_socket_addr("socks5", text)
        .map(|addr| Socks5 {
            host: addr.ip(),
            port: addr.port(),
        })
        .map_err(|error| FieldProblem::from(&error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::settings::Proto, profile::testing};

    fn profile() -> NetworkProfile {
        let id = ProfileId::parse("0123456789abcdef0123456789abcdef").unwrap();
        let mut profile = NetworkProfile::new(id, "Home", "home-net");
        profile.peers = vec!["tcp://203.0.113.7:11010".parse().unwrap()];
        profile
    }

    #[test]
    fn a_form_made_from_a_profile_builds_the_same_profile() {
        let mut full = profile();
        full.ip = IpMode::Static("10.144.144.1/24".parse().unwrap());
        full.hostname = Some("laptop".to_owned());
        full.listeners = vec!["tcp://127.0.0.1:11010".parse().unwrap()];
        full.mtu = Some(1400);
        full.dev_name = Some("et0".to_owned());
        full.exit_nodes = vec!["10.144.144.9".parse().unwrap()];
        full.subnet_proxy = vec![SubnetProxy::plain("192.168.50.0/24".parse().unwrap())];
        full.socks5 = Some(Socks5::local(1080));
        full.port_forwards = vec![PortForward {
            proto: Proto::Udp,
            bind: "127.0.0.1:5353".parse().unwrap(),
            dst: "10.144.144.2:53".parse().unwrap(),
        }];
        full.relay_whitelist = RelayWhitelist::Only(vec!["team-*".to_owned()]);
        full.encryption = Encryption::ChaCha20;
        full.private_mode = false;
        full.offers_exit_node = true;
        assert_eq!(EditorDraft::from_profile(&full).build().unwrap(), full);
        assert_eq!(
            EditorDraft::from_profile(&profile()).build().unwrap(),
            profile()
        );
    }

    #[test]
    fn a_form_made_from_a_profile_with_every_field_builds_the_same_profile() {
        let (full, groups) = testing::full_profile().unwrap();
        let mut form = EditorDraft::from_profile(&full);
        assert_eq!(form.vpn_portal_port, "22022");
        assert_eq!(form.recv_limit, "1048576");
        assert_eq!(form.routes, "192.168.0.0/16");
        assert_eq!(form.subnets, "192.168.50.0/24->10.50.0.0/24");
        assert_eq!(form.forwards, "tcp 127.0.0.1:8080 10.144.144.2:80");
        assert_eq!(form.build().unwrap(), full);

        // Once the keychain answered, the group secrets are checked and saved only when they changed.
        form.secret_loaded = true;
        form.apply_group_secrets(groups.clone());
        assert_eq!(form.build().unwrap(), full);
        assert_eq!(form.group_secrets_to_save(), None);
        form.acl
            .as_mut()
            .unwrap()
            .group_secrets
            .insert("admins", Secret::new("a-new-secret"));
        let saved = form.group_secrets_to_save().unwrap();
        assert_eq!(saved.get("admins"), Some(&Secret::new("a-new-secret")));
        // Dropping the ACL deletes the entry.
        form.acl = None;
        assert_eq!(form.group_secrets_to_save(), Some(GroupSecrets::default()));
        assert_eq!(form.build().unwrap().acl, None);
    }

    #[test]
    fn a_declared_group_without_a_secret_is_one_acl_problem() {
        let (full, _) = testing::full_profile().unwrap();
        let mut form = EditorDraft::from_profile(&full);
        // Not asked yet: the keychain may still hold it.
        assert!(form.build().is_ok());
        form.secret_loaded = true;
        form.apply_group_secrets(GroupSecrets::default());
        let problems = form.problems();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].field, Field::Acl);
        assert!(matches!(problems[0].kind, ProblemKind::Acl(_)));
    }

    #[test]
    fn new_fields_report_their_problems() {
        let mut form = EditorDraft::from_profile(&profile());
        form.recv_limit = "fast".to_owned();
        form.mapped_listeners = "tcp://203.0.113.5:11223\nhttps://x.example.com".to_owned();
        form.routes_on = true;
        form.routes = "10.0.0.0/8\n10.1.2.3/8".to_owned();
        form.vpn_portal_on = true;
        form.vpn_portal_cidr = "10.14.14.7/24".to_owned();
        form.vpn_portal_port = "0".to_owned();
        form.subnet_on = true;
        form.subnets = "192.168.50.0/24->10.50.0.0/16".to_owned();
        let problems = form.problems();
        for expected in [
            FieldProblem::of(Field::RecvLimit, ProblemKind::MustBePositive),
            FieldProblem::in_entry(Field::MappedListeners, 2, ProblemKind::ConnectorOnly),
            FieldProblem::in_entry(Field::Routes, 2, ProblemKind::HostBits),
            FieldProblem::of(Field::VpnPortalCidr, ProblemKind::HostBits),
            FieldProblem::of(Field::VpnPortalPort, ProblemKind::BadPort),
            FieldProblem::in_entry(Field::SubnetProxy, 1, ProblemKind::MappedSize),
        ] {
            assert!(problems.contains(&expected), "{expected:?} in {problems:?}");
        }
        // An empty routes list is valid when manual routes are on: no routes at all.
        let mut form = EditorDraft::from_profile(&profile());
        form.routes_on = true;
        assert_eq!(form.build().unwrap().manual_routes, Some(Vec::new()));
        // Text of a switched-off feature is ignored.
        form.routes_on = false;
        form.routes = "garbage".to_owned();
        assert_eq!(form.build().unwrap().manual_routes, None);
    }

    #[test]
    fn problems_name_their_field_and_entry() {
        let mut form = EditorDraft::from_profile(&profile());
        form.peers = "tcp://203.0.113.7:11010\ngopher://x".to_owned();
        form.mtu = "99999".to_owned();
        form.subnet_on = true;
        form.subnets = "192.168.50.7/24".to_owned();
        let problems = form.problems();
        assert!(problems.contains(&FieldProblem::in_entry(
            Field::Peers,
            2,
            ProblemKind::UnknownScheme
        )));
        assert!(problems.iter().any(|p| p.field == Field::Mtu));
        assert!(problems.contains(&FieldProblem::in_entry(
            Field::SubnetProxy,
            1,
            ProblemKind::HostBits
        )));
    }

    #[test]
    fn text_of_a_switched_off_feature_is_ignored() {
        let mut form = EditorDraft::from_profile(&profile());
        form.subnets = "not a subnet".to_owned();
        assert!(form.build().is_ok());
        form.subnet_on = true;
        assert!(form.build().is_err());
    }

    #[test]
    fn exposures_follow_the_form() {
        let mut form = EditorDraft::from_profile(&profile());
        assert!(form.exposures().is_empty());
        form.listeners_on = true;
        form.listeners = "tcp://0.0.0.0:11010".to_owned();
        form.encryption = Encryption::Xor;
        let found = form.exposures();
        assert!(
            found
                .iter()
                .any(|e| matches!(e, Exposure::ListenerOnAllInterfaces(_)))
        );
        assert!(found.contains(&Exposure::ObfuscationOnly));
    }

    #[test]
    fn socks5_and_forwards_have_friendly_shapes() {
        let mut form = EditorDraft::from_profile(&profile());
        form.socks5_on = true;
        form.socks5 = "1080".to_owned();
        form.forwards_on = true;
        form.forwards = "tcp 127.0.0.1:8080 10.144.144.2:80".to_owned();
        let built = form.build().unwrap();
        assert_eq!(built.socks5, Some(Socks5::local(1080)));
        assert_eq!(built.port_forwards.len(), 1);
        form.forwards = "tcp 127.0.0.1:8080".to_owned();
        assert!(form.problems().contains(&FieldProblem::in_entry(
            Field::PortForwards,
            1,
            ProblemKind::ForwardShape
        )));
    }
}
