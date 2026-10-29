//! The Node tab: what one running network's node says about itself: its virtual IP and identity, how STUN sees its
//! NAT, the addresses it listens on and is seen at, the subnets it proxies, and its feature flags.

use nsh_core::prelude::*;
use uuid::Uuid;

use super::{Pick, clock, feed_view, format_ago, heading, t, toolbar};
use crate::{
    components::{CopyLabels, fact, fact_mono, kv_grid, machine_text, meta_text, section_label},
    core::{
        remote::{NodeFacts, PeerFeatures},
        state::NatType,
        target::TargetId,
    },
    store::{Store, console::Feed},
};

/// A feature flag: its label and how to read it.
type Flag = (Pick, fn(&PeerFeatures) -> bool);

/// The feature flags the grid lists.
const FLAGS: [Flag; 11] = [
    (|s| s.flag_public_server, |f| f.is_public_server),
    (|s| s.flag_avoid_relay_data, |f| f.avoid_relay_data),
    (|s| s.flag_kcp_input, |f| f.kcp_input),
    (|s| s.flag_no_relay_kcp, |f| f.no_relay_kcp),
    (|s| s.flag_conn_list_sync, |f| f.support_conn_list_sync),
    (|s| s.flag_quic_input, |f| f.quic_input),
    (|s| s.flag_no_relay_quic, |f| f.no_relay_quic),
    (|s| s.flag_credential_peer, |f| f.is_credential_peer),
    (|s| s.flag_need_p2p, |f| f.need_p2p),
    (|s| s.flag_disable_p2p, |f| f.disable_p2p),
    (|s| s.flag_ipv6_provider, |f| f.ipv6_public_addr_provider),
];

/// One running network's node: refresh, and its facts.
pub(in crate::pages::console) fn node_tab(
    store: Store,
    id: TargetId,
    mid: Uuid,
    instance: Uuid,
) -> AnyElement {
    let console = store.console();
    let feed = move || console.inspect_feed(id, mid, instance, |views| &views.node);
    let refresh = move || console.refresh_node(id, mid, instance);
    refresh();
    view()
        .gap_4()
        .child(toolbar(
            store,
            move || feed().read_at_ms,
            refresh,
            "machine-node-refresh",
        ))
        .child(feed_view(
            store,
            feed,
            |_| false,
            refresh,
            || view().into_element(),
            move || facts_view(store, feed),
        ))
        .into_element()
}

/// The facts of the node read last.
fn facts_view(store: Store, feed: impl Fn() -> Feed<NodeFacts> + Copy + 'static) -> AnyElement {
    let node = computed(move || feed().value.unwrap_or_default());
    let now = clock();
    let copy = move || CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied));
    let read = move |pick: fn(&NodeFacts) -> String| move || node.with(pick);
    let identity = kv_grid(vec![
        fact_mono(
            t(store, |s| s.node_virtual_ip),
            read(|node| {
                node.virtual_ipv4
                    .map(|ip| ip.to_string())
                    .unwrap_or_default()
            }),
        )
        .copyable(copy())
        .test_id("node-virtual-ip"),
        fact_mono(
            t(store, |s| s.node_peer_id),
            read(|node| node.peer_id.to_string()),
        )
        .copyable(copy())
        .test_id("node-peer-id"),
        fact(
            t(store, |s| s.node_hostname),
            read(|node| node.hostname.clone()),
        ),
        fact_mono(
            t(store, |s| s.node_version),
            read(|node| node.version.clone()),
        ),
    ]);
    let connectivity = kv_grid(vec![
        fact(t(store, |s| s.node_udp_nat), move || {
            nat_name(store, node.with(|node| node.stun.udp_nat_type))
        })
        .test_id("node-udp-nat"),
        fact(t(store, |s| s.node_tcp_nat), move || {
            nat_name(store, node.with(|node| node.stun.tcp_nat_type))
        })
        .test_id("node-tcp-nat"),
        fact_mono(
            t(store, |s| s.node_port_range),
            read(|node| match node.stun.port_range {
                Some((low, high)) if low == high => low.to_string(),
                Some((low, high)) => format!("{low}\u{2013}{high}"),
                None => String::new(),
            }),
        ),
        fact(t(store, |s| s.node_nat_checked), move || {
            node.with(|node| node.stun.updated_at_ms)
                .filter(|ms| *ms > 0)
                .map(|ms| format_ago(store.strings(), now.get(), ms as f64))
                .unwrap_or_default()
        }),
    ]);
    let flags = kv_grid(
        FLAGS
            .iter()
            .map(|&(label, on)| {
                fact(t(store, label), move || {
                    let words = store.strings().console_inspect;
                    if node.with(|node| on(&node.features)) {
                        words.flag_yes
                    } else {
                        words.flag_no
                    }
                })
            })
            .collect(),
    );
    view()
        .gap_6()
        .child(
            view()
                .gap_3()
                .child(heading(t(store, |s| s.node_h_identity)))
                .child(identity),
        )
        .child(
            view()
                .gap_3()
                .child(heading(t(store, |s| s.node_h_nat)))
                .child(connectivity)
                .child(meta_text(t(store, |s| s.node_nat_hint))),
        )
        .child(
            view()
                .gap_3()
                .child(heading(t(store, |s| s.node_h_addresses)))
                .child(list(t(store, |s| s.node_listeners), move || {
                    node.with(|node| node.listeners.clone())
                }))
                .child(list(t(store, |s| s.node_proxy_cidrs), move || {
                    node.with(|node| node.proxy_cidrs.clone())
                }))
                .child(list(t(store, |s| s.node_public_addresses), move || {
                    node.with(public_addresses)
                }))
                .child(list(t(store, |s| s.node_interface_addresses), move || {
                    node.with(|node| node.interface_ips.iter().map(ToString::to_string).collect())
                })),
        )
        .child(
            view()
                .gap_3()
                .child(heading(t(store, |s| s.node_h_features)))
                .child(flags),
        )
        .into_element()
}

/// The addresses the node is seen at from outside: what STUN saw, then what the core knows besides.
fn public_addresses(node: &NodeFacts) -> Vec<String> {
    let mut all: Vec<String> = node
        .stun
        .public_ips
        .iter()
        .map(ToString::to_string)
        .collect();
    let extra = [
        node.public_ipv4.map(|ip| ip.to_string()),
        node.public_ipv6.map(|ip| ip.to_string()),
        node.public_ipv6_addr.map(|net| net.to_string()),
    ];
    for address in extra.into_iter().flatten() {
        if !all.contains(&address) {
            all.push(address);
        }
    }
    all
}

/// A label over the `items`, one per line, or a muted dash for none.
fn list(label: impl IntoReactive<SharedString>, items: impl Fn() -> Vec<String> + 'static) -> View {
    view()
        .gap_1()
        .child(section_label(label))
        .child(dynamic(move || {
            let items = items();
            if items.is_empty() {
                return meta_text("\u{2014}").into_element();
            }
            view()
                .gap_1()
                .child(
                    items
                        .into_iter()
                        .map(|item| {
                            machine_text(SharedString::from(item))
                                .number_of_lines(1)
                                .selectable(true)
                                .into_element()
                        })
                        .collect::<Vec<_>>(),
                )
                .into_element()
        }))
}

/// A NAT type in plain words.
fn nat_name(store: Store, nat: NatType) -> &'static str {
    let words = store.strings().node.detail;
    match nat {
        NatType::Unknown => words.nat_unknown,
        NatType::OpenInternet => words.nat_open_internet,
        NatType::NoPat => words.nat_no_pat,
        NatType::FullCone => words.nat_full_cone,
        NatType::Restricted => words.nat_restricted,
        NatType::PortRestricted => words.nat_port_restricted,
        NatType::Symmetric => words.nat_symmetric,
        NatType::SymUdpFirewall => words.nat_sym_udp_firewall,
        NatType::SymmetricEasyInc => words.nat_symmetric_easy_inc,
        NatType::SymmetricEasyDec => words.nat_symmetric_easy_dec,
    }
}
