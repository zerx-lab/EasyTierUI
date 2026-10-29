//! The Stats tab: the metrics the core of one running network counts, filtered by a search, and a button that puts the
//! same metrics on the clipboard as Prometheus text.

use nsh_core::prelude::*;
use uuid::Uuid;

use super::{Feed, feed_view, remote_error_text, t, toolbar, tr};
use crate::{
    components::{
        ButtonKind, Icon, action_button, column, columns, data_table, empty_panel, meta_text,
        search_field, table_row,
    },
    core::{remote::Metric, target::TargetId},
    store::{Store, console::CopyState},
};

/// One running network's metrics: refresh, search and copy, and the table.
pub(in crate::pages::console) fn stats_tab(
    store: Store,
    id: TargetId,
    mid: Uuid,
    instance: Uuid,
) -> AnyElement {
    let console = store.console();
    let feed = move || console.inspect_feed(id, mid, instance, |views| &views.stats);
    let refresh = move || console.refresh_stats(id, mid, instance);
    refresh();
    let query = signal(String::new());
    let search = search_field(
        t(store, |s| s.stats_search),
        move || query.get(),
        move |text| query.set(text.to_owned()),
    )
    .clear_label(tr(store, |c| c.search_clear))
    .test_id("machine-stats-search");
    view()
        .gap_4()
        .child(toolbar(
            store,
            move || feed().read_at_ms,
            refresh,
            "machine-stats-refresh",
        ))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(search)
                .child(copy_button(store, id, mid, instance)),
        )
        .child(dynamic(move || match console.prometheus_copy() {
            CopyState::Failed(error) => {
                Some(meta_text(remote_error_text(store.strings(), &error)).into_element())
            }
            CopyState::Idle | CopyState::Reading | CopyState::Copied => None,
        }))
        .child(feed_view(
            store,
            feed,
            |metrics: &Vec<Metric>| metrics.is_empty(),
            refresh,
            move || {
                empty_panel(
                    Icon::Activity,
                    t(store, |s| s.stats_empty_title),
                    t(store, |s| s.stats_empty_body),
                    None,
                )
                .into_element()
            },
            move || table(store, feed, query),
        ))
        .into_element()
}

/// The button that copies the metrics as Prometheus text; it says while it reads and when it has copied.
fn copy_button(store: Store, id: TargetId, mid: Uuid, instance: Uuid) -> View {
    let console = store.console();
    view().test_id("machine-stats-copy").child(action_button(
        ButtonKind::Secondary,
        move || {
            let words = store.strings().console_inspect;
            match console.prometheus_copy() {
                CopyState::Reading => words.stats_copy_reading,
                CopyState::Copied => words.stats_copy_done,
                CopyState::Idle | CopyState::Failed(_) => words.stats_copy,
            }
        },
        move || console.copy_prometheus(id, mid, instance),
    ))
}

/// A metric's labels as `name=value, name=value`.
fn labels_text(metric: &Metric) -> String {
    metric
        .labels
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The metrics the search keeps, in the core's order.
fn matching(metrics: Vec<Metric>, query: &str) -> Vec<Metric> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return metrics;
    }
    metrics
        .into_iter()
        .filter(|metric| {
            metric.name.to_lowercase().contains(&query)
                || labels_text(metric).to_lowercase().contains(&query)
        })
        .collect()
}

/// The metrics read last as a table.
fn table(
    store: Store,
    feed: impl Fn() -> Feed<Vec<Metric>> + Copy + 'static,
    query: Signal<String>,
) -> AnyElement {
    let shown = computed(move || matching(feed().value.unwrap_or_default(), &query.get()));
    let cols = columns(vec![
        column(t(store, |s| s.stats_col_name)).width(208.0).mono(),
        column(t(store, |s| s.stats_col_labels)).flex().mono(),
        column(t(store, |s| s.stats_col_value))
            .width(104.0)
            .mono()
            .right(),
    ]);
    let rows_columns = cols.clone();
    data_table(&cols)
        .body(each(
            move || shown.get(),
            |metric: &Metric| format!("{}{{{}}}", metric.name, labels_text(metric)),
            move |metric, _| {
                table_row(&rows_columns)
                    .text(move || metric.with(|metric| metric.name.clone()))
                    .text(move || metric.with(labels_text))
                    .text(move || metric.with(|metric| metric.value.to_string()))
                    .build()
            },
        ))
        .empty(
            move || shown.with(|shown| shown.is_empty()),
            move || {
                empty_panel(
                    Icon::Search,
                    tr(store, |c| c.no_match_title),
                    tr(store, |c| c.no_match_body),
                    Some(
                        action_button(
                            ButtonKind::Secondary,
                            tr(store, |c| c.search_clear),
                            move || query.set(String::new()),
                        )
                        .into_element(),
                    ),
                )
                .into_element()
            },
        )
        .test_id("machine-stats")
        .into_element()
}
