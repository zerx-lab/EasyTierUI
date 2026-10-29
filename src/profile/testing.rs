//! Test fixtures shared by the profile and store tests: a profile that sets every field, mirroring the full config
//! EasyTier 2.6.4 parsed in `tests/fixtures/easytier-2.6.4/web-parse-config-full-request.json`.

use super::model::{
    Encryption, IpMode, NetworkProfile, ProfileId, RelayWhitelist, Socks5, SubnetProxy,
};
use crate::core::{
    acl::{
        Acl, AclAction, AclChain, AclProtocol, AclRule, ChainKind, GroupSecrets, IpRange, PortRange,
    },
    secret::Secret,
    settings::{MappedListener, NodeFlags, PortForward, Proto, VpnPortal},
};

/// The profile id the fixtures use.
pub(crate) const ID: &str = "0123456789abcdef0123456789abcdef";

/// The one ACL group secret the fixtures use, for group `admins`.
pub(crate) fn group_secrets() -> GroupSecrets {
    let mut groups = GroupSecrets::default();
    groups.insert("admins", Secret::new("example-group-secret"));
    groups
}

/// A profile with every setting away from its default, and the secrets of its ACL groups.
pub(crate) fn full_profile() -> Option<(NetworkProfile, GroupSecrets)> {
    let id = ProfileId::parse(ID).ok()?;
    let mut profile = NetworkProfile::new(id, "probe-net", "probe-net");
    profile.hostname = Some("probe-host".to_owned());
    profile.ip = IpMode::Static("10.144.144.5/24".parse().ok()?);
    profile.peers = vec!["tcp://203.0.113.9:11010".parse().ok()?];
    profile.listeners = vec![
        "tcp://0.0.0.0:11010".parse().ok()?,
        "udp://0.0.0.0:11010".parse().ok()?,
    ];
    profile.mapped_listeners = vec![MappedListener::parse("m", "tcp://203.0.113.5:11223").ok()?];
    profile.tun = false;
    profile.encryption = Encryption::AesGcm;
    profile.mtu = Some(1300);
    profile.instance_recv_bps_limit = Some(1_048_576);
    profile.flags = NodeFlags {
        latency_first: true,
        use_smoltcp: true,
        disable_ipv6: true,
        ipv6_public_addr_auto: true,
        enable_kcp_proxy: true,
        disable_kcp_input: true,
        enable_quic_proxy: true,
        disable_quic_input: true,
        disable_p2p: true,
        p2p_only: true,
        lazy_p2p: true,
        need_p2p: true,
        bind_device: false,
        multi_thread: false,
        proxy_forward_by_system: true,
        disable_tcp_hole_punching: true,
        disable_udp_hole_punching: true,
        disable_sym_hole_punching: true,
        enable_udp_broadcast_relay: true,
        disable_upnp: true,
        accept_dns: true,
    };
    profile.subnet_proxy = vec![SubnetProxy {
        cidr: "192.168.50.0/24".parse().ok()?,
        mapped: Some("10.50.0.0/24".parse().ok()?),
    }];
    profile.manual_routes = Some(vec!["192.168.0.0/16".parse().ok()?]);
    profile.vpn_portal = Some(VpnPortal {
        client_cidr: "10.14.14.0/24".parse().ok()?,
        listen_port: 22022,
    });
    profile.offers_exit_node = true;
    profile.exit_nodes = vec!["10.144.144.1".parse().ok()?];
    profile.socks5 = Some(Socks5::local(1080));
    profile.port_forwards = vec![PortForward {
        proto: Proto::Tcp,
        bind: "127.0.0.1:8080".parse().ok()?,
        dst: "10.144.144.2:80".parse().ok()?,
    }];
    profile.relay_all_peer_rpc = true;
    profile.private_mode = true;
    profile.relay_whitelist = RelayWhitelist::Only(vec!["team-*".to_owned(), "other".to_owned()]);
    profile.dev_name = Some("et0".to_owned());

    let mut rule = AclRule::new("web");
    rule.protocol = AclProtocol::Tcp;
    rule.source_ips = vec![IpRange::parse("10.144.144.0/24").ok()?];
    rule.destination_ports = vec![
        PortRange::parse("80").ok()?,
        PortRange::parse("8000-8080").ok()?,
    ];
    rule.action = AclAction::Allow;
    rule.stateful = true;
    rule.source_groups = vec!["admins".to_owned()];
    let mut chain = AclChain::new(ChainKind::Inbound, "Inbound");
    chain.description = "d".to_owned();
    chain.default_action = AclAction::Drop;
    chain.rules = vec![rule];
    profile.acl = Some(Acl {
        chains: vec![chain],
        declared_groups: vec!["admins".to_owned()],
        joined_groups: vec!["admins".to_owned()],
    });
    Some((profile, group_secrets()))
}
