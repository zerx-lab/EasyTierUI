//! The Peers page: a toolbar (network, search, route filter, the age of the reading), a summary strip, the charts of
//! throughput and latency, and a table of the peers of the running networks. A row opens the peer's drawer. A peer
//! list never appears before the core has answered.

use nsh_core::prelude::*;

use super::{
    common::{lamp_of, nat_word, responsive, status_word, tag},
    failure,
    format::{LOSS_BAD, LOSSY, SLOW_LATENCY_MS, address_text, latency_text, loss_text, rate_text},
    panels::{self, Scope},
};
use crate::{
    components::{
        ButtonKind, Columns, Icon, Lamp, SelectOption, Thresholds, action_button, age_text, column,
        columns, data_table, empty_panel, live_dot, meta_text, search_field, segmented_inline,
        select, status_chip, status_lamp, summary_cell, summary_strip, table_row, threshold_bar,
        toolbar, toolbar_spacer,
    },
    core::state::{NatType, RouteKind},
    i18n::fill,
    profile::ProfileId,
    store::{
        Page, Store,
        node::{PeerRow, PeerStats, RouteFilter, Status},
    },
};

/// Latency, as a bar: amber from [`SLOW_LATENCY_MS`], full at three times that, never red.
const LATENCY_BAR: Thresholds = Thresholds {
    warn: SLOW_LATENCY_MS,
    bad: None,
    full: SLOW_LATENCY_MS * 3.0,
};

/// Loss, as a bar: amber from [`LOSSY`], red from [`LOSS_BAD`], full at 25 %.
const LOSS_BAR: Thresholds = Thresholds {
    warn: LOSSY,
    bad: Some(LOSS_BAD),
    full: 0.25,
};

/// The page.
pub(super) fn page(store: Store) -> View {
    let node = store.node();
    view().gap_6().child(
        when(
            move || node.running_ids().is_empty(),
            move || nothing_running(store),
        )
        .otherwise(move || running(store)),
    )
}

/// No network answers: the networks that are on their way up or down or failed, then where to start one.
fn nothing_running(store: Store) -> View {
    let node = store.node();
    view().gap_6().child(waiting(store)).child(
        when(
            move || node.active_ids().is_empty(),
            move || {
                empty_panel(
                    Icon::Peers,
                    store.tr(|s| s.peers_empty_title),
                    store.tr(|s| s.peers_empty_body),
                    Some(
                        tag(
                            action_button(
                                ButtonKind::Secondary,
                                store.tr(|s| s.node.peers.open_networks),
                                move || store.open(Page::Networks),
                            ),
                            "peers-open-networks",
                        )
                        .into_element(),
                    ),
                )
            },
        )
        .otherwise(move || {
            tag(
                action_button(
                    ButtonKind::Secondary,
                    store.tr(|s| s.node.peers.open_networks),
                    move || store.open(Page::Networks),
                ),
                "peers-open-networks",
            )
        }),
    )
}

/// The networks that are not running but not stopped either: what they wait for, or why they failed.
fn waiting(store: Store) -> View {
    let node = store.node();
    view().gap_3().child(each(
        move || {
            node.active_ids()
                .into_iter()
                .filter(|id| node.status(id) != Status::Running)
                .collect::<Vec<_>>()
        },
        |id: &ProfileId| id.clone(),
        move |id, _| waiting_note(store, id.get_untracked()),
    ))
}

/// One such network: its name and lamp, and what it waits for.
fn waiting_note(store: Store, id: ProfileId) -> View {
    let node = store.node();
    let status = {
        let id = id.clone();
        computed(move || node.status(&id))
    };
    let name = {
        let id = id.clone();
        move || node.profile(&id).map(|p| p.name).unwrap_or_default()
    };
    let header = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(
            crate::components::body_text(name)
                .font_weight(FontWeight::MEDIUM)
                .role(AccessibilityRole::Header),
        )
        .child(status_lamp(
            move || lamp_of(&status.get()),
            move || status_word(store.strings(), &status.get()),
        ));
    let body = dynamic(move || {
        let id = id.clone();
        match status.get() {
            Status::Starting => meta_text(store.tr(|s| s.node.peers.waiting)).into_element(),
            Status::Stopping | Status::Removing => {
                meta_text(store.tr(|s| s.node.peers.stopping)).into_element()
            }
            Status::Failed(found) => view()
                .gap_2()
                .child(failure::panel(store, id, found))
                .child(meta_text(store.tr(|s| s.node.peers.start_from_networks)))
                .into_element(),
            Status::Running | Status::Stopped => view().into_element(),
        }
    });
    view().gap_2().child(header).child(body)
}

/// The page while a network answers.
fn running(store: Store) -> View {
    view()
        .gap_6()
        .child(toolbar_row(store))
        .child(waiting(store))
        .child(summary(store))
        .child(panels::charts_panel(store, scope(store), "peers"))
        .child(table(store))
}

/// The networks the page covers now.
fn scope(store: Store) -> Scope {
    let node = store.node();
    std::rc::Rc::new(move || node.peers_scope())
}

// ----- the toolbar -----

/// The network, a search and the route filter on the left; the age of the reading on the right.
fn toolbar_row(store: Store) -> View {
    let node = store.node();
    let words = store.strings().node.peers;
    let routes = segmented_inline(
        store.tr(|s| s.node.peers.route_label),
        vec![
            SelectOption::new(RouteFilter::All.key(), words.filter_all),
            SelectOption::new(RouteFilter::Direct.key(), words.filter_direct),
            SelectOption::new(RouteFilter::Relayed.key(), words.filter_relayed),
        ],
        move || SharedString::from(node.peers_route().key()),
        move |picked| {
            if let Some(route) = RouteFilter::from_key(picked) {
                node.set_peers_route(route);
            }
        },
        "peers-route",
    );
    let running = computed(move || node.running_ids());
    let networks = dynamic(move || {
        let ids = running.get();
        untracked(move || network_select(store, ids))
    });
    let read_at = move || {
        node.peers_scope()
            .iter()
            .filter_map(|id| node.snapshot(id).map(|snapshot| snapshot.read_at_ms))
            .max_by(f64::total_cmp)
    };
    toolbar()
        .child(networks)
        .child(
            search_field(
                store.tr(|s| s.node.peers.search_placeholder),
                move || node.peers_query(),
                move |query| node.set_peers_query(query),
            )
            .clear_label(store.tr(|s| s.node.peers.search_clear))
            .test_id("peers-search"),
        )
        .child(routes)
        .child(toolbar_spacer())
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(live_dot(move || !node.running_ids().is_empty()))
                .child(age_text(
                    read_at,
                    store.tr(|s| s.node.peers.updated_ago),
                    store.tr(|s| s.common.updated_now),
                )),
        )
}

/// The network selector: every running network, or one.
fn network_select(store: Store, ids: Vec<ProfileId>) -> View {
    let node = store.node();
    let mut options = vec![SelectOption::new(
        "all",
        store.strings().node.peers.network_all,
    )];
    for id in &ids {
        let name = node.profile(id).map(|p| p.name).unwrap_or_default();
        options.push(SelectOption::new(id.to_string(), name));
    }
    view().child(
        select(
            store.tr(|s| s.node.peers.network_label),
            options,
            move || match node.peers_network() {
                Some(id) => SharedString::from(id.to_string()),
                None => SharedString::from("all"),
            },
            move |picked| {
                let chosen = node
                    .running_ids()
                    .into_iter()
                    .find(|id| id.to_string() == picked);
                node.set_peers_network(chosen);
            },
        )
        .inline()
        .test_id("peers-network"),
    )
}

// ----- the summary strip -----

/// Which optional cells of the strip have data. The strip is made again when this changes, not at every poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shape {
    latency: bool,
    loss: bool,
    own_ip: bool,
}

/// The figures of the peers in scope.
fn stats(store: Store) -> PeerStats {
    let rows = store.node().scope_peers();
    PeerStats::of(rows.iter().map(|row| &row.peer))
}

/// This node's own address when exactly one network is in scope and has one.
fn own_address(store: Store) -> Option<String> {
    let node = store.node();
    match node.peers_scope().as_slice() {
        [only] => node
            .snapshot(only)
            .and_then(|snapshot| snapshot.node.virtual_ipv4)
            .map(|net| net.addr().to_string()),
        _ => None,
    }
}

/// Peers online with how they are reached, the median latency, the worst loss and (for one network) this node's
/// address.
fn summary(store: Store) -> View {
    let shape = computed(move || {
        let stats = stats(store);
        Shape {
            latency: stats.median_latency_ms.is_some(),
            loss: stats.worst_loss.is_some(),
            own_ip: own_address(store).is_some(),
        }
    });
    view().child(dynamic(move || {
        let shape = shape.get();
        untracked(move || strip(store, shape))
    }))
}

/// The strip, its values following the store.
fn strip(store: Store, shape: Shape) -> View {
    let words = store.strings().node.peers;
    let mut cells = vec![
        summary_cell(words.summary_online, move || {
            stats(store).mix.online.to_string()
        })
        .bar(
            Lamp::Good,
            move || stats(store).mix.direct.to_string(),
            store.tr(|s| s.node.peers.summary_direct),
        )
        .bar(
            Lamp::Degraded,
            move || stats(store).mix.relayed.to_string(),
            store.tr(|s| s.node.peers.summary_relayed),
        )
        .test_id("summary-online"),
    ];
    if shape.latency {
        cells.push(
            summary_cell(words.summary_latency, move || {
                let ms = stats(store).median_latency_ms.unwrap_or_default();
                if ms < 10.0 {
                    format!("{ms:.1}")
                } else {
                    format!("{ms:.0}")
                }
            })
            .unit("ms")
            .caption(move || {
                fill(
                    store.strings().node.peers.summary_latency_of,
                    &[("count", &stats(store).measured.to_string())],
                )
            })
            .test_id("summary-latency"),
        );
    }
    if shape.loss {
        cells.push(
            summary_cell(words.summary_loss, move || {
                format!("{:.1}", stats(store).worst_loss.unwrap_or_default() * 100.0)
            })
            .unit("%")
            .test_id("summary-loss"),
        );
    }
    if shape.own_ip {
        cells.push(
            summary_cell(words.your_ip, move || {
                own_address(store).unwrap_or_default()
            })
            .test_id("peers-own-ip"),
        );
    }
    summary_strip(cells)
}

// ----- the table -----

/// A peer, keyed by its network and its id: a row stays while its figures change.
fn key(row: &PeerRow) -> (ProfileId, u32) {
    (row.network.clone(), row.peer.peer_id)
}

/// The table of peers, with its empty states.
fn table(store: Store) -> View {
    let node = store.node();
    let words = store.strings().node.peers;
    let latency_hint = fill(
        words.latency_hint,
        &[("warn", &format!("{SLOW_LATENCY_MS:.0}"))],
    );
    let loss_hint = fill(
        words.loss_hint,
        &[
            ("warn", &format!("{:.0}", LOSSY * 100.0)),
            ("bad", &format!("{:.0}", LOSS_BAD * 100.0)),
        ],
    );
    let cols = columns(vec![
        column(store.tr(|s| s.node.peers.col_host)).flex(),
        column(store.tr(|s| s.node.peers.col_ip))
            .width(124.0)
            .mono()
            .right(),
        column(store.tr(|s| s.node.peers.col_route)).width(140.0),
        column(store.tr(|s| s.node.peers.col_latency))
            .width(128.0)
            .hint(latency_hint),
        column(store.tr(|s| s.node.peers.col_loss))
            .width(120.0)
            .hint(loss_hint),
        responsive(
            column(store.tr(|s| s.node.peers.col_rx))
                .width(96.0)
                .mono()
                .right(),
            1150.0,
        ),
        responsive(
            column(store.tr(|s| s.node.peers.col_tx))
                .width(96.0)
                .mono()
                .right(),
            1150.0,
        ),
        responsive(
            column(store.tr(|s| s.node.peers.col_tunnel))
                .width(64.0)
                .mono(),
            1290.0,
        ),
        responsive(
            column(store.tr(|s| s.node.peers.col_nat)).width(120.0),
            1410.0,
        ),
        responsive(
            column(store.tr(|s| s.node.peers.col_version))
                .width(88.0)
                .mono(),
            1530.0,
        ),
    ]);
    let rows = {
        let cols = cols.clone();
        each(
            move || node.peer_rows(),
            key,
            move |item, _| row(store, &cols, item).build(),
        )
    };
    view().child(
        data_table(&cols)
            .body(rows)
            .empty(
                move || node.peer_rows().is_empty(),
                move || empty(store).into_element(),
            )
            .test_id("peers-table"),
    )
}

/// No rows: nobody else in the scope yet, or nobody the search and route filter keep.
fn empty(store: Store) -> View {
    let node = store.node();
    view().child(
        when(
            move || node.scope_peers().is_empty(),
            move || {
                empty_panel(
                    Icon::Peers,
                    store.tr(|s| s.node.peers.none_title),
                    store.tr(|s| s.node.peers.none_body),
                    None,
                )
            },
        )
        .otherwise(move || {
            empty_panel(
                Icon::Search,
                store.tr(|s| s.node.peers.no_match_title),
                store.tr(|s| s.node.peers.no_match_body),
                Some(
                    tag(
                        action_button(
                            ButtonKind::Secondary,
                            store.tr(|s| s.node.peers.no_match_action),
                            move || node.clear_peers_narrowing(),
                        ),
                        "peers-clear-filter",
                    )
                    .into_element(),
                ),
            )
        }),
    )
}

/// The lamp of a route.
pub(super) fn route_lamp(route: RouteKind) -> Lamp {
    match route {
        RouteKind::Direct => Lamp::Good,
        RouteKind::Relayed { .. } => Lamp::Degraded,
        RouteKind::Unknown => Lamp::Idle,
    }
}

/// The words of a route.
pub(super) fn route_word(store: Store, route: RouteKind) -> String {
    let words = store.strings().node.peers;
    match route {
        RouteKind::Direct => words.route_direct.to_owned(),
        RouteKind::Relayed { hops } => fill(words.route_relayed, &[("hops", &hops.to_string())]),
        RouteKind::Unknown => words.route_unknown.to_owned(),
    }
}

/// A rate, or nothing (a dash in the cell) before the peer's counter was read twice.
fn rate_cell_text(rate: Option<f64>) -> String {
    rate.map_or_else(String::new, rate_text)
}

/// One peer's row. `item` follows the core's readings while the row stays.
fn row(store: Store, cols: &Columns, item: ReadSignal<PeerRow>) -> crate::components::TableRow {
    let node = store.node();
    let reference = item.with_untracked(PeerRow::reference);
    let of = move |f: fn(&PeerRow) -> String| move || item.with(f);
    let rates = {
        let reference = reference.clone();
        computed(move || node.latest_peer_rates(&reference.network, reference.peer_id))
    };
    let latency = move || item.with(|row| row.peer.latency_ms);
    let loss = move || item.with(|row| row.peer.loss);
    let route = move || item.with(|row| row.peer.route);
    let selected = {
        let reference = reference.clone();
        move || node.open_peer_ref().as_ref() == Some(&reference)
    };
    let open = {
        let reference = reference.clone();
        move || node.open_peer(reference.clone())
    };
    table_row(cols)
        .text(of(PeerRow::display_name))
        .text(of(|row| address_text(row.peer.virtual_ipv4)))
        .cell(status_chip(
            move || route_lamp(route()),
            move || route_word(store, route()),
        ))
        .cell(threshold_bar(
            latency,
            move || latency_text(latency()),
            LATENCY_BAR,
        ))
        .cell(threshold_bar(loss, move || loss_text(loss()), LOSS_BAR))
        .text(move || rate_cell_text(rates.get().rx_bps))
        .text(move || rate_cell_text(rates.get().tx_bps))
        .text(of(|row| row.peer.tunnel.clone().unwrap_or_default()))
        .text(move || {
            let nat = item.with(|row| row.peer.nat_type);
            if nat == NatType::Unknown {
                String::new()
            } else {
                nat_word(store, nat).to_owned()
            }
        })
        .text(of(|row| row.peer.version.clone().unwrap_or_default()))
        .selected(selected)
        .on_press(open)
        .label(move || {
            SharedString::from(fill(
                store.strings().node.peers.row_label,
                &[("name", &item.with(PeerRow::display_name))],
            ))
        })
        .test_id(format!("peer-{}", reference.peer_id))
}
