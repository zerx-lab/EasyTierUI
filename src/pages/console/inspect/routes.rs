//! The Routes tab: the routing table of one running network, one row per node with the way to it.

use nsh_core::prelude::*;
use uuid::Uuid;

use super::{Feed, feed_view, filled, format_latency, t, toolbar, tr};
use crate::{
    components::{
        Columns, Icon, Lamp, Thresholds, column, columns, data_table, empty_panel, machine_text,
        meta_text, status_chip, table_row, threshold_bar,
    },
    core::{remote::RouteEntry, target::TargetId},
    store::Store,
};

/// One running network's routing table: refresh, and the table.
pub(in crate::pages::console) fn routes_tab(
    store: Store,
    id: TargetId,
    mid: Uuid,
    instance: Uuid,
) -> AnyElement {
    let console = store.console();
    let feed = move || console.inspect_feed(id, mid, instance, |views| &views.routes);
    let refresh = move || console.refresh_routes(id, mid, instance);
    refresh();
    view()
        .gap_4()
        .child(toolbar(
            store,
            move || feed().read_at_ms,
            refresh,
            "machine-routes-refresh",
        ))
        .child(feed_view(
            store,
            feed,
            |routes: &Vec<RouteEntry>| routes.is_empty(),
            refresh,
            move || {
                empty_panel(
                    Icon::Network,
                    t(store, |s| s.routes_empty_title),
                    t(store, |s| s.routes_empty_body),
                    None,
                )
                .into_element()
            },
            move || routes_table(store, feed),
        ))
        .into_element()
}

/// The columns of the table.
fn route_columns(store: Store) -> Columns {
    columns(vec![
        column(t(store, |s| s.routes_col_host)).flex(),
        column(tr(store, |c| c.col_virtual_ip))
            .width(128.0)
            .mono()
            .right(),
        column(tr(store, |c| c.col_route)).width(144.0),
        column(t(store, |s| s.routes_col_next_hop))
            .width(88.0)
            .mono()
            .right()
            .hint(t(store, |s| s.routes_col_next_hop_hint)),
        column(tr(store, |c| c.col_latency))
            .width(104.0)
            .hint(tr(store, |c| c.col_latency_hint)),
    ])
}

/// The routes read last as a table, by hostname.
fn routes_table(
    store: Store,
    feed: impl Fn() -> Feed<Vec<RouteEntry>> + Copy + 'static,
) -> AnyElement {
    let cols = route_columns(store);
    let rows_columns = cols.clone();
    data_table(&cols)
        .body(each(
            move || {
                let mut routes = feed().value.unwrap_or_default();
                routes.sort_by(|a, b| {
                    a.hostname
                        .cmp(&b.hostname)
                        .then_with(|| a.peer_id.cmp(&b.peer_id))
                });
                routes
            },
            |route: &RouteEntry| route.peer_id,
            move |route, _| route_row(store, &rows_columns, route),
        ))
        .test_id("machine-routes")
        .into_element()
}

/// One node: its name and address, whether the way to it is direct, the first hop, and the latency; its core version
/// and the subnets it proxies in a line below.
fn route_row(store: Store, cols: &Columns, route: ReadSignal<RouteEntry>) -> View {
    let direct = move || route.with(|route| route.cost <= 1);
    let latency = move || route.with(|route| route.latency_ms.map(f64::from));
    let way = move || {
        if direct() {
            store.strings().console.peer_chip_direct.to_owned()
        } else {
            let hops = route.with(|route| route.cost).to_string();
            filled(store, |s| s.routes_relayed_hops, &[("hops", hops.as_str())])
        }
    };
    table_row(cols)
        .text(move || route.with(|route| route.hostname.clone()))
        .text(move || {
            route.with(|route| {
                route
                    .virtual_ipv4
                    .map(|ip| ip.to_string())
                    .or_else(|| route.virtual_ipv6.map(|ip| ip.to_string()))
                    .unwrap_or_default()
            })
        })
        .cell(status_chip(
            move || if direct() { Lamp::Good } else { Lamp::Degraded },
            way,
        ))
        .text(move || route.with(|route| route.next_hop_peer_id.to_string()))
        .cell(threshold_bar(
            latency,
            move || latency().map(format_latency).unwrap_or_default(),
            Thresholds {
                warn: 100.0,
                bad: None,
                full: 300.0,
            },
        ))
        .detail(extras(store, route))
        .build()
}

/// The line below a row: the node's core version and the subnets it proxies, each only when it has one.
fn extras(store: Store, route: ReadSignal<RouteEntry>) -> View {
    let pair = move |label: &'static str, value: String| {
        view()
            .flex_row()
            .items_center()
            .gap_1()
            .min_w_0()
            .child(meta_text(label))
            .child(machine_text(SharedString::from(value)).number_of_lines(1))
            .into_element()
    };
    view().px_4().pb_2().child(dynamic(move || {
        let (version, subnets) =
            route.with(|route| (route.version.clone(), route.proxy_cidrs.join(", ")));
        let words = store.strings().console;
        let mut parts = Vec::new();
        if !version.is_empty() {
            parts.push(pair(words.col_version, version));
        }
        if !subnets.is_empty() {
            parts.push(pair(
                store.strings().console_inspect.routes_col_subnets,
                subnets,
            ));
        }
        (!parts.is_empty()).then(|| {
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_x_3()
                .gap_y_1()
                .child(parts)
                .into_element()
        })
    }))
}
