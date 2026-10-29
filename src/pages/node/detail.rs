//! The network drawer: the place for one network's detail and editing (DESIGN.md "The Drawer Rule").
//!
//! On screen while the node store names a network. The header carries the name, a "Network" chip, the lamp and the
//! actions that fit the state (Start, Stop, Restart); a key-value grid of the node's facts shows while it runs;
//! the tabs are a segmented control bound to the store's tab (Overview, Settings, Logs, Export); the footer is sticky.
//! A managed network has no Export tab and its Settings are read-only apart from the name and hostname.

mod export;
mod logs;
mod overview;
mod settings;

use nsh_core::prelude::*;

use super::common::{copy_labels, lamp_of, nat_word, save_failure_view, status_word, tag};
use crate::{
    components::{
        ButtonKind, SelectOption, action_button, age_text, busy_button, dialog_footer, drawer,
        fact, fact_mono, kv_grid, live_dot, meta_text, segmented_inline,
    },
    core::state::NodeSnapshot,
    i18n::fill,
    profile::ProfileId,
    store::{
        Store,
        node::{DetailTab, NodeStore, Status},
    },
};

/// The drawer, on screen while `store.node().detail()` names a network.
pub(super) fn drawer_view(store: Store) -> AnyElement {
    let node = store.node();
    let status = computed(move || node.detail().map_or(Status::Stopped, |id| node.status(&id)));
    drawer(
        move || node.detail().is_some(),
        move || {
            SharedString::from(
                node.detail()
                    .and_then(|id| node.profile(&id))
                    .map(|profile| profile.name)
                    .unwrap_or_default(),
            )
        },
        store.tr(|s| s.node.detail.drawer_close),
        move || node.close_detail(),
    )
    .kind(store.tr(|s| s.kind_network))
    .status(
        move || lamp_of(&status.get()),
        move || status_word(store.strings(), &status.get()),
    )
    .header_actions(move || header_actions(store))
    .meta(move || meta(store))
    .tabs(move || tabs(store))
    .body(move || body(store))
    .footer(move || footer(store))
    .test_id("network-drawer")
    .build()
}

/// Whether a control plane owns network `id` (read once; a network does not change hands while its drawer is open).
fn is_managed(node: NodeStore, id: &ProfileId) -> bool {
    untracked(|| node.profile(id)).is_some_and(|profile| profile.is_managed())
}

/// The id a tab goes by in the segmented control.
const fn tab_id(tab: DetailTab) -> &'static str {
    match tab {
        DetailTab::Overview => "overview",
        DetailTab::Settings => "settings",
        DetailTab::Logs => "logs",
        DetailTab::Export => "export",
    }
}

/// The tab an id of the segmented control stands for.
fn tab_of(id: &str) -> Option<DetailTab> {
    [
        DetailTab::Overview,
        DetailTab::Settings,
        DetailTab::Logs,
        DetailTab::Export,
    ]
    .into_iter()
    .find(|tab| tab_id(*tab) == id)
}

// ----- the header -----

/// Start, Stop and Restart as the state allows them, made again when the state or the busy flag changes. Both are
/// read here and passed to the buttons as values, never bound inside buttons this render is about to replace.
fn header_actions(store: Store) -> View {
    let node = store.node();
    view().child(dynamic(move || {
        let id = node.detail()?;
        let status = node.status(&id);
        let busy = node.is_busy(&id);
        Some(untracked(|| actions(store, id, &status, busy)).into_element())
    }))
}

/// The buttons of `status`.
fn actions(store: Store, id: ProfileId, status: &Status, busy_now: bool) -> View {
    let node = store.node();
    let busy = move || busy_now;
    let start = {
        let id = id.clone();
        tag(
            busy_button(
                ButtonKind::Primary,
                store.tr(|s| s.node.list.action_start),
                busy,
                move || node.start(id.clone()),
            ),
            "detail-start",
        )
    };
    let stop = {
        let id = id.clone();
        tag(
            busy_button(
                ButtonKind::Secondary,
                store.tr(|s| s.node.list.action_stop),
                busy,
                move || node.stop(id.clone()),
            ),
            "detail-stop",
        )
    };
    let restart = tag(
        busy_button(
            ButtonKind::Secondary,
            store.tr(|s| s.node.detail.action_restart),
            busy,
            move || node.restart(id.clone()),
        ),
        "detail-restart",
    );
    let row = view().flex_row().items_center().gap_2();
    match status {
        Status::Stopped | Status::Failed(_) => row.child(start),
        Status::Running => row.child(restart).child(stop),
        Status::Starting => row.child(stop),
        Status::Stopping | Status::Removing => row,
    }
}

// ----- the facts -----

/// The node's key-value grid while the network runs.
fn meta(store: Store) -> View {
    let node = store.node();
    let snapshot = computed(move || node.detail().and_then(|id| node.snapshot(&id)));
    let running = computed(move || snapshot.with(Option::is_some));
    view().child(dynamic(move || {
        running
            .get()
            .then(|| untracked(|| facts(store, snapshot)).into_element())
    }))
}

/// The grid: virtual IP, hostname, NAT type, peers and the core's version, each following the latest snapshot.
fn facts(store: Store, snapshot: Memo<Option<NodeSnapshot>>) -> View {
    let words = store.strings().node.detail;
    let of = move |read: fn(&NodeSnapshot) -> String| {
        move || snapshot.with(|snapshot| snapshot.as_ref().map(read).unwrap_or_default())
    };
    let nat = move || {
        snapshot.with(|snapshot| {
            snapshot
                .as_ref()
                .map(|snapshot| nat_word(store, snapshot.node.nat_type).to_owned())
                .unwrap_or_default()
        })
    };
    let peers = move || {
        snapshot.with(|snapshot| {
            snapshot
                .as_ref()
                .map(|snapshot| {
                    let mix = crate::store::node::PeerMix::of(&snapshot.peers);
                    fill(
                        store.strings().node.detail.peers_mix,
                        &[
                            ("online", &mix.online.to_string()),
                            ("direct", &mix.direct.to_string()),
                            ("relayed", &mix.relayed.to_string()),
                        ],
                    )
                })
                .unwrap_or_default()
        })
    };
    kv_grid(vec![
        fact_mono(
            store.tr(|s| s.node.list.virtual_ip),
            of(|snapshot| {
                snapshot
                    .node
                    .virtual_ipv4
                    .map_or_else(String::new, |net| net.to_string())
            }),
        )
        .copyable(copy_labels(store))
        .test_id("detail-virtual-ip"),
        fact(
            words.label_hostname,
            of(|snapshot| snapshot.node.hostname.clone()),
        )
        .test_id("detail-hostname"),
        fact(words.label_nat, nat).test_id("detail-nat"),
        fact_mono(store.tr(|s| s.node.list.peers_online), peers).test_id("detail-peer-count"),
        fact_mono(
            words.label_core_version,
            of(|snapshot| snapshot.node.version.clone()),
        )
        .test_id("detail-version"),
    ])
}

// ----- the tabs -----

/// The tab strip.
fn tabs(store: Store) -> View {
    let node = store.node();
    let managed = untracked(|| node.detail()).is_some_and(|id| is_managed(node, &id));
    let words = store.strings().node.detail;
    let mut options = vec![
        SelectOption::new(tab_id(DetailTab::Overview), words.tab_overview),
        SelectOption::new(tab_id(DetailTab::Settings), words.tab_settings),
        SelectOption::new(tab_id(DetailTab::Logs), words.tab_logs),
    ];
    if !managed {
        options.push(SelectOption::new(
            tab_id(DetailTab::Export),
            words.tab_export,
        ));
    }
    segmented_inline(
        store.tr(|s| s.node.detail.tabs_label),
        options,
        move || SharedString::from(tab_id(node.tab())),
        move |picked| {
            if let Some(tab) = tab_of(picked) {
                node.set_tab(tab);
            }
        },
        "detail-tab",
    )
}

/// The tab chosen: made again when another network opens or the tab changes.
fn body(store: Store) -> View {
    let node = store.node();
    view().child(dynamic(move || {
        let id = node.detail()?;
        Some(untracked(|| panel(store, id)).into_element())
    }))
}

/// The content of the tab chosen.
fn panel(store: Store, id: ProfileId) -> View {
    let node = store.node();
    let managed = is_managed(node, &id);
    view().child(dynamic(move || {
        // A managed network has nothing to export.
        let tab = match node.tab() {
            DetailTab::Export if managed => DetailTab::Overview,
            tab => tab,
        };
        let id = id.clone();
        untracked(|| match tab {
            DetailTab::Overview => overview::tab(store, id, managed).into_element(),
            DetailTab::Settings => settings::tab(store, id, managed).into_element(),
            DetailTab::Logs => logs::tab(store, id).into_element(),
            DetailTab::Export => export::tab(store).into_element(),
        })
    }))
}

// ----- the footer -----

/// The sticky footer: Cancel and Save on the Settings tab; the age of the reading and Close on the others.
fn footer(store: Store) -> View {
    let node = store.node();
    view().child(dynamic(move || {
        let id = node.detail()?;
        let tab = node.tab();
        Some(untracked(|| actions_for(store, id, tab)).into_element())
    }))
}

/// The footer of `tab`.
fn actions_for(store: Store, id: ProfileId, tab: DetailTab) -> View {
    let node = store.node();
    if tab != DetailTab::Settings {
        let running = {
            let id = id.clone();
            computed(move || node.snapshot(&id).is_some())
        };
        let read_at = move || {
            node.detail()
                .and_then(|id| node.snapshot(&id))
                .map(|snapshot| snapshot.read_at_ms)
        };
        return view()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_3()
            .child(when(
                move || running.get(),
                move || {
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(live_dot(|| true))
                        .child(age_text(
                            read_at,
                            store.tr(|s| s.common.updated_ago),
                            store.tr(|s| s.common.updated_now),
                        ))
                },
            ))
            .child(tag(
                action_button(
                    ButtonKind::Secondary,
                    store.tr(|s| s.common.close),
                    move || node.close_detail(),
                ),
                "detail-close",
            ));
    }
    let active = computed(move || node.status(&id).is_active());
    let saving = move || {
        node.editor()
            .with(|draft| draft.as_ref().is_some_and(|draft| draft.saving))
    };
    view()
        .gap_2()
        .child(save_failure_view(store, move || {
            node.editor()
                .with(|draft| draft.as_ref().and_then(|draft| draft.failure.clone()))
        }))
        .child(when(
            move || active.get(),
            move || {
                meta_text(store.tr(|s| s.node.detail.saving_restarts)).test_id("detail-save-note")
            },
        ))
        .child(
            dialog_footer()
                .child(tag(
                    action_button(
                        ButtonKind::Ghost,
                        store.tr(|s| s.common.cancel),
                        move || node.close_detail(),
                    ),
                    "detail-cancel",
                ))
                .child(tag(
                    busy_button(
                        ButtonKind::Primary,
                        store.tr(|s| s.common.save),
                        saving,
                        move || node.editor_save(),
                    ),
                    "detail-save",
                )),
        )
}
