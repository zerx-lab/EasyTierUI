//! The Overview tab: why the network is not running (when it is not), and while it runs its throughput and latency over
//! time and the lists the node reports (listeners, public addresses, shared subnets).

use nsh_base::theme;
use nsh_core::prelude::*;

use super::super::{
    failure,
    panels::{self, only},
};
use crate::{
    components::{CalloutKind, body_text, callout, machine_text, meta_text, section_label},
    core::state::NodeSnapshot,
    profile::ProfileId,
    store::{
        Store,
        node::{DetailTab, Status},
    },
};

/// The tab of network `id`.
pub(super) fn tab(store: Store, id: ProfileId, managed: bool) -> View {
    let node = store.node();
    let status = computed({
        let id = id.clone();
        move || node.status(&id)
    });
    let snapshot = computed({
        let id = id.clone();
        move || node.snapshot(&id)
    });
    let stop_error = computed({
        let id = id.clone();
        move || node.stop_error(&id)
    });
    let start_failure = computed({
        let id = id.clone();
        move || node.start_failure(&id)
    });
    let running = computed(move || snapshot.with(Option::is_some));

    view()
        .gap_6()
        .child(managed.then(|| {
            callout(
                CalloutKind::Info,
                store.tr(|s| s.node.list.managed_read_only),
            )
            .test_id("managed-note")
        }))
        .child(dynamic({
            let id = id.clone();
            move || match status.get() {
                Status::Failed(failed) => {
                    Some(untracked(|| failure::panel(store, id.clone(), failed)).into_element())
                }
                _ => None,
            }
        }))
        .child(dynamic(move || {
            stop_error
                .get()
                .map(|reason| untracked(|| failure::stop_error(store, reason)).into_element())
        }))
        .child(dynamic(move || {
            let failed = start_failure.get().is_some();
            (failed && status.get() == Status::Running).then(|| {
                callout(
                    CalloutKind::Warning,
                    store.tr(|s| s.node.detail.start_failed_running),
                )
                .action(store.tr(|s| s.node.shared.failure_action_logs), move || {
                    node.set_tab(DetailTab::Logs);
                })
                .test_id("start-failed-running")
                .into_element()
            })
        }))
        .child(
            when(
                move || running.get(),
                move || running_view(store, id.clone(), snapshot),
            )
            .otherwise(move || {
                meta_text(move || {
                    let words = store.strings().node.detail;
                    match status.get() {
                        Status::Stopped => words.quiet_stopped,
                        Status::Starting | Status::Running => words.quiet_starting,
                        Status::Stopping => words.quiet_stopping,
                        Status::Removing => words.quiet_removing,
                        Status::Failed(_) => words.quiet_failed,
                    }
                })
                .test_id("status-quiet")
            }),
        )
}

/// A running network: the two charts, then the node's lists.
fn running_view(store: Store, id: ProfileId, snapshot: Memo<Option<NodeSnapshot>>) -> View {
    let list_of = move |read: fn(&NodeSnapshot) -> Vec<String>| {
        computed(move || snapshot.with(|snapshot| snapshot.as_ref().map(read).unwrap_or_default()))
    };
    let listeners = list_of(|snapshot| snapshot.node.listeners.clone());
    let public_ips = list_of(|snapshot| snapshot.node.public_ips.clone());
    let subnets = list_of(|snapshot| {
        snapshot
            .node
            .proxy_cidrs
            .iter()
            .map(ToString::to_string)
            .collect()
    });
    view()
        .gap_6()
        .child(panels::charts_grid(store, only(id), "detail"))
        .child(
            view()
                .gap_4()
                .child(
                    body_text(store.tr(|s| s.node.detail.section_node))
                        .font_weight(FontWeight::MEDIUM)
                        .role(AccessibilityRole::Header)
                        .text_color(move || theme().foreground),
                )
                .child(
                    view()
                        .grid()
                        .grid_template_columns("repeat(auto-fit, minmax(240px, 1fr))")
                        .gap_x_6()
                        .gap_y_4()
                        .child(labelled(
                            store.tr(|s| s.node.detail.label_listeners),
                            lines(store, listeners, "detail-listeners"),
                        ))
                        .child(labelled(
                            store.tr(|s| s.node.detail.label_public_ips),
                            lines(store, public_ips, "detail-public-ips"),
                        ))
                        .child(labelled(
                            store.tr(|s| s.node.detail.label_subnets),
                            lines(store, subnets, "detail-subnets"),
                        )),
                ),
        )
}

/// A label in Label type over its value.
fn labelled(label: impl IntoReactive<SharedString>, value: impl IntoElement) -> View {
    view()
        .gap_1()
        .min_w_0()
        .child(section_label(label))
        .child(value)
}

/// Machine lines of a list, or the word for none.
fn lines(store: Store, items: Memo<Vec<String>>, test_id: &'static str) -> AnyElement {
    dynamic(move || {
        let items = items.get();
        if items.is_empty() {
            return view()
                .child(meta_text(store.tr(|s| s.node.detail.word_none)))
                .test_id(test_id)
                .into_element();
        }
        view()
            .gap_0p5()
            .test_id(test_id)
            .child(
                items
                    .into_iter()
                    .map(|item| machine_text(item).number_of_lines(1).selectable(true))
                    .collect::<Vec<_>>(),
            )
            .into_element()
    })
    .into_element()
}
