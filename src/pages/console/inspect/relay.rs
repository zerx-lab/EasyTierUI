//! The Relay tab: the other networks whose traffic this node carries (with their links), how much each relaying node
//! of the mesh carries, and the mesh's map of direct links between peers.

use nsh_core::prelude::*;
use uuid::Uuid;

use super::{
    Feed, clock, feed_view, filled, format_ago, format_bytes, format_latency, heading, t, toolbar,
    tr,
};
use crate::{
    components::{
        Columns, Icon, Thresholds, body_text, column, columns, data_table, empty_panel, meta_text,
        neutral_chip, table_group_header, table_row, threshold_bar,
    },
    core::{
        remote::{
            ForeignNetwork, ForeignNetworkHost, ForeignNetworkSummary, PeerLink, PeerMapNode,
        },
        target::TargetId,
    },
    store::Store,
};

/// One open link of a foreign network's peer.
#[derive(Debug, Clone, PartialEq)]
struct LinkItem {
    /// Its place among the peer's links, which tells equal links apart.
    index: usize,
    /// The peer it leads to.
    peer_id: u32,
    link: PeerLink,
}

/// A foreign network relayed by one node of the mesh.
#[derive(Debug, Clone, PartialEq)]
struct MeshItem {
    /// The relaying node.
    host: u32,
    name: String,
    peers: usize,
    updated_ms: Option<f64>,
}

/// The four reads of the tab, under one refresh.
pub(in crate::pages::console) fn relay_tab(
    store: Store,
    id: TargetId,
    mid: Uuid,
    instance: Uuid,
) -> AnyElement {
    let console = store.console();
    let foreign = move || console.inspect_feed(id, mid, instance, |views| &views.foreign);
    let summary = move || console.inspect_feed(id, mid, instance, |views| &views.foreign_summary);
    let mesh = move || console.inspect_feed(id, mid, instance, |views| &views.global_foreign);
    let map = move || console.peer_map(id, mid);
    let refresh = move || console.refresh_relay(id, mid, instance);
    refresh();
    view()
        .gap_6()
        .child(toolbar(
            store,
            move || foreign().read_at_ms,
            refresh,
            "machine-relay-refresh",
        ))
        .child(section(
            store,
            |s| s.relay_h_here,
            feed_view(
                store,
                foreign,
                |networks: &Vec<ForeignNetwork>| networks.is_empty(),
                refresh,
                move || {
                    empty(
                        store,
                        |s| s.relay_here_empty_title,
                        |s| s.relay_here_empty_body,
                    )
                },
                move || foreign_table(store, foreign),
            ),
        ))
        .child(section(
            store,
            |s| s.relay_h_summary,
            feed_view(
                store,
                summary,
                |nodes: &Vec<ForeignNetworkSummary>| nodes.is_empty(),
                refresh,
                move || {
                    empty(
                        store,
                        |s| s.relay_summary_empty_title,
                        |s| s.relay_summary_empty_body,
                    )
                },
                move || summary_table(store, summary),
            ),
        ))
        .child(section(
            store,
            |s| s.relay_h_mesh,
            feed_view(
                store,
                mesh,
                |hosts: &Vec<ForeignNetworkHost>| hosts.iter().all(|host| host.networks.is_empty()),
                refresh,
                move || {
                    empty(
                        store,
                        |s| s.relay_mesh_empty_title,
                        |s| s.relay_mesh_empty_body,
                    )
                },
                move || mesh_table(store, mesh),
            ),
        ))
        .child(section(
            store,
            |s| s.relay_h_map,
            feed_view(
                store,
                map,
                |nodes: &Vec<PeerMapNode>| nodes.is_empty(),
                move || console.refresh_peer_map(id, mid),
                move || {
                    empty(
                        store,
                        |s| s.relay_map_empty_title,
                        |s| s.relay_map_empty_body,
                    )
                },
                move || map_table(store, map),
            ),
        ))
        .into_element()
}

/// A heading over one part of the tab.
fn section(store: Store, title: super::Pick, content: AnyElement) -> View {
    view()
        .gap_3()
        .child(heading(t(store, title)))
        .child(content)
}

/// The empty state of one part.
fn empty(store: Store, title: super::Pick, body: super::Pick) -> AnyElement {
    empty_panel(Icon::Network, t(store, title), t(store, body), None).into_element()
}

// --- Networks relayed here ---------------------------------------------------------------------------------------

/// The columns of the links table.
fn link_columns(store: Store) -> Columns {
    columns(vec![
        column(tr(store, |c| c.col_peer)).width(96.0).mono(),
        column(t(store, |s| s.relay_col_tunnel)).width(72.0),
        column(t(store, |s| s.relay_col_remote)).flex().mono(),
        column(tr(store, |c| c.col_latency))
            .width(104.0)
            .hint(tr(store, |c| c.col_latency_hint)),
        column(tr(store, |c| c.col_received))
            .width(84.0)
            .mono()
            .right()
            .hide_below(1040.0),
        column(tr(store, |c| c.col_sent))
            .width(84.0)
            .mono()
            .right()
            .hide_below(1040.0),
    ])
}

/// The foreign networks read last, each with its open links.
fn foreign_table(
    store: Store,
    feed: impl Fn() -> Feed<Vec<ForeignNetwork>> + Copy + 'static,
) -> AnyElement {
    let cols = link_columns(store);
    let rows_columns = cols.clone();
    data_table(&cols)
        .body(each(
            move || feed().value.unwrap_or_default(),
            |network: &ForeignNetwork| network.name.clone(),
            move |network, _| foreign_group(store, &rows_columns, network),
        ))
        .test_id("machine-relay-foreign")
        .into_element()
}

/// One foreign network: its name, how many peers are connected here and the id this node has there, then its links.
fn foreign_group(store: Store, cols: &Columns, network: ReadSignal<ForeignNetwork>) -> View {
    let links = move || {
        network.with(|network| {
            network
                .peers
                .iter()
                .flat_map(|peer| {
                    peer.links
                        .iter()
                        .enumerate()
                        .filter(|(_, link)| !link.closed)
                        .map(|(index, link)| LinkItem {
                            index,
                            peer_id: peer.peer_id,
                            link: link.clone(),
                        })
                })
                .collect::<Vec<_>>()
        })
    };
    let rows_columns = cols.clone();
    view()
        .child(
            table_group_header()
                .child(
                    body_text(move || network.with(|network| network.name.clone()))
                        .font_weight(FontWeight::SEMIBOLD)
                        .number_of_lines(1)
                        .min_w_0(),
                )
                .child(neutral_chip(move || {
                    let count = network.with(|network| network.peers.len()).to_string();
                    filled(
                        store,
                        |s| s.relay_network_peers,
                        &[("count", count.as_str())],
                    )
                })),
        )
        .child(view().px_4().py_2().child(meta_text(move || {
            let mine = network.with(|network| network.my_peer_id).to_string();
            filled(store, |s| s.relay_my_id, &[("id", mine.as_str())])
        })))
        .child(dynamic(move || {
            links().is_empty().then(|| {
                view()
                    .px_4()
                    .py_3()
                    .child(meta_text(t(store, |s| s.relay_no_links)))
                    .into_element()
            })
        }))
        .child(each(
            links,
            |item: &LinkItem| (item.peer_id, item.index),
            move |item, _| link_row(&rows_columns, item),
        ))
}

/// One open link: the peer, the tunnel, the address, latency and the bytes it moved.
fn link_row(cols: &Columns, item: ReadSignal<LinkItem>) -> View {
    let latency = move || item.with(|item| item.link.latency_ms);
    table_row(cols)
        .text(move || item.with(|item| item.peer_id.to_string()))
        .text(move || item.with(|item| item.link.tunnel.clone()))
        .text(move || item.with(|item| item.link.remote.clone().unwrap_or_default()))
        .cell(threshold_bar(
            latency,
            move || latency().map(format_latency).unwrap_or_default(),
            Thresholds {
                warn: 100.0,
                bad: None,
                full: 300.0,
            },
        ))
        .text(move || item.with(|item| format_bytes(item.link.rx_bytes)))
        .text(move || item.with(|item| format_bytes(item.link.tx_bytes)))
        .build()
}

// --- The relay summary ---------------------------------------------------------------------------------------------

/// One relaying node per row: how many foreign networks and peers it carries.
fn summary_table(
    store: Store,
    feed: impl Fn() -> Feed<Vec<ForeignNetworkSummary>> + Copy + 'static,
) -> AnyElement {
    let cols = columns(vec![
        column(t(store, |s| s.relay_col_node)).flex().mono(),
        column(t(store, |s| s.relay_col_networks))
            .width(112.0)
            .mono()
            .right(),
        column(t(store, |s| s.relay_col_peers))
            .width(112.0)
            .mono()
            .right(),
    ]);
    let rows_columns = cols.clone();
    data_table(&cols)
        .body(each(
            move || feed().value.unwrap_or_default(),
            |node: &ForeignNetworkSummary| node.peer_id,
            move |node, _| {
                table_row(&rows_columns)
                    .text(move || node.with(|node| node.peer_id.to_string()))
                    .text(move || node.with(|node| node.network_count.to_string()))
                    .text(move || node.with(|node| node.peer_count.to_string()))
                    .build()
            },
        ))
        .test_id("machine-relay-summary")
        .into_element()
}

// --- Networks relayed across the mesh ------------------------------------------------------------------------------

/// Every foreign network some node of the mesh relays, one row each.
fn mesh_table(
    store: Store,
    feed: impl Fn() -> Feed<Vec<ForeignNetworkHost>> + Copy + 'static,
) -> AnyElement {
    let now = clock();
    let cols = columns(vec![
        column(t(store, |s| s.relay_col_node)).width(128.0).mono(),
        column(t(store, |s| s.relay_col_network)).flex(),
        column(t(store, |s| s.relay_col_peers))
            .width(88.0)
            .mono()
            .right(),
        column(t(store, |s| s.relay_col_updated)).width(120.0),
    ]);
    let rows_columns = cols.clone();
    data_table(&cols)
        .body(each(
            move || {
                feed()
                    .value
                    .unwrap_or_default()
                    .into_iter()
                    .flat_map(|host| {
                        let relay = host.peer_id;
                        host.networks.into_iter().map(move |network| MeshItem {
                            host: relay,
                            name: network.name,
                            peers: network.peer_ids.len(),
                            updated_ms: network.updated_at_ms,
                        })
                    })
                    .collect::<Vec<_>>()
            },
            |item: &MeshItem| (item.host, item.name.clone()),
            move |item, _| {
                table_row(&rows_columns)
                    .text(move || item.with(|item| item.host.to_string()))
                    .text(move || item.with(|item| item.name.clone()))
                    .text(move || item.with(|item| item.peers.to_string()))
                    .text(move || {
                        item.with(|item| item.updated_ms)
                            .map(|ms| format_ago(store.strings(), now.get(), ms))
                            .unwrap_or_default()
                    })
                    .build()
            },
        ))
        .test_id("machine-relay-mesh")
        .into_element()
}

// --- The peer map ----------------------------------------------------------------------------------------------------

/// The mesh's direct links: each node with the peers it is directly connected to and the latency of each link.
fn map_table(
    store: Store,
    feed: impl Fn() -> Feed<Vec<PeerMapNode>> + Copy + 'static,
) -> AnyElement {
    let cols = columns(vec![
        column(tr(store, |c| c.col_peer)).flex().mono(),
        column(tr(store, |c| c.col_latency))
            .width(160.0)
            .hint(tr(store, |c| c.col_latency_hint)),
    ]);
    let rows_columns = cols.clone();
    data_table(&cols)
        .body(each(
            move || {
                let mut nodes = feed().value.unwrap_or_default();
                nodes.sort_by_key(|node| node.peer_id);
                nodes
            },
            |node: &PeerMapNode| node.peer_id,
            move |node, _| map_group(store, &rows_columns, node),
        ))
        .test_id("machine-relay-map")
        .into_element()
}

/// One node of the map: a header with its id and link count, then a row per direct peer.
fn map_group(store: Store, cols: &Columns, node: ReadSignal<PeerMapNode>) -> View {
    let rows_columns = cols.clone();
    view()
        .child(
            table_group_header().child(
                body_text(move || {
                    let (id, count) =
                        node.with(|node| (node.peer_id.to_string(), node.direct_peers.len()));
                    filled(
                        store,
                        |s| s.relay_map_node,
                        &[("id", id.as_str()), ("count", count.to_string().as_str())],
                    )
                })
                .font_weight(FontWeight::SEMIBOLD)
                .number_of_lines(1)
                .min_w_0(),
            ),
        )
        .child(each(
            move || {
                let mut links = node.with(|node| node.direct_peers.clone());
                links.sort_by_key(|link| link.peer_id);
                links
            },
            |link: &crate::core::remote::PeerMapLink| link.peer_id,
            move |link, _| {
                let latency = move || link.with(|link| Some(f64::from(link.latency_ms)));
                table_row(&rows_columns)
                    .text(move || link.with(|link| link.peer_id.to_string()))
                    .cell(threshold_bar(
                        latency,
                        move || latency().map(format_latency).unwrap_or_default(),
                        Thresholds {
                            warn: 100.0,
                            bad: None,
                            full: 300.0,
                        },
                    ))
                    .build()
            },
        ))
}
