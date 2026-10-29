//! The Logs tab: the core's recent output for this network, already redacted by the backend, with a refresh button
//! and a follow toggle that reads it again at every update.

use nsh_core::prelude::*;

use super::super::common::copy_labels;
use crate::{
    components::{Icon, IconTone, code_block, icon_button, meta_text},
    core::state::LogStream,
    profile::ProfileId,
    store::{Store, node::LogView},
};

/// What the log area shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Content {
    /// At least one line.
    Lines,
    /// Nothing yet and a read is in flight.
    Loading,
    /// A read came back empty.
    Empty,
}

/// The tab of network `id`.
pub(super) fn tab(store: Store, id: ProfileId) -> View {
    let node = store.node();
    let logs = node.logs();
    let content = computed(move || {
        logs.with(|view| {
            if !view.lines.is_empty() {
                Content::Lines
            } else if view.loading {
                Content::Loading
            } else {
                Content::Empty
            }
        })
    });
    // The toggle is made again when it flips, with its state passed in as a value.
    let follow = dynamic(move || {
        let on = node.logs_follow();
        icon_button(
            Icon::ArrowDown,
            store.tr(|s| s.node.detail.logs_follow),
            move || node.set_logs_follow(!on),
        )
        .selected(on)
        .tone(if on {
            IconTone::Active
        } else {
            IconTone::Quiet
        })
        .test_id("logs-follow")
    });
    view()
        .gap_4()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .child(meta_text(store.tr(|s| s.node.detail.logs_note)).flex_1())
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(follow)
                        .child(
                            icon_button(Icon::Refresh, store.tr(|s| s.common.refresh), move || {
                                node.load_logs(id.clone())
                            })
                            .test_id("logs-refresh"),
                        ),
                ),
        )
        .child(dynamic(move || match content.get() {
            Content::Lines => untracked(|| {
                code_block("logs", move || logs_text(logs), copy_labels(store)).into_element()
            }),
            Content::Loading => meta_text(store.tr(|s| s.common.loading))
                .test_id("logs-loading")
                .into_element(),
            Content::Empty => meta_text(store.tr(|s| s.node.detail.logs_empty))
                .test_id("logs-empty")
                .into_element(),
        }))
}

/// The lines as one text, oldest first: the app's own notes start with `› `. No timestamps (the lines carry UTC
/// milliseconds, which would mislead).
fn logs_text(logs: Signal<LogView>) -> String {
    logs.with(|view| {
        view.lines
            .iter()
            .map(|line| match line.stream {
                LogStream::App => format!("› {}", line.text),
                LogStream::Stdout | LogStream::Stderr => line.text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    })
}
