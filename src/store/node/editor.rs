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
        net::{self, ListenerUrl},
        secret::Secret,
    },
    profile::{
        Encryption, Exposure, IpMode, MTU_RANGE, NetworkProfile, PortForward, ProfileId, Proto,
        RelayWhitelist, Socks5,
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
    /// Prefer the lowest-latency route.
    pub latency_first: bool,
    /// Never hole-punch.
    pub disable_p2p: bool,
    /// Resolve peer hostnames through EasyTier's DNS.
    pub accept_dns: bool,
    /// Run the core on several threads.
    pub multi_thread: bool,
    /// Peers may route their internet traffic through this node.
    pub offers_exit_node: bool,
    /// Send this node's traffic through exit nodes.
    pub exit_nodes_on: bool,
    /// Exit nodes' virtual addresses, one per line.
    pub exit_nodes: String,
    /// Share subnets with the network.
    pub subnet_on: bool,
    /// Subnets, one per line.
    pub subnets: String,
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
            latency_first: profile.latency_first,
            disable_p2p: profile.disable_p2p,
            accept_dns: profile.accept_dns,
            multi_thread: profile.multi_thread,
            offers_exit_node: profile.offers_exit_node,
            exit_nodes_on: !profile.exit_nodes.is_empty(),
            exit_nodes: join_lines(&profile.exit_nodes),
            subnet_on: !profile.subnet_proxy.is_empty(),
            subnets: join_lines(&profile.subnet_proxy),
            socks5_on: profile.socks5.is_some(),
            socks5: profile
                .socks5
                .map(|proxy| SocketAddr::new(proxy.host, proxy.port).to_string())
                .unwrap_or_default(),
            forwards_on: !profile.port_forwards.is_empty(),
            forwards: join_lines(profile.port_forwards.iter().map(|forward| {
                format!("{} {} {}", forward.proto.name(), forward.bind, forward.dst)
            })),
            relay_all_peer_rpc: profile.relay_all_peer_rpc,
            private_mode: profile.private_mode,
            whitelist_only,
            whitelist,
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
        profile.latency_first = self.latency_first;
        profile.disable_p2p = self.disable_p2p;
        profile.accept_dns = self.accept_dns;
        profile.multi_thread = self.multi_thread;
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
                match net::parse_ipv4_subnet(&format!("subnet_proxy[{index}]"), line) {
                    Ok(subnet) => profile.subnet_proxy.push(subnet),
                    Err(error) => problems.push(FieldProblem::from(&error)),
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
                match parse_forward(index, line) {
                    Ok(forward) => profile.port_forwards.push(forward),
                    Err(problem) => problems.push(problem),
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

/// `tcp 127.0.0.1:8080 10.144.144.2:80`.
fn parse_forward(index: usize, line: &str) -> Result<PortForward, FieldProblem> {
    let entry = index.saturating_add(1);
    let shape = FieldProblem::in_entry(Field::PortForwards, entry, ProblemKind::ForwardShape);
    let mut parts = line.split_whitespace();
    let (Some(proto), Some(bind), Some(dst), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(shape);
    };
    let proto = match proto.to_ascii_lowercase().as_str() {
        "tcp" => Proto::Tcp,
        "udp" => Proto::Udp,
        _ => return Err(shape),
    };
    let address = |part: &str, which: &str| -> Result<SocketAddr, FieldProblem> {
        net::parse_socket_addr(&format!("port_forwards[{index}].{which}"), part)
            .map_err(|error| FieldProblem::from(&error))
    };
    Ok(PortForward {
        proto,
        bind: address(bind, "bind")?,
        dst: address(dst, "dst")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        full.subnet_proxy = vec!["192.168.50.0/24".parse().unwrap()];
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
