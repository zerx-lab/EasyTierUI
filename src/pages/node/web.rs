//! The Networks and Peers pages in the web build. A browser cannot join a network, so they manage a machine
//! enrolled on the server that served the page: behind the console's sign-in gate, a toolbar picks the machine
//! (remembered across restarts) and the page shows what the console's machine drawer shows for it.

use std::time::Duration;

use nsh_base::theme;
use nsh_core::prelude::*;
use uuid::Uuid;

use super::super::console::embed;
use crate::{
    components::{
        ButtonKind, CalloutKind, Icon, SelectOption, action_button, callout, empty_panel,
        meta_text, select,
    },
    core::{remote::RemoteError, target::TargetId},
    store::{Page, Store},
};

/// How often the server's machine list is read again while a page shows.
const MACHINES_POLL: Duration = Duration::from_secs(10);

/// How often the chosen machine's networks (and peers) are read again while a page shows.
const MACHINE_POLL: Duration = Duration::from_secs(15);

/// Which of the two pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Networks,
    Peers,
}

/// Which machine the page manages, or why none.
#[derive(Debug, Clone, PartialEq)]
enum Choice {
    Loading,
    Failed(RemoteError),
    /// The server lists no machine.
    NoMachines,
    /// Several machines and none was chosen.
    Pick,
    /// The remembered machine is not listed any more.
    Gone,
    Machine(Uuid),
}

/// The machine's networks.
pub(super) fn networks(store: Store) -> AnyElement {
    embed::gate(store, move |id| ready(store, id, Tab::Networks))
}

/// The peers of the machine's running networks.
pub(super) fn peers(store: Store) -> AnyElement {
    embed::gate(store, move |id| ready(store, id, Tab::Peers))
}

/// What the page manages on server `id` (tracked).
fn choose(store: Store, id: TargetId) -> Choice {
    let console = store.console();
    let feed = console.machines(id);
    let Some(machines) = feed.value else {
        return match feed.error {
            Some(error) => Choice::Failed(error),
            None => Choice::Loading,
        };
    };
    match (console.node_machine(id), machines.as_slice()) {
        (_, []) => Choice::NoMachines,
        (Some(kept), listed) if listed.iter().any(|machine| machine.id == kept) => {
            Choice::Machine(kept)
        }
        (Some(_), _) => Choice::Gone,
        (None, [only]) => Choice::Machine(only.id),
        (None, _) => Choice::Pick,
    }
}

/// The page of a signed-in server.
fn ready(store: Store, id: TargetId, tab: Tab) -> AnyElement {
    let console = store.console();
    let now = embed::clock();
    let choice = computed(move || choose(store, id));
    let _machines = set_interval(MACHINES_POLL, move || console.refresh_machines(id));
    let _machine = set_interval(MACHINE_POLL, move || {
        if let Choice::Machine(mid) = choose(store, id) {
            console.refresh_networks(id, mid);
            if tab == Tab::Peers {
                console.refresh_peers();
            }
        }
    });
    let options = computed(move || {
        let mut listed: Vec<(String, String)> = console
            .machines(id)
            .value
            .unwrap_or_default()
            .into_iter()
            .map(|machine| (machine.id.to_string(), machine.hostname))
            .collect();
        listed.sort_by(|a, b| a.1.cmp(&b.1));
        listed
    });
    let picker = dynamic(move || {
        let listed = options.get();
        if listed.is_empty() {
            return None;
        }
        Some(untracked(|| machine_picker(store, id, choice, listed)))
    });
    let actions = dynamic(move || {
        if tab != Tab::Networks {
            return None;
        }
        let Choice::Machine(mid) = choice.get() else {
            return None;
        };
        Some(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(view().test_id("web-node-import").child(action_button(
                    ButtonKind::Secondary,
                    store.tr(|s| s.console.import_config),
                    move || {
                        console.open_import(id);
                        console.set_editor_machine(mid);
                    },
                )))
                .child(view().test_id("web-node-create").child(action_button(
                    ButtonKind::Primary,
                    store.tr(|s| s.console.create_network),
                    move || console.open_create(id, mid),
                )))
                .into_element(),
        )
    });
    view()
        .gap_4()
        .child(embed::server_toolbar(
            store,
            id,
            vec![picker, actions],
            move || {
                console.refresh_machines(id);
                if let Choice::Machine(mid) = choose(store, id) {
                    console.refresh_networks(id, mid);
                    if tab == Tab::Peers {
                        console.refresh_peers();
                    }
                }
            },
            move || console.machines(id).read_at_ms,
        ))
        .child(embed::failure_banner(store, id))
        .child(dynamic(move || match choice.get() {
            Choice::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
            Choice::Failed(error) => untracked(|| {
                embed::load_failed(store, &error, move || console.refresh_machines(id))
            }),
            Choice::NoMachines => panel(empty_panel(
                Icon::Server,
                store.tr(|s| s.node.web.none_title),
                store.tr(|s| s.node.web.none_body),
                Some(
                    action_button(
                        ButtonKind::Secondary,
                        store.tr(|s| s.node.web.none_action),
                        move || store.open(Page::Machines),
                    )
                    .into_element(),
                ),
            )),
            Choice::Pick => panel(empty_panel(
                Icon::Server,
                store.tr(|s| s.node.web.pick_title),
                store.tr(|s| s.node.web.pick_body),
                None,
            )),
            Choice::Gone => panel(empty_panel(
                Icon::AlertCircle,
                store.tr(|s| s.node.web.gone_title),
                store.tr(|s| s.node.web.gone_body),
                None,
            )),
            Choice::Machine(mid) => untracked(|| machine_body(store, id, mid, tab, now)),
        }))
        .into_element()
}

/// The machine choice, in the toolbar.
fn machine_picker(
    store: Store,
    id: TargetId,
    choice: Memo<Choice>,
    machines: Vec<(String, String)>,
) -> AnyElement {
    let mut options = vec![SelectOption::new(
        "",
        store.strings().node.web.picker_choose,
    )];
    options.extend(
        machines
            .into_iter()
            .map(|(machine, host)| SelectOption::new(machine, host)),
    );
    select(
        store.tr(|s| s.node.web.picker_label),
        options,
        move || match choice.get() {
            Choice::Machine(mid) => SharedString::from(mid.to_string()),
            _ => SharedString::from(""),
        },
        move |value| {
            if let Ok(machine) = Uuid::parse_str(value) {
                store.console().pick_node_machine(id, machine);
            }
        },
    )
    .inline()
    .test_id("web-node-machine")
    .build()
    .into_element()
}

/// The chosen machine's networks or peers, with a word when it has not reported lately.
fn machine_body(store: Store, id: TargetId, mid: Uuid, tab: Tab, now: Signal<f64>) -> AnyElement {
    let console = store.console();
    console.refresh_networks(id, mid);
    let offline = computed(move || {
        console
            .machines(id)
            .value
            .and_then(|machines| machines.into_iter().find(|machine| machine.id == mid))
            .is_some_and(|machine| !machine.is_online(now.get()))
    });
    view()
        .gap_4()
        .child(dynamic(move || {
            offline.get().then(|| {
                callout(CalloutKind::Warning, store.tr(|s| s.node.web.offline)).into_element()
            })
        }))
        .child(match tab {
            Tab::Networks => embed::machine_networks(store, id, mid),
            Tab::Peers => embed::machine_peers(store, id, mid),
        })
        .into_element()
}

/// An empty state inside a panel with a hairline, like a table.
fn panel(content: View) -> AnyElement {
    view()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .child(content)
        .into_element()
}
