//! The charts of the Peers page and the network drawer: throughput over time and the latency of the slowest peers,
//! drawn from the history the node store keeps in memory, on a time axis of the last three minutes.

use std::rc::Rc;

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    components::{ChartUnit, SeriesColor, body_text, card, chart_series, metric_chart},
    profile::ProfileId,
    store::{
        Store,
        node::{HISTORY_SPAN_MS, LATENCY_LINES, LatencyLine},
    },
};

/// The networks a chart covers, read where the chart draws (so a changed selection redraws it).
pub(super) type Scope = Rc<dyn Fn() -> Vec<ProfileId>>;

/// A scope of exactly the network `id`.
pub(super) fn only(id: ProfileId) -> Scope {
    Rc::new(move || vec![id.clone()])
}

/// A chart's heading in Body type, medium weight.
fn heading(label: impl IntoReactive<SharedString>) -> Text {
    body_text(label)
        .font_weight(FontWeight::MEDIUM)
        .role(AccessibilityRole::Header)
        .text_color(move || theme().foreground)
}

/// The two charts side by side (stacked on a narrow window): throughput, then latency, each under its heading.
pub(super) fn charts_grid(store: Store, scope: Scope, test_id: &'static str) -> View {
    view()
        .grid()
        .grid_template_columns("repeat(auto-fit, minmax(320px, 1fr))")
        .gap_x_6()
        .gap_y_4()
        .child(
            view()
                .gap_3()
                .child(heading(store.tr(|s| s.node.peers.chart_throughput)))
                .child(throughput_chart(store, scope.clone(), test_id)),
        )
        .child(
            view()
                .gap_3()
                .child(heading(store.tr(|s| s.node.peers.chart_latency)))
                .child(latency_chart(store, scope, test_id)),
        )
}

/// [`charts_grid`] in a panel, for a page (the drawer draws the grid bare).
pub(super) fn charts_panel(store: Store, scope: Scope, test_id: &'static str) -> View {
    card().child(charts_grid(store, scope, test_id))
}

/// The throughput of `scope` over time: received and sent as areas, on a bytes-per-second axis.
pub(super) fn throughput_chart(store: Store, scope: Scope, test_id: &'static str) -> View {
    let node = store.node();
    let rates = computed({
        let scope = scope.clone();
        move || node.throughput_series(&scope())
    });
    view().child(
        metric_chart(
            store.tr(|s| s.node.peers.chart_throughput),
            move || rates.with(|series| series.t.clone()),
            vec![
                chart_series(
                    store.tr(|s| s.node.peers.legend_received),
                    SeriesColor::Received,
                    move || rates.with(|series| series.rx.clone()),
                )
                .filled(),
                chart_series(
                    store.tr(|s| s.node.peers.legend_sent),
                    SeriesColor::Sent,
                    move || rates.with(|series| series.tx.clone()),
                )
                .filled(),
            ],
            ChartUnit::BytesPerSecond,
            store.tr(|s| s.node.peers.chart_collecting),
        )
        .span(
            HISTORY_SPAN_MS,
            store.tr(|s| s.node.peers.chart_span_from),
            store.tr(|s| s.node.peers.chart_span_to),
        )
        .test_id(format!("{test_id}-throughput")),
    )
}

/// The latency of the slowest peers of `scope` (at most five), one line each, on a milliseconds axis. The chart has
/// a slot per line, filled in the order of the peers' keys, so it stays mounted while peers come and go; a line's
/// color follows its peer.
pub(super) fn latency_chart(store: Store, scope: Scope, test_id: &'static str) -> View {
    let node = store.node();
    let lines = computed({
        let scope = scope.clone();
        move || node.latency_lines(&scope())
    });
    let series: Vec<_> = (0..LATENCY_LINES)
        .map(|slot| {
            let line = move || lines.with(|chart| chart.lines.get(slot).cloned());
            chart_series(
                move || {
                    line()
                        .map(|line| peer_name(store, &line))
                        .unwrap_or_default()
                },
                SeriesColor::Chart(1),
                move || line().map(|line| line.samples).unwrap_or_default(),
            )
            .color_with(move || {
                line().map_or(SeriesColor::Chart(1), |line| SeriesColor::hashed(&line.key))
            })
            .shown_while(move || line().is_some())
        })
        .collect();
    view().child(
        metric_chart(
            store.tr(|s| s.node.peers.chart_latency),
            move || lines.with(|chart| chart.t.clone()),
            series,
            ChartUnit::Milliseconds,
            store.tr(|s| s.node.peers.chart_collecting),
        )
        .span(
            HISTORY_SPAN_MS,
            store.tr(|s| s.node.peers.chart_span_from),
            store.tr(|s| s.node.peers.chart_span_to),
        )
        .test_id(format!("{test_id}-latency")),
    )
}

/// The name a latency line goes by: the peer's hostname, or its id when it reports none.
fn peer_name(store: Store, line: &LatencyLine) -> String {
    store
        .node()
        .snapshot(&line.network)
        .and_then(|snapshot| {
            snapshot
                .peers
                .into_iter()
                .find(|peer| peer.peer_id == line.peer_id)
        })
        .map(|peer| peer.hostname)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| line.peer_id.to_string())
}
