//! The peer drawer: one peer's facts, its latency and packet loss over the last minutes, and the network it belongs to.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    common::{copy_labels, icon_label_button, nat_word},
    format::{bytes_text, ip_text, latency_text, loss_text, rate_text},
    peers::{route_lamp, route_word},
};
use crate::{
    components::{
        ButtonKind, CalloutKind, ChartUnit, Icon, SeriesColor, body_text, callout, chart_series,
        drawer, fact, fact_mono, kv_grid, metric_chart,
    },
    core::state::{NatType, PeerSummary, RouteKind},
    profile::ProfileId,
    store::{
        Store,
        node::{DetailTab, HISTORY_SPAN_MS, PeerRates, PeerRef, TimedSamples},
    },
};

/// The drawer, on screen while the node store names a peer.
pub(super) fn drawer_view(store: Store) -> AnyElement {
    let node = store.node();
    let current = move || node.open_peer_ref();
    let peer = move || current().and_then(|reference| node.peer(&reference));
    let route = move || peer().map_or(RouteKind::Unknown, |peer| peer.route);
    drawer(
        move || node.open_peer_ref().is_some(),
        move || {
            SharedString::from(
                peer()
                    .map(|peer| {
                        if peer.hostname.is_empty() {
                            peer.peer_id.to_string()
                        } else {
                            peer.hostname
                        }
                    })
                    .unwrap_or_default(),
            )
        },
        store.tr(|s| s.node.peers.drawer_close),
        move || node.close_peer(),
    )
    .kind(store.tr(|s| s.kind_peer))
    .status(
        move || route_lamp(route()),
        move || SharedString::from(route_word(store, route())),
    )
    .meta(move || facts(store))
    .body(move || body(store))
    .test_id("peer-drawer")
    .build()
}

/// A text of the open peer, empty (a dash) once it is gone.
fn read(store: Store, pick: fn(&PeerSummary) -> String) -> impl Fn() -> String + Copy + 'static {
    let node = store.node();
    move || {
        node.open_peer_ref()
            .and_then(|reference| node.peer(&reference))
            .map(|peer| pick(&peer))
            .unwrap_or_default()
    }
}

/// The facts: address, id, network, route, latency, loss, traffic now and in total, tunnel, NAT type and version.
fn facts(store: Store) -> View {
    let node = store.node();
    let words = store.strings().node.peers;
    let rate = move |pick: fn(PeerRates) -> Option<f64>| {
        move || {
            node.open_peer_ref()
                .and_then(|reference| {
                    pick(node.latest_peer_rates(&reference.network, reference.peer_id))
                })
                .map_or_else(String::new, rate_text)
        }
    };
    let route = move || {
        node.open_peer_ref()
            .and_then(|reference| node.peer(&reference))
            .map(|peer| route_word(store, peer.route))
            .unwrap_or_default()
    };
    let network = move || {
        node.open_peer_ref()
            .and_then(|reference| node.profile(&reference.network))
            .map(|profile| profile.name)
            .unwrap_or_default()
    };
    let nat = move || {
        node.open_peer_ref()
            .and_then(|reference| node.peer(&reference))
            .map(|peer| match peer.nat_type {
                NatType::Unknown => String::new(),
                nat => nat_word(store, nat).to_owned(),
            })
            .unwrap_or_default()
    };
    kv_grid(vec![
        fact_mono(
            words.label_ip,
            read(store, |peer| ip_text(peer.virtual_ipv4)),
        )
        .copyable(copy_labels(store))
        .test_id("peer-ip"),
        fact_mono(
            words.label_peer_id,
            read(store, |peer| peer.peer_id.to_string()),
        ),
        fact(words.label_network, network).test_id("peer-network"),
        fact(words.col_route, route),
        fact_mono(
            words.col_latency,
            read(store, |peer| {
                peer.latency_ms
                    .map_or_else(String::new, |ms| latency_text(Some(ms)))
            }),
        ),
        fact_mono(
            words.col_loss,
            read(store, |peer| {
                peer.loss
                    .map_or_else(String::new, |loss| loss_text(Some(loss)))
            }),
        ),
        fact_mono(words.col_rx, rate(|rates| rates.rx_bps)).test_id("peer-rx-rate"),
        fact_mono(words.col_tx, rate(|rates| rates.tx_bps)).test_id("peer-tx-rate"),
        fact_mono(
            words.label_rx_total,
            read(store, |peer| {
                peer.rx_bytes
                    .map_or_else(String::new, |b| bytes_text(Some(b)))
            }),
        ),
        fact_mono(
            words.label_tx_total,
            read(store, |peer| {
                peer.tx_bytes
                    .map_or_else(String::new, |b| bytes_text(Some(b)))
            }),
        ),
        fact_mono(
            words.col_tunnel,
            read(store, |peer| peer.tunnel.clone().unwrap_or_default()),
        ),
        fact(words.col_nat, nat),
        fact_mono(
            words.col_version,
            read(store, |peer| peer.version.clone().unwrap_or_default()),
        ),
    ])
}

/// The body: the peer's latency and loss over time, and the way to its network. A peer that left says so.
fn body(store: Store) -> View {
    let node = store.node();
    let here = computed(move || {
        node.open_peer_ref()
            .is_some_and(|reference| node.peer(&reference).is_some())
    });
    view().gap_6().child(dynamic(move || {
        if !here.get() {
            return callout(CalloutKind::Info, store.tr(|s| s.node.peers.drawer_gone))
                .test_id("peer-gone")
                .build()
                .into_element();
        }
        untracked(move || charts(store)).into_element()
    }))
}

/// The two charts and the link to the network.
fn charts(store: Store) -> View {
    let node = store.node();
    let samples = move |pick: fn(&crate::store::NodeStore, &ProfileId, u32) -> TimedSamples| {
        computed(move || match node.open_peer_ref() {
            Some(PeerRef { network, peer_id }) => pick(&node, &network, peer_id),
            None => TimedSamples::default(),
        })
    };
    let latency = samples(|node, network, peer_id| node.peer_latency_samples(network, peer_id));
    let loss = samples(|node, network, peer_id| node.peer_loss_samples(network, peer_id));
    let heading = |label: fn(&'static crate::i18n::Strings) -> &'static str| {
        body_text(store.tr(label))
            .font_weight(FontWeight::MEDIUM)
            .role(AccessibilityRole::Header)
            .text_color(move || theme().foreground)
    };
    let network = move || node.open_peer_ref().map(|reference| reference.network);
    view()
        .gap_6()
        .child(
            view()
                .gap_3()
                .child(heading(|s| s.node.peers.chart_peer_latency))
                .child(
                    metric_chart(
                        store.tr(|s| s.node.peers.chart_peer_latency),
                        move || latency.with(|samples| samples.t.clone()),
                        vec![chart_series(
                            store.tr(|s| s.node.peers.col_latency),
                            SeriesColor::Chart(3),
                            move || latency.with(|samples| samples.values.clone()),
                        )],
                        ChartUnit::Milliseconds,
                        store.tr(|s| s.node.peers.chart_collecting),
                    )
                    .span(
                        HISTORY_SPAN_MS,
                        store.tr(|s| s.node.peers.chart_span_from),
                        store.tr(|s| s.node.peers.chart_span_to),
                    )
                    .test_id("peer-latency"),
                ),
        )
        .child(
            view()
                .gap_3()
                .child(heading(|s| s.node.peers.chart_peer_loss))
                .child(
                    metric_chart(
                        store.tr(|s| s.node.peers.chart_peer_loss),
                        move || loss.with(|samples| samples.t.clone()),
                        vec![chart_series(
                            store.tr(|s| s.node.peers.col_loss),
                            SeriesColor::Chart(4),
                            move || loss.with(|samples| samples.values.clone()),
                        )],
                        ChartUnit::Percent,
                        store.tr(|s| s.node.peers.chart_collecting),
                    )
                    .span(
                        HISTORY_SPAN_MS,
                        store.tr(|s| s.node.peers.chart_span_from),
                        store.tr(|s| s.node.peers.chart_span_to),
                    )
                    .test_id("peer-loss"),
                ),
        )
        .child(view().flex_row().child(super::common::tag(
            icon_label_button(
                ButtonKind::Secondary,
                Icon::Network,
                store.tr(|s| s.node.peers.open_network),
                move || {
                    if let Some(id) = network() {
                        node.open_detail(id, DetailTab::Overview);
                    }
                },
            ),
            "peer-open-network",
        )))
}
