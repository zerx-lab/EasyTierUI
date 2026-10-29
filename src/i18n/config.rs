//! Words for the network settings both editors share: the core's behavior switches, the VPN portal, mapped
//! listeners, manual routes, the receive limit and the access control list (ACL).
//!
//! One field per string; `EN` and `ZH_CN` must both set every field. Templates use `{name}` placeholders filled by
//! [`super::fill`]. EasyTier's help texts are rewritten in the app's voice, not translated word for word.

use crate::core::{
    acl::ChainKind,
    settings::{Flag, FlagGroup},
};

/// Strings of the shared settings editors in one language.
#[derive(Debug)]
pub struct ConfigStrings {
    /// Heading of the behavior switches section.
    pub section_flags: &'static str,
    /// Sub-label of the switches about routes and direct links.
    pub flag_group_routing: &'static str,
    /// Sub-label of the switches about NAT traversal.
    pub flag_group_traversal: &'static str,
    /// Sub-label of the switches about KCP and QUIC.
    pub flag_group_transport: &'static str,
    /// Sub-label of the switches about the network stack and the system.
    pub flag_group_system: &'static str,

    /// Label: prefer the lowest latency.
    pub flag_latency_first: &'static str,
    /// Help: prefer the lowest latency.
    pub flag_latency_first_help: &'static str,
    /// Label: no automatic direct links.
    pub flag_disable_p2p: &'static str,
    /// Help: no automatic direct links.
    pub flag_disable_p2p_help: &'static str,
    /// Label: direct links only.
    pub flag_p2p_only: &'static str,
    /// Help: direct links only.
    pub flag_p2p_only_help: &'static str,
    /// Label: direct links on demand.
    pub flag_lazy_p2p: &'static str,
    /// Help: direct links on demand.
    pub flag_lazy_p2p_help: &'static str,
    /// Label: ask peers for a direct link.
    pub flag_need_p2p: &'static str,
    /// Help: ask peers for a direct link.
    pub flag_need_p2p_help: &'static str,
    /// Label: physical interfaces only.
    pub flag_bind_device: &'static str,
    /// Help: physical interfaces only.
    pub flag_bind_device_help: &'static str,
    /// Label: no TCP hole punching.
    pub flag_disable_tcp_hole_punching: &'static str,
    /// Help: no TCP hole punching.
    pub flag_disable_tcp_hole_punching_help: &'static str,
    /// Label: no UDP hole punching.
    pub flag_disable_udp_hole_punching: &'static str,
    /// Help: no UDP hole punching.
    pub flag_disable_udp_hole_punching_help: &'static str,
    /// Label: no symmetric NAT hole punching.
    pub flag_disable_sym_hole_punching: &'static str,
    /// Help: no symmetric NAT hole punching.
    pub flag_disable_sym_hole_punching_help: &'static str,
    /// Label: no UPnP / NAT-PMP.
    pub flag_disable_upnp: &'static str,
    /// Help: no UPnP / NAT-PMP.
    pub flag_disable_upnp_help: &'static str,
    /// Label: TCP over KCP.
    pub flag_enable_kcp_proxy: &'static str,
    /// Help: TCP over KCP.
    pub flag_enable_kcp_proxy_help: &'static str,
    /// Label: refuse incoming KCP.
    pub flag_disable_kcp_input: &'static str,
    /// Help: refuse incoming KCP.
    pub flag_disable_kcp_input_help: &'static str,
    /// Label: TCP over QUIC.
    pub flag_enable_quic_proxy: &'static str,
    /// Help: TCP over QUIC.
    pub flag_enable_quic_proxy_help: &'static str,
    /// Label: refuse incoming QUIC.
    pub flag_disable_quic_input: &'static str,
    /// Help: refuse incoming QUIC.
    pub flag_disable_quic_input_help: &'static str,
    /// Label: several threads.
    pub flag_multi_thread: &'static str,
    /// Help: several threads.
    pub flag_multi_thread_help: &'static str,
    /// Label: user-space network stack.
    pub flag_use_smoltcp: &'static str,
    /// Help: user-space network stack.
    pub flag_use_smoltcp_help: &'static str,
    /// Label: no IPv6.
    pub flag_disable_ipv6: &'static str,
    /// Help: no IPv6.
    pub flag_disable_ipv6_help: &'static str,
    /// Label: automatic public IPv6 address.
    pub flag_ipv6_public_addr_auto: &'static str,
    /// Help: automatic public IPv6 address.
    pub flag_ipv6_public_addr_auto_help: &'static str,
    /// Label: magic DNS.
    pub flag_accept_dns: &'static str,
    /// Help: magic DNS.
    pub flag_accept_dns_help: &'static str,
    /// Label: forwarding by the system kernel.
    pub flag_proxy_forward_by_system: &'static str,
    /// Help: forwarding by the system kernel.
    pub flag_proxy_forward_by_system_help: &'static str,
    /// Label: UDP broadcast relay.
    pub flag_enable_udp_broadcast_relay: &'static str,
    /// Help: UDP broadcast relay.
    pub flag_enable_udp_broadcast_relay_help: &'static str,
    /// Exposure note under the kernel-forwarding switch while it is on.
    pub flag_forward_by_system_exposure: &'static str,

    /// Switch: the WireGuard VPN portal.
    pub vpn_portal: &'static str,
    /// What the VPN portal is for.
    pub vpn_portal_help: &'static str,
    /// Field: the subnet WireGuard clients get addresses from.
    pub vpn_portal_cidr: &'static str,
    /// Help under the client subnet field.
    pub vpn_portal_cidr_help: &'static str,
    /// Field: the portal's UDP port.
    pub vpn_portal_port: &'static str,
    /// Exposure note while the portal is on: it listens on every interface.
    pub vpn_portal_exposure: &'static str,
    /// Field: the public addresses of listeners.
    pub mapped_listeners: &'static str,
    /// Help under the mapped listeners field.
    pub mapped_listeners_help: &'static str,
    /// Switch: manual routes.
    pub manual_routes: &'static str,
    /// What manual routes do, including that an empty list means no routes.
    pub manual_routes_help: &'static str,
    /// Field: the manual routes themselves.
    pub manual_routes_field: &'static str,
    /// Field: the instance's receive rate limit.
    pub recv_limit: &'static str,
    /// Help under the receive limit field.
    pub recv_limit_help: &'static str,
    /// Placeholder of the receive limit field.
    pub recv_limit_placeholder: &'static str,
    /// Help under the port forwards field: the line form.
    pub port_forwards_help: &'static str,
    /// Heading of the access control section.
    pub section_acl: &'static str,

    /// Switch: access control on or off.
    pub acl_switch: &'static str,
    /// What access control does.
    pub acl_switch_help: &'static str,
    /// Confirmation title: turning access control off.
    pub acl_off_title: &'static str,
    /// Confirmation body: turning access control off.
    pub acl_off_body: &'static str,
    /// Confirmation action: turn access control off.
    pub acl_off_action: &'static str,
    /// Empty state title: no chains yet.
    pub acl_empty_title: &'static str,
    /// Empty state body: what chains do.
    pub acl_empty_body: &'static str,
    /// Sub-label of the chain list.
    pub acl_chains: &'static str,
    /// Button: add an inbound chain.
    pub acl_add_inbound: &'static str,
    /// Button: add an outbound chain.
    pub acl_add_outbound: &'static str,
    /// Button: add a forward chain.
    pub acl_add_forward: &'static str,
    /// Kind: inbound traffic (also a new chain's name).
    pub acl_kind_inbound: &'static str,
    /// Kind: outbound traffic (also a new chain's name).
    pub acl_kind_outbound: &'static str,
    /// Kind: forwarded traffic (also a new chain's name).
    pub acl_kind_forward: &'static str,
    /// What inbound chains apply to.
    pub acl_kind_inbound_help: &'static str,
    /// What outbound chains apply to.
    pub acl_kind_outbound_help: &'static str,
    /// What forward chains apply to.
    pub acl_kind_forward_help: &'static str,
    /// Field: a chain's name.
    pub acl_chain_name: &'static str,
    /// Field: a chain's description.
    pub acl_chain_description: &'static str,
    /// Switch: a chain is in use.
    pub acl_chain_enabled: &'static str,
    /// Field: the traffic a chain applies to.
    pub acl_chain_kind: &'static str,
    /// Field: what happens when no rule matches.
    pub acl_chain_default: &'static str,
    /// Warning: a chain that drops by default.
    pub acl_chain_drops_all: &'static str,
    /// Action word: let traffic through.
    pub acl_allow: &'static str,
    /// Action word: drop traffic.
    pub acl_drop: &'static str,
    /// Button: delete a chain.
    pub acl_chain_delete: &'static str,
    /// Confirmation title: delete a chain; `{name}` is the chain.
    pub acl_chain_delete_title: &'static str,
    /// Confirmation body: delete a chain.
    pub acl_chain_delete_body: &'static str,
    /// Sub-label of a chain's rule list.
    pub acl_rules: &'static str,
    /// Note under the rule list: the order rules are checked in.
    pub acl_rules_order: &'static str,
    /// Empty rule list.
    pub acl_rules_empty: &'static str,
    /// Button and dialog title: add a rule.
    pub acl_rule_add: &'static str,
    /// Button and dialog title: edit a rule.
    pub acl_rule_edit: &'static str,
    /// Button: delete a rule.
    pub acl_rule_delete: &'static str,
    /// Name of a rule without one; `{n}` is its place in the chain.
    pub acl_rule_untitled: &'static str,
    /// Chip: a rule's priority; `{n}` is the number.
    pub acl_rule_priority_chip: &'static str,
    /// Chip: a rule that is switched off.
    pub acl_rule_disabled: &'static str,
    /// Field: a rule's name.
    pub acl_rule_name: &'static str,
    /// Field: a rule's description.
    pub acl_rule_description: &'static str,
    /// Switch: a rule is in use.
    pub acl_rule_enabled: &'static str,
    /// Field: allow or drop.
    pub acl_rule_action: &'static str,
    /// Field: the protocol a rule matches.
    pub acl_rule_protocol: &'static str,
    /// Protocol option: every protocol.
    pub acl_protocol_any: &'static str,
    /// Field: source addresses.
    pub acl_rule_source_ips: &'static str,
    /// Field: destination addresses.
    pub acl_rule_destination_ips: &'static str,
    /// Help under the address fields.
    pub acl_rule_ips_help: &'static str,
    /// Field: source ports.
    pub acl_rule_source_ports: &'static str,
    /// Field: destination ports.
    pub acl_rule_destination_ports: &'static str,
    /// Help under the port fields.
    pub acl_rule_ports_help: &'static str,
    /// Shown instead of the port fields for protocols without ports.
    pub acl_rule_no_ports: &'static str,
    /// Field: a rule's priority.
    pub acl_rule_priority: &'static str,
    /// Help under the priority field.
    pub acl_rule_priority_help: &'static str,
    /// Switch: connection tracking.
    pub acl_rule_stateful: &'static str,
    /// What connection tracking does.
    pub acl_rule_stateful_help: &'static str,
    /// Field: packets per second.
    pub acl_rule_rate_limit: &'static str,
    /// Field: burst size in packets.
    pub acl_rule_burst_limit: &'static str,
    /// Help under the limit fields.
    pub acl_rule_limit_help: &'static str,
    /// Sub-label: groups the source must belong to.
    pub acl_rule_source_groups: &'static str,
    /// Sub-label: groups the destination must belong to.
    pub acl_rule_destination_groups: &'static str,
    /// Help under the group pickers.
    pub acl_rule_groups_help: &'static str,
    /// Shown when no group is known yet.
    pub acl_rule_no_groups: &'static str,
    /// Shown when saving a rule failed on its fields.
    pub acl_rule_fix_fields: &'static str,
    /// Error of a field: `{n}` is the entry, `{problem}` what is wrong with it.
    pub acl_entry_error: &'static str,
    /// Error of a number field: `{max}` is the highest value.
    pub acl_number_error: &'static str,
    /// An entry is empty.
    pub acl_range_empty: &'static str,
    /// An entry is not an address.
    pub acl_range_bad_ip: &'static str,
    /// A network written with host bits.
    pub acl_range_host_bits: &'static str,
    /// An entry is not a port.
    pub acl_range_bad_port: &'static str,
    /// Sub-label of the group section.
    pub acl_groups: &'static str,
    /// What groups are for.
    pub acl_groups_help: &'static str,
    /// Sub-label: the groups this node declares.
    pub acl_declared: &'static str,
    /// What declaring a group means.
    pub acl_declared_help: &'static str,
    /// No group declared.
    pub acl_groups_empty: &'static str,
    /// Button: add a group.
    pub acl_group_add: &'static str,
    /// Button and dialog title: edit a group.
    pub acl_group_edit: &'static str,
    /// Button: delete a group.
    pub acl_group_delete: &'static str,
    /// Field: a group's name.
    pub acl_group_name: &'static str,
    /// Field: a group's secret.
    pub acl_group_secret: &'static str,
    /// Help under the group secret.
    pub acl_group_secret_help: &'static str,
    /// A group row: its secret is set.
    pub acl_group_secret_set: &'static str,
    /// A group row: its secret is missing.
    pub acl_group_secret_missing: &'static str,
    /// Error: this platform cannot generate a secret.
    pub acl_group_generate_failed: &'static str,
    /// Confirmation title: delete a group; `{name}` is the group.
    pub acl_group_delete_title: &'static str,
    /// Confirmation body: what deleting a group changes; `{count}` rules use it.
    pub acl_group_delete_body: &'static str,
    /// Sub-label: the groups this node belongs to.
    pub acl_joined: &'static str,
    /// What belonging to a group means.
    pub acl_joined_help: &'static str,
    /// Shown when there is nothing to join.
    pub acl_joined_none: &'static str,
    /// A membership of a group this node does not declare.
    pub acl_joined_undeclared: &'static str,
    /// Button: leave a group.
    pub acl_joined_remove: &'static str,
    /// Where a problem is: the whole ACL.
    pub acl_place_acl: &'static str,
    /// Where a problem is: a chain (`{chain}` counts from 1).
    pub acl_place_chain: &'static str,
    /// Where a problem is: a rule (`{chain}`, `{rule}` count from 1).
    pub acl_place_rule: &'static str,
    /// Where a problem is: a declared group (`{group}` counts from 1).
    pub acl_place_declared: &'static str,
    /// Where a problem is: a membership (`{group}` counts from 1).
    pub acl_place_joined: &'static str,
    /// One problem line: `{place}` and `{problem}`.
    pub acl_error_line: &'static str,
    /// Problem: too many entries (`{max}`).
    pub acl_problem_too_many: &'static str,
    /// Problem: too long (`{max}` characters).
    pub acl_problem_too_long: &'static str,
    /// Problem: a bad name.
    pub acl_problem_bad_name: &'static str,
    /// Problem: a name used twice.
    pub acl_problem_duplicate: &'static str,
    /// Problem: a declared group without a secret.
    pub acl_problem_missing_secret: &'static str,
    /// Problem: priority too high (`{max}`).
    pub acl_problem_priority: &'static str,
    /// Problem: ports on a protocol without ports.
    pub acl_problem_ports: &'static str,
    /// Problem: an address or port is invalid (`{problem}` says how).
    pub acl_problem_range: &'static str,
    /// Problem: not something the ACL defines.
    pub acl_problem_unreadable: &'static str,
}

impl ConfigStrings {
    /// The label of the switch `flag`.
    pub const fn flag_label(&self, flag: Flag) -> &'static str {
        match flag {
            Flag::LatencyFirst => self.flag_latency_first,
            Flag::DisableP2p => self.flag_disable_p2p,
            Flag::P2pOnly => self.flag_p2p_only,
            Flag::LazyP2p => self.flag_lazy_p2p,
            Flag::NeedP2p => self.flag_need_p2p,
            Flag::BindDevice => self.flag_bind_device,
            Flag::DisableTcpHolePunching => self.flag_disable_tcp_hole_punching,
            Flag::DisableUdpHolePunching => self.flag_disable_udp_hole_punching,
            Flag::DisableSymHolePunching => self.flag_disable_sym_hole_punching,
            Flag::DisableUpnp => self.flag_disable_upnp,
            Flag::EnableKcpProxy => self.flag_enable_kcp_proxy,
            Flag::DisableKcpInput => self.flag_disable_kcp_input,
            Flag::EnableQuicProxy => self.flag_enable_quic_proxy,
            Flag::DisableQuicInput => self.flag_disable_quic_input,
            Flag::MultiThread => self.flag_multi_thread,
            Flag::UseSmoltcp => self.flag_use_smoltcp,
            Flag::DisableIpv6 => self.flag_disable_ipv6,
            Flag::Ipv6PublicAddrAuto => self.flag_ipv6_public_addr_auto,
            Flag::MagicDns => self.flag_accept_dns,
            Flag::ProxyForwardBySystem => self.flag_proxy_forward_by_system,
            Flag::UdpBroadcastRelay => self.flag_enable_udp_broadcast_relay,
        }
    }

    /// What the switch `flag` does.
    pub const fn flag_help(&self, flag: Flag) -> &'static str {
        match flag {
            Flag::LatencyFirst => self.flag_latency_first_help,
            Flag::DisableP2p => self.flag_disable_p2p_help,
            Flag::P2pOnly => self.flag_p2p_only_help,
            Flag::LazyP2p => self.flag_lazy_p2p_help,
            Flag::NeedP2p => self.flag_need_p2p_help,
            Flag::BindDevice => self.flag_bind_device_help,
            Flag::DisableTcpHolePunching => self.flag_disable_tcp_hole_punching_help,
            Flag::DisableUdpHolePunching => self.flag_disable_udp_hole_punching_help,
            Flag::DisableSymHolePunching => self.flag_disable_sym_hole_punching_help,
            Flag::DisableUpnp => self.flag_disable_upnp_help,
            Flag::EnableKcpProxy => self.flag_enable_kcp_proxy_help,
            Flag::DisableKcpInput => self.flag_disable_kcp_input_help,
            Flag::EnableQuicProxy => self.flag_enable_quic_proxy_help,
            Flag::DisableQuicInput => self.flag_disable_quic_input_help,
            Flag::MultiThread => self.flag_multi_thread_help,
            Flag::UseSmoltcp => self.flag_use_smoltcp_help,
            Flag::DisableIpv6 => self.flag_disable_ipv6_help,
            Flag::Ipv6PublicAddrAuto => self.flag_ipv6_public_addr_auto_help,
            Flag::MagicDns => self.flag_accept_dns_help,
            Flag::ProxyForwardBySystem => self.flag_proxy_forward_by_system_help,
            Flag::UdpBroadcastRelay => self.flag_enable_udp_broadcast_relay_help,
        }
    }

    /// The sub-label of the switch group `group`.
    pub const fn flag_group(&self, group: FlagGroup) -> &'static str {
        match group {
            FlagGroup::Routing => self.flag_group_routing,
            FlagGroup::Traversal => self.flag_group_traversal,
            FlagGroup::Transport => self.flag_group_transport,
            FlagGroup::System => self.flag_group_system,
        }
    }

    /// The name of a chain kind.
    pub const fn acl_kind(&self, kind: ChainKind) -> &'static str {
        match kind {
            ChainKind::Inbound => self.acl_kind_inbound,
            ChainKind::Outbound => self.acl_kind_outbound,
            ChainKind::Forward => self.acl_kind_forward,
        }
    }

    /// What a chain kind applies to.
    pub const fn acl_kind_help(&self, kind: ChainKind) -> &'static str {
        match kind {
            ChainKind::Inbound => self.acl_kind_inbound_help,
            ChainKind::Outbound => self.acl_kind_outbound_help,
            ChainKind::Forward => self.acl_kind_forward_help,
        }
    }

    /// The button that adds a chain of `kind`.
    pub const fn acl_add_chain(&self, kind: ChainKind) -> &'static str {
        match kind {
            ChainKind::Inbound => self.acl_add_inbound,
            ChainKind::Outbound => self.acl_add_outbound,
            ChainKind::Forward => self.acl_add_forward,
        }
    }
}

pub(super) static EN: ConfigStrings = ConfigStrings {
    section_flags: "Behavior",
    flag_group_routing: "Routes and direct links",
    flag_group_traversal: "NAT traversal",
    flag_group_transport: "KCP and QUIC",
    flag_group_system: "Network stack and system",

    flag_latency_first: "Prefer the lowest latency",
    flag_latency_first_help: "Picks the route with the lowest total latency instead of the fewest hops.",
    flag_disable_p2p: "Don't open direct links on its own",
    flag_disable_p2p_help: "This node stops starting direct (P2P) links. Peers that ask for a direct link can still make one.",
    flag_p2p_only: "Direct links only",
    flag_p2p_only_help: "Talks only to peers it has a direct link to and never sends traffic through a relay.",
    flag_lazy_p2p: "Open direct links on demand",
    flag_lazy_p2p_help: "Tries a direct link to a peer only when traffic is headed there. Peers that ask for a direct link are still connected right away.",
    flag_need_p2p: "Ask peers for a direct link",
    flag_need_p2p_help: "Peers that open direct links on demand still connect to this node directly right away.",
    flag_bind_device: "Use physical interfaces only",
    flag_bind_device_help: "Keeps EasyTier from connecting over virtual networks such as other VPNs.",
    flag_disable_tcp_hole_punching: "Turn off TCP hole punching",
    flag_disable_tcp_hole_punching_help: "Skips TCP hole punching when looking for a direct link.",
    flag_disable_udp_hole_punching: "Turn off UDP hole punching",
    flag_disable_udp_hole_punching_help: "Skips UDP hole punching when looking for a direct link.",
    flag_disable_sym_hole_punching: "Turn off symmetric NAT hole punching",
    flag_disable_sym_hole_punching_help: "Skips the birthday-attack punching used behind symmetric NATs and treats them like cone NATs.",
    flag_disable_upnp: "Turn off UPnP and NAT-PMP",
    flag_disable_upnp_help: "Stops asking the router to open ports for this node's listeners. Port mapping is on by default.",
    flag_enable_kcp_proxy: "Carry TCP over KCP",
    flag_enable_kcp_proxy_help: "Turns TCP traffic into KCP between peers, which can lower latency and raise speed.",
    flag_disable_kcp_input: "Refuse incoming KCP",
    flag_disable_kcp_input_help: "Peers that use the KCP proxy connect to this node over TCP instead.",
    flag_enable_quic_proxy: "Carry TCP over QUIC",
    flag_enable_quic_proxy_help: "Turns TCP traffic into QUIC between peers, which can lower latency and raise speed.",
    flag_disable_quic_input: "Refuse incoming QUIC",
    flag_disable_quic_input_help: "Peers that use the QUIC proxy connect to this node over TCP instead.",
    flag_multi_thread: "Use several threads",
    flag_multi_thread_help: "Spreads the core's work over more processor cores. One thread uses fewer resources on small devices.",
    flag_use_smoltcp: "Use the user-space network stack",
    flag_use_smoltcp_help: "The subnet proxy and the KCP and QUIC proxies use EasyTier's own TCP/IP stack, which avoids trouble with the operating system's firewall.",
    flag_disable_ipv6: "Turn off IPv6",
    flag_disable_ipv6_help: "This node uses IPv4 only inside the network.",
    flag_ipv6_public_addr_auto: "Get a public IPv6 address automatically",
    flag_ipv6_public_addr_auto_help: "Takes a public IPv6 address from a peer that shares its IPv6 subnet.",
    flag_accept_dns: "Resolve peer names (magic DNS)",
    flag_accept_dns_help: "Peers can be reached by hostname, such as node1.et.net, through EasyTier's own name service.",
    flag_proxy_forward_by_system: "Forward through the system kernel",
    flag_proxy_forward_by_system_help: "The operating system forwards the packets of proxied subnets instead of EasyTier's own NAT.",
    flag_enable_udp_broadcast_relay: "Relay UDP broadcasts",
    flag_enable_udp_broadcast_relay_help: "Windows only: passes this device's local UDP broadcasts on to peers, so games can find rooms on the LAN. Needs administrator rights.",
    flag_forward_by_system_exposure: "The kernel forwards the traffic of proxied subnets without EasyTier's NAT, and the ACL's forward chain no longer applies to it. Turn it on only if you manage this system's firewall yourself.",

    vpn_portal: "WireGuard portal",
    vpn_portal_help: "Plain WireGuard clients join the network through this node, without installing EasyTier.",
    vpn_portal_cidr: "Client subnet",
    vpn_portal_cidr_help: "The addresses WireGuard clients get, such as 10.14.14.0/24. They must not overlap the network's own addresses.",
    vpn_portal_port: "Listen port",
    vpn_portal_exposure: "The portal listens on every network interface of this device, not only the virtual one. Anyone who can reach that UDP port can try to connect as a WireGuard client.",
    mapped_listeners: "Mapped listeners",
    mapped_listeners_help: "If peers can reach this node from outside by another address, such as a port forwarded on your router, list it here so they use it. One address per line.",
    manual_routes: "Manual routes",
    manual_routes_help: "You assign the routes yourself. This turns off the subnet proxy and WireGuard routes that peers announce. With no entries, no routes are added.",
    manual_routes_field: "Routes, one per line",
    recv_limit: "Receive limit",
    recv_limit_help: "Caps what this network receives on this node in total, in bytes per second. Leave it empty for no limit.",
    recv_limit_placeholder: "No limit",
    port_forwards_help: "One per line: the protocol (tcp or udp), the local address and port, then the target address and port inside the network.",
    section_acl: "Access control",

    acl_switch: "Access control (ACL)",
    acl_switch_help: "Allow or drop traffic by direction, address, port, protocol and group. Without rules, everything is allowed.",
    acl_off_title: "Turn off access control?",
    acl_off_body: "The chains and groups set up here are removed from this configuration.",
    acl_off_action: "Turn off",
    acl_empty_title: "No chains yet",
    acl_empty_body: "A chain holds the rules for one direction of traffic: inbound to this node, outbound from it, or forwarded into the subnets it proxies. Until you add one, everything is allowed.",
    acl_chains: "Chains",
    acl_add_inbound: "Add inbound chain",
    acl_add_outbound: "Add outbound chain",
    acl_add_forward: "Add forward chain",
    acl_kind_inbound: "Inbound",
    acl_kind_outbound: "Outbound",
    acl_kind_forward: "Forward",
    acl_kind_inbound_help: "Traffic sent to this node.",
    acl_kind_outbound_help: "Traffic this node sends.",
    acl_kind_forward_help: "Traffic this node forwards into the subnets it proxies.",
    acl_chain_name: "Name",
    acl_chain_description: "Description",
    acl_chain_enabled: "Chain in use",
    acl_chain_kind: "Applies to",
    acl_chain_default: "When no rule matches",
    acl_chain_drops_all: "This chain drops every packet that no rule allows. Make sure an allow rule covers the traffic you need.",
    acl_allow: "Allow",
    acl_drop: "Drop",
    acl_chain_delete: "Delete chain",
    acl_chain_delete_title: "Delete chain “{name}”?",
    acl_chain_delete_body: "The chain and all its rules are removed from this configuration.",
    acl_rules: "Rules",
    acl_rules_order: "Rules are checked from the highest priority down.",
    acl_rules_empty: "No rules: the default action decides everything.",
    acl_rule_add: "Add rule",
    acl_rule_edit: "Edit rule",
    acl_rule_delete: "Delete rule",
    acl_rule_untitled: "Rule {n}",
    acl_rule_priority_chip: "Priority {n}",
    acl_rule_disabled: "Off",
    acl_rule_name: "Name",
    acl_rule_description: "Description",
    acl_rule_enabled: "Rule in use",
    acl_rule_action: "Action",
    acl_rule_protocol: "Protocol",
    acl_protocol_any: "Any",
    acl_rule_source_ips: "Source addresses",
    acl_rule_destination_ips: "Destination addresses",
    acl_rule_ips_help: "IP addresses or networks, separated by commas or lines. Empty matches any.",
    acl_rule_source_ports: "Source ports",
    acl_rule_destination_ports: "Destination ports",
    acl_rule_ports_help: "Ports or ranges, separated by commas or lines. Empty matches any.",
    acl_rule_no_ports: "This protocol has no ports.",
    acl_rule_priority: "Priority",
    acl_rule_priority_help: "From 0 to 65535. The higher number is checked first.",
    acl_rule_stateful: "Track connections",
    acl_rule_stateful_help: "Replies to a connection the rule allowed are allowed too.",
    acl_rule_rate_limit: "Rate limit (packets per second)",
    acl_rule_burst_limit: "Burst limit (packets)",
    acl_rule_limit_help: "Empty or 0 for no limit.",
    acl_rule_source_groups: "Source groups",
    acl_rule_destination_groups: "Destination groups",
    acl_rule_groups_help: "Matches only peers in the chosen groups. Choose none to match any peer.",
    acl_rule_no_groups: "No groups yet. Declare one under Groups to match peers by group.",
    acl_rule_fix_fields: "Fix the marked fields to save this rule.",
    acl_entry_error: "Entry {n}: {problem}",
    acl_number_error: "Enter a whole number from 0 to {max}.",
    acl_range_empty: "it is empty",
    acl_range_bad_ip: "not an IP address or a network such as 10.144.144.0/24",
    acl_range_host_bits: "the address has bits set beyond the prefix; write the network address",
    acl_range_bad_port: "not a port or a range such as 1000-2000",
    acl_groups: "Groups",
    acl_groups_help: "A group ties rules to peers: nodes prove they belong to a group with its secret, and rules can match on groups.",
    acl_declared: "Declared groups",
    acl_declared_help: "Groups whose secret this node knows.",
    acl_groups_empty: "No groups declared.",
    acl_group_add: "Add group",
    acl_group_edit: "Edit group",
    acl_group_delete: "Delete group",
    acl_group_name: "Group name",
    acl_group_secret: "Group secret",
    acl_group_secret_help: "Every node of the group needs the same secret. Keep it as private as the network secret.",
    acl_group_secret_set: "Secret set",
    acl_group_secret_missing: "No secret",
    acl_group_generate_failed: "This device can't generate a secret. Type one instead.",
    acl_group_delete_title: "Delete group “{name}”?",
    acl_group_delete_body: "Rules lose their condition on this group ({count} affected), and a rule left with no group matches every peer. This node leaves the group.",
    acl_joined: "Memberships",
    acl_joined_help: "Groups this node belongs to. Rules on other nodes can match it by them.",
    acl_joined_none: "Declare a group first to join it.",
    acl_joined_undeclared: "Not declared on this node",
    acl_joined_remove: "Leave group",
    acl_place_acl: "Access control",
    acl_place_chain: "Chain {chain}",
    acl_place_rule: "Chain {chain}, rule {rule}",
    acl_place_declared: "Declared group {group}",
    acl_place_joined: "Membership {group}",
    acl_error_line: "{place}: {problem}",
    acl_problem_too_many: "Too many entries; at most {max} are allowed.",
    acl_problem_too_long: "Too long; at most {max} characters are allowed.",
    acl_problem_bad_name: "The name is empty or has control characters.",
    acl_problem_duplicate: "This name is used more than once.",
    acl_problem_missing_secret: "No secret set; every declared group needs one.",
    acl_problem_priority: "The priority is above {max}.",
    acl_problem_ports: "A rule for this protocol can't match ports. Clear the ports or choose TCP, UDP or any protocol.",
    acl_problem_range: "An address or port is invalid: {problem}.",
    acl_problem_unreadable: "EasyTier's access control doesn't define this value.",
};

pub(super) static ZH_CN: ConfigStrings = ConfigStrings {
    section_flags: "行为",
    flag_group_routing: "路由与直连",
    flag_group_traversal: "NAT 穿透",
    flag_group_transport: "KCP 与 QUIC",
    flag_group_system: "网络栈与系统",

    flag_latency_first: "优先选择延迟最低的路径",
    flag_latency_first_help: "按总延迟而不是跳数来选择路径。",
    flag_disable_p2p: "不主动建立直连",
    flag_disable_p2p_help: "本节点不再主动发起直连（P2P）。要求直连的对等节点仍可与本节点建立直连。",
    flag_p2p_only: "仅使用直连",
    flag_p2p_only_help: "只与已建立直连的对等节点通信，不经中继转发。",
    flag_lazy_p2p: "按需建立直连",
    flag_lazy_p2p_help: "有流量发往某个对等节点时才尝试直连。要求直连的对等节点仍会立即连接。",
    flag_need_p2p: "要求对等节点直连",
    flag_need_p2p_help: "即使对方设为按需建立直连，也会立刻与本节点建立直连。",
    flag_bind_device: "仅使用物理网卡",
    flag_bind_device_help: "避免 EasyTier 经由其他 VPN 等虚拟网络建立连接。",
    flag_disable_tcp_hole_punching: "关闭 TCP 打洞",
    flag_disable_tcp_hole_punching_help: "寻找直连时不再尝试 TCP 打洞。",
    flag_disable_udp_hole_punching: "关闭 UDP 打洞",
    flag_disable_udp_hole_punching_help: "寻找直连时不再尝试 UDP 打洞。",
    flag_disable_sym_hole_punching: "关闭对称 NAT 打洞",
    flag_disable_sym_hole_punching_help: "不再对对称 NAT 使用生日攻击式打洞，按锥形 NAT 处理。",
    flag_disable_upnp: "关闭 UPnP 与 NAT-PMP",
    flag_disable_upnp_help: "不再请求路由器为本节点的监听器映射端口。端口映射默认开启。",
    flag_enable_kcp_proxy: "用 KCP 承载 TCP",
    flag_enable_kcp_proxy_help: "在对等节点之间把 TCP 流量转为 KCP，可降低延迟、提高速度。",
    flag_disable_kcp_input: "拒绝接收 KCP",
    flag_disable_kcp_input_help: "使用 KCP 代理的对等节点改用 TCP 连接本节点。",
    flag_enable_quic_proxy: "用 QUIC 承载 TCP",
    flag_enable_quic_proxy_help: "在对等节点之间把 TCP 流量转为 QUIC，可降低延迟、提高速度。",
    flag_disable_quic_input: "拒绝接收 QUIC",
    flag_disable_quic_input_help: "使用 QUIC 代理的对等节点改用 TCP 连接本节点。",
    flag_multi_thread: "使用多线程",
    flag_multi_thread_help: "把内核的工作分摊到更多处理器核心。性能较弱的设备上，单线程占用的资源更少。",
    flag_use_smoltcp: "使用用户态协议栈",
    flag_use_smoltcp_help: "子网代理以及 KCP、QUIC 代理改用 EasyTier 自带的 TCP/IP 协议栈，避免被操作系统防火墙拦截。",
    flag_disable_ipv6: "关闭 IPv6",
    flag_disable_ipv6_help: "本节点在网络内只使用 IPv4。",
    flag_ipv6_public_addr_auto: "自动获取公网 IPv6 地址",
    flag_ipv6_public_addr_auto_help: "从共享了 IPv6 子网的对等节点获取一个公网 IPv6 地址。",
    flag_accept_dns: "解析节点名称（魔法 DNS）",
    flag_accept_dns_help: "通过 EasyTier 自带的域名服务，用主机名访问其他节点，例如 node1.et.net。",
    flag_proxy_forward_by_system: "由系统内核转发",
    flag_proxy_forward_by_system_help: "被代理子网的数据包交给操作系统转发，不再使用 EasyTier 内置的 NAT。",
    flag_enable_udp_broadcast_relay: "中继 UDP 广播",
    flag_enable_udp_broadcast_relay_help: "仅限 Windows：把本机的 UDP 广播转发给对等节点，让局域网游戏能发现房间。需要管理员权限。",
    flag_forward_by_system_exposure: "内核会绕过 EasyTier 的 NAT 转发被代理子网的流量，ACL 的转发链对这些流量也不再生效。仅在你自己管理本机防火墙时开启。",

    vpn_portal: "WireGuard 门户",
    vpn_portal_help: "普通 WireGuard 客户端无需安装 EasyTier，即可通过本节点加入网络。",
    vpn_portal_cidr: "客户端子网",
    vpn_portal_cidr_help: "分配给 WireGuard 客户端的地址段，例如 10.14.14.0/24，不能与网络自身的地址重叠。",
    vpn_portal_port: "监听端口",
    vpn_portal_exposure: "门户会监听本设备的所有网络接口，而不只是虚拟网卡。任何能访问该 UDP 端口的人都可以尝试以 WireGuard 客户端身份连接。",
    mapped_listeners: "监听映射",
    mapped_listeners_help: "如果对等节点能通过另一个地址从外部访问本节点（例如路由器上转发的端口），在此填写该地址，对方会优先使用。每行一个地址。",
    manual_routes: "手动路由",
    manual_routes_help: "由你自己指定路由。这会关闭子网代理，以及对等节点传播的 WireGuard 路由。列表为空时不会添加任何路由。",
    manual_routes_field: "路由，每行一个",
    recv_limit: "接收限速",
    recv_limit_help: "限制本节点上该网络的总接收速率，单位为字节每秒。留空表示不限速。",
    recv_limit_placeholder: "不限速",
    port_forwards_help: "每行一条：协议（tcp 或 udp）、本机地址和端口，然后是网络内的目标地址和端口。",
    section_acl: "访问控制",

    acl_switch: "访问控制（ACL）",
    acl_switch_help: "按方向、地址、端口、协议和组来允许或丢弃流量。没有规则时，一切流量都被允许。",
    acl_off_title: "关闭访问控制？",
    acl_off_body: "这里设置的规则链和组会从此配置中移除。",
    acl_off_action: "关闭",
    acl_empty_title: "还没有规则链",
    acl_empty_body: "一条规则链管理一个方向的流量：发往本节点的入站、本节点发出的出站，或转发到所代理子网的流量。添加之前，一切流量都被允许。",
    acl_chains: "规则链",
    acl_add_inbound: "添加入站链",
    acl_add_outbound: "添加出站链",
    acl_add_forward: "添加转发链",
    acl_kind_inbound: "入站",
    acl_kind_outbound: "出站",
    acl_kind_forward: "转发",
    acl_kind_inbound_help: "发往本节点的流量。",
    acl_kind_outbound_help: "本节点发出的流量。",
    acl_kind_forward_help: "本节点转发到所代理子网的流量。",
    acl_chain_name: "名称",
    acl_chain_description: "描述",
    acl_chain_enabled: "启用此链",
    acl_chain_kind: "适用流量",
    acl_chain_default: "没有规则匹配时",
    acl_chain_drops_all: "这条链会丢弃所有未被规则允许的数据包。请确认有允许规则覆盖你需要的流量。",
    acl_allow: "允许",
    acl_drop: "丢弃",
    acl_chain_delete: "删除规则链",
    acl_chain_delete_title: "删除规则链“{name}”？",
    acl_chain_delete_body: "该链及其所有规则将从此配置中移除。",
    acl_rules: "规则",
    acl_rules_order: "规则按优先级从高到低依次检查。",
    acl_rules_empty: "没有规则：一切由默认动作决定。",
    acl_rule_add: "添加规则",
    acl_rule_edit: "编辑规则",
    acl_rule_delete: "删除规则",
    acl_rule_untitled: "规则 {n}",
    acl_rule_priority_chip: "优先级 {n}",
    acl_rule_disabled: "已停用",
    acl_rule_name: "名称",
    acl_rule_description: "描述",
    acl_rule_enabled: "启用此规则",
    acl_rule_action: "动作",
    acl_rule_protocol: "协议",
    acl_protocol_any: "任意",
    acl_rule_source_ips: "来源地址",
    acl_rule_destination_ips: "目的地址",
    acl_rule_ips_help: "IP 地址或网段，用逗号或换行分隔。留空表示任意。",
    acl_rule_source_ports: "来源端口",
    acl_rule_destination_ports: "目的端口",
    acl_rule_ports_help: "端口或端口范围，用逗号或换行分隔。留空表示任意。",
    acl_rule_no_ports: "该协议没有端口。",
    acl_rule_priority: "优先级",
    acl_rule_priority_help: "0 到 65535，数字越大越先检查。",
    acl_rule_stateful: "跟踪连接",
    acl_rule_stateful_help: "规则放行的连接，其回包也会放行。",
    acl_rule_rate_limit: "速率限制（包/秒）",
    acl_rule_burst_limit: "突发限制（包）",
    acl_rule_limit_help: "留空或填 0 表示不限制。",
    acl_rule_source_groups: "来源组",
    acl_rule_destination_groups: "目的组",
    acl_rule_groups_help: "只匹配属于所选组的对等节点。一个都不选则匹配任意节点。",
    acl_rule_no_groups: "还没有组。先在“组”中声明一个，才能按组匹配对等节点。",
    acl_rule_fix_fields: "修正标出的字段后才能保存这条规则。",
    acl_entry_error: "第 {n} 项：{problem}",
    acl_number_error: "请输入 0 到 {max} 之间的整数。",
    acl_range_empty: "内容为空",
    acl_range_bad_ip: "不是 IP 地址，也不是 10.144.144.0/24 这样的网段",
    acl_range_host_bits: "地址在前缀之外还有非零位，请写成网络地址",
    acl_range_bad_port: "不是端口，也不是 1000-2000 这样的范围",
    acl_groups: "组",
    acl_groups_help: "组把规则与对等节点联系起来：节点用组密钥证明自己属于某个组，规则可以按组匹配。",
    acl_declared: "声明的组",
    acl_declared_help: "本节点知道其密钥的组。",
    acl_groups_empty: "尚未声明任何组。",
    acl_group_add: "添加组",
    acl_group_edit: "编辑组",
    acl_group_delete: "删除组",
    acl_group_name: "组名",
    acl_group_secret: "组密钥",
    acl_group_secret_help: "组内每个节点都需要相同的密钥。请像网络密钥一样妥善保管。",
    acl_group_secret_set: "已设置密钥",
    acl_group_secret_missing: "没有密钥",
    acl_group_generate_failed: "这台设备无法生成密钥，请手动输入。",
    acl_group_delete_title: "删除组“{name}”？",
    acl_group_delete_body: "规则将不再按该组匹配（涉及 {count} 条规则），只剩这一个组的规则会匹配所有对等节点。本节点也会退出该组。",
    acl_joined: "加入的组",
    acl_joined_help: "本节点所属的组。其他节点上的规则可以据此匹配本节点。",
    acl_joined_none: "先声明一个组，才能加入。",
    acl_joined_undeclared: "本节点未声明",
    acl_joined_remove: "退出该组",
    acl_place_acl: "访问控制",
    acl_place_chain: "第 {chain} 条规则链",
    acl_place_rule: "第 {chain} 条规则链的第 {rule} 条规则",
    acl_place_declared: "第 {group} 个声明的组",
    acl_place_joined: "第 {group} 个加入的组",
    acl_error_line: "{place}：{problem}",
    acl_problem_too_many: "条目过多，最多 {max} 项。",
    acl_problem_too_long: "内容过长，最多 {max} 个字符。",
    acl_problem_bad_name: "名称为空或含有控制字符。",
    acl_problem_duplicate: "名称重复。",
    acl_problem_missing_secret: "没有设置密钥，每个声明的组都需要密钥。",
    acl_problem_priority: "优先级超过了 {max}。",
    acl_problem_ports: "该协议的规则不能匹配端口。请清空端口，或改用 TCP、UDP 或任意协议。",
    acl_problem_range: "地址或端口无效：{problem}。",
    acl_problem_unreadable: "EasyTier 的访问控制不支持这个值。",
};
