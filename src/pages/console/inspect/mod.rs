//! The machine drawer's tabs that read one running network of a machine: Node, Routes, Relay and Stats. Each is
//! called by the drawer inside the network picker, with the picked network instance. The helpers here are what they
//! share: a read's toolbar (when it was read, refresh) and the states of a read (loading, failed, empty, rows).

mod node;
mod relay;
mod routes;
mod stats;

use nsh_core::prelude::*;

pub(super) use self::{node::node_tab, relay::relay_tab, routes::routes_tab, stats::stats_tab};
use super::{
    format::{format_ago, format_bytes, format_latency, remote_error_text},
    machines::clock,
    networks::load_failed,
    tr,
};
use crate::{
    components::{CalloutKind, Icon, body_text, callout, icon_button, meta_text},
    core::remote::RemoteError,
    i18n::{ConsoleInspectStrings, fill},
    store::{Store, console::Feed},
};

/// Picks one string of the inspect tabs.
type Pick = fn(&'static ConsoleInspectStrings) -> &'static str;

/// A binding of the inspect string `pick` chooses, following the language as it changes.
fn t(store: Store, pick: Pick) -> impl Fn() -> &'static str + Copy + 'static {
    move || pick(store.strings().console_inspect)
}

/// The inspect template `pick` chooses, filled with `args`, in the current language.
fn filled(store: Store, pick: Pick, args: &[(&str, &str)]) -> String {
    fill(pick(store.strings().console_inspect), args)
}

/// What a read shows.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Loading,
    Failed(RemoteError),
    Empty,
    Rows,
}

/// What `feed` shows: rows, or why there are none.
fn kind_of<T>(feed: &Feed<T>, is_empty: fn(&T) -> bool) -> Kind {
    match &feed.value {
        None => match &feed.error {
            Some(error) => Kind::Failed(error.clone()),
            None => Kind::Loading,
        },
        Some(value) if is_empty(value) => Kind::Empty,
        Some(_) => Kind::Rows,
    }
}

/// The row above a tab's content: when it was read on the left, refresh on the right.
pub(super) fn toolbar(
    store: Store,
    read_at: impl Fn() -> Option<f64> + 'static,
    refresh: impl Fn() + 'static,
    test_id: &'static str,
) -> View {
    let now = clock();
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            view()
                .flex_1()
                .min_w_0()
                .child(meta_text(move || match read_at() {
                    Some(then) => filled(
                        store,
                        |s| s.read_ago,
                        &[("age", format_ago(store.strings(), now.get(), then).as_str())],
                    ),
                    None => String::new(),
                })),
        )
        .child(icon_button(Icon::Refresh, store.tr(|s| s.common.refresh), refresh).test_id(test_id))
}

/// A read's states in the drawer's body: a warning above the last good rows when the newest read failed, and in
/// place of them "loading", the failure with its retry, `empty`, or the `rows`.
pub(super) fn feed_view<T: Clone + 'static>(
    store: Store,
    feed: impl Fn() -> Feed<T> + Copy + 'static,
    is_empty: fn(&T) -> bool,
    retry: impl Fn() + Copy + 'static,
    empty: impl Fn() -> AnyElement + 'static,
    rows: impl Fn() -> AnyElement + 'static,
) -> AnyElement {
    let kind = computed(move || kind_of(&feed(), is_empty));
    let stale = computed(move || {
        let feed = feed();
        feed.value.and(feed.error)
    });
    view()
        .gap_3()
        .child(dynamic(move || {
            stale.get().map(|error| {
                view()
                    .gap_1()
                    .child(
                        callout(CalloutKind::Warning, tr(store, |c| c.refresh_failed))
                            .action(store.tr(|s| s.common.retry), retry),
                    )
                    .child(meta_text(remote_error_text(store.strings(), &error)))
                    .into_element()
            })
        }))
        .child(dynamic(move || match kind.get() {
            Kind::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
            Kind::Failed(error) => untracked(|| load_failed(store, &error, retry)),
            Kind::Empty => untracked(&empty),
            Kind::Rows => untracked(&rows),
        }))
        .into_element()
}

/// A heading above a part of a tab.
pub(super) fn heading(content: impl IntoReactive<SharedString>) -> Text {
    body_text(content).font_weight(FontWeight::SEMIBOLD)
}
