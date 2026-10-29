//! Networks: every network across a server's machines, grouped by name in one table. Also the row of one network on
//! one machine, which the machine's drawer uses too.

use std::{collections::BTreeMap, time::Duration};

use nsh_base::theme;
use nsh_core::prelude::*;
use uuid::Uuid;

use super::{
    account::{failure_banner, server_toolbar},
    format::remote_error_text,
    gate::{signed_in, template},
    tr,
};
use crate::{
    components::{
        ButtonKind, CalloutKind, Columns, Icon, Lamp, MenuAction, action_button, age_text,
        app_color, body_text, busy_button, callout, column, columns, confirm, data_table,
        empty_panel, machine_text, menu_action, meta_text, more_menu, neutral_chip, search_field,
        status_chip, summary_cell, summary_strip, table_group_header, table_row,
    },
    core::{
        remote::{PeersSummary, RemoteError, ServerNetwork},
        target::TargetId,
    },
    i18n::fill,
    store::{Page, Store, console::EditorMode},
};

/// How often every machine's networks are read again while the page shows.
const POLL: Duration = Duration::from_secs(15);

/// Failed reads in a row before a machine's networks get a warning: a single one is usually a proxied call that
/// missed easytier-web's fixed 5 s timeout and heals on the next poll; the row shows its data's age meanwhile.
const WARN_AFTER_FAILURES: u32 = 2;

/// One network on one machine: what a row shows.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct NetworkItem {
    /// The machine.
    pub machine: Uuid,
    /// The machine's hostname.
    pub host: String,
    /// The network on it.
    pub network: ServerNetwork,
}

/// The networks of every machine with the same name.
#[derive(Debug, Clone, PartialEq)]
struct Group {
    name: String,
    members: Vec<NetworkItem>,
}

/// What the page shows instead of, or besides, the groups.
#[derive(Debug, Clone, PartialEq)]
enum Overview {
    Loading,
    Failed(RemoteError),
    NoMachines,
    NoNetworks,
    Groups,
}

/// Where a network row is shown, which decides its columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Placement {
    /// Under its network's name on the Networks page: the machine is the row's title.
    Group,
    /// In a machine's drawer: the network is the row's title.
    Drawer,
}

/// The page of a signed-in server.
pub(super) fn ready(store: Store, id: TargetId) -> AnyElement {
    let console = store.console();
    console.refresh_all_networks(id);
    let _poll = set_interval(POLL, move || console.refresh_all_networks(id));
    let overview = computed(move || overview_of(store, id));
    let errors = computed(move || errors_of(store, id));
    let search = search_field(
        tr(store, |c| c.search_networks),
        move || console.network_query(),
        move |query| console.set_network_query(query),
    )
    .clear_label(tr(store, |c| c.search_clear))
    .test_id("network-search");
    view()
        .gap_4()
        .child(server_toolbar(
            store,
            id,
            vec![search.into_element()],
            true,
            move || console.refresh_all_networks(id),
            move || console.machines(id).read_at_ms,
        ))
        .child(failure_banner(store, id))
        .child(dynamic(move || {
            errors
                .get()
                .into_iter()
                .map(|(host, error)| {
                    view()
                        .gap_1()
                        .child(callout(
                            CalloutKind::Warning,
                            template(store, |c| c.networks_machine_failed, vec![("host", host)]),
                        ))
                        .child(meta_text(SharedString::from(remote_error_text(
                            store.strings(),
                            &error,
                        ))))
                })
                .collect::<Vec<_>>()
        }))
        .child(dynamic(move || match overview.get() {
            Overview::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
            Overview::Failed(error) => {
                untracked(|| load_failed(store, &error, move || console.refresh_all_networks(id)))
            }
            Overview::NoMachines => panel_of(empty_panel(
                Icon::Server,
                tr(store, |c| c.machines_empty_title),
                tr(store, |c| c.machines_empty_body),
                None,
            )),
            Overview::NoNetworks => panel_of(empty_panel(
                Icon::Network,
                tr(store, |c| c.networks_empty_title),
                tr(store, |c| c.networks_empty_body),
                Some(
                    action_button(
                        ButtonKind::Secondary,
                        store.tr(|s| s.page_machines),
                        move || store.open(Page::Machines),
                    )
                    .into_element(),
                ),
            )),
            Overview::Groups => untracked(|| table(store, id)),
        }))
        .into_element()
}

/// The actions this page puts in the page header: importing a config into a machine, while a server is signed in.
pub(super) fn header_actions(store: Store) -> AnyElement {
    dynamic(move || {
        let id = signed_in(store)?;
        Some(
            view()
                .test_id("network-import")
                .child(action_button(
                    ButtonKind::Secondary,
                    tr(store, |c| c.import_config),
                    move || store.console().open_import(id),
                ))
                .into_element(),
        )
    })
}

/// An empty state inside a panel with a hairline, like a table.
fn panel_of(content: View) -> AnyElement {
    view()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .child(content)
        .into_element()
}

/// A failed read with its reason and a retry.
pub(super) fn load_failed(
    store: Store,
    error: &RemoteError,
    retry: impl Fn() + 'static,
) -> AnyElement {
    view()
        .gap_1()
        .child(
            callout(CalloutKind::Danger, tr(store, |c| c.load_failed))
                .action(store.tr(|s| s.common.retry), retry),
        )
        .child(meta_text(SharedString::from(remote_error_text(
            store.strings(),
            error,
        ))))
        .into_element()
}

fn overview_of(store: Store, id: TargetId) -> Overview {
    let console = store.console();
    let machines = console.machines(id);
    let Some(list) = machines.value else {
        return match machines.error {
            Some(error) => Overview::Failed(error),
            None => Overview::Loading,
        };
    };
    if list.is_empty() {
        return Overview::NoMachines;
    }
    let feeds = console.all_networks(id);
    let any_network = list.iter().any(|machine| {
        feeds
            .get(&machine.id)
            .and_then(|feed| feed.value.as_ref())
            .is_some_and(|networks| !networks.is_empty())
    });
    if any_network {
        return Overview::Groups;
    }
    let pending = list.iter().any(|machine| {
        feeds
            .get(&machine.id)
            .is_none_or(|feed| feed.loading || (feed.value.is_none() && feed.error.is_none()))
    });
    if pending {
        Overview::Loading
    } else {
        Overview::NoNetworks
    }
}

fn groups_of(store: Store, id: TargetId) -> Vec<Group> {
    let console = store.console();
    let machines = console.machines(id).value.unwrap_or_default();
    let feeds = console.all_networks(id);
    let mut by_name: BTreeMap<String, Vec<NetworkItem>> = BTreeMap::new();
    for machine in machines {
        let Some(networks) = feeds.get(&machine.id).and_then(|feed| feed.value.clone()) else {
            continue;
        };
        for network in networks {
            by_name
                .entry(network.name.clone())
                .or_default()
                .push(NetworkItem {
                    machine: machine.id,
                    host: machine.hostname.clone(),
                    network,
                });
        }
    }
    by_name
        .into_iter()
        .map(|(name, mut members)| {
            members.sort_by(|a, b| a.host.cmp(&b.host));
            Group { name, members }
        })
        .collect()
}

/// Whether `item` matches the lowercase search `query`: its network's name, its machine or its address holds it.
fn matches(item: &NetworkItem, query: &str) -> bool {
    query.is_empty()
        || item.network.name.to_lowercase().contains(query)
        || item.network.instance_name.to_lowercase().contains(query)
        || item.host.to_lowercase().contains(query)
        || item
            .network
            .virtual_ipv4
            .is_some_and(|ip| ip.to_string().contains(query))
}

/// The groups the search keeps, each with the members that match (or all of them when the network's name matches).
fn filtered(groups: Vec<Group>, query: &str) -> Vec<Group> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return groups;
    }
    groups
        .into_iter()
        .filter_map(|group| {
            if group.name.to_lowercase().contains(&query) {
                return Some(group);
            }
            let members: Vec<NetworkItem> = group
                .members
                .into_iter()
                .filter(|item| matches(item, &query))
                .collect();
            (!members.is_empty()).then_some(Group {
                name: group.name,
                members,
            })
        })
        .collect()
}

/// The machines whose networks could not be read repeatedly (or never), by hostname.
fn errors_of(store: Store, id: TargetId) -> Vec<(String, RemoteError)> {
    let console = store.console();
    let machines = console.machines(id).value.unwrap_or_default();
    let feeds = console.all_networks(id);
    machines
        .into_iter()
        .filter_map(|machine| {
            let feed = feeds.get(&machine.id)?;
            let error = feed.error.clone()?;
            let warn = feed.failures >= WARN_AFTER_FAILURES || feed.value.is_none();
            warn.then_some((machine.hostname, error))
        })
        .collect()
}

/// What the strip above the table counts.
#[derive(Debug, Clone, PartialEq, Default)]
struct Stats {
    names: usize,
    instances: usize,
    running: usize,
    peers: PeersSummary,
    reported: bool,
}

fn stats_of(groups: &[Group]) -> Stats {
    let mut stats = Stats {
        names: groups.len(),
        ..Stats::default()
    };
    for item in groups.iter().flat_map(|group| group.members.iter()) {
        stats.instances = stats.instances.saturating_add(1);
        if item.network.running {
            stats.running = stats.running.saturating_add(1);
        }
        if let Some(peers) = item.network.peers {
            stats.reported = true;
            stats.peers.total = stats.peers.total.saturating_add(peers.total);
            stats.peers.direct = stats.peers.direct.saturating_add(peers.direct);
            stats.peers.relayed = stats.peers.relayed.saturating_add(peers.relayed);
        }
    }
    stats
}

/// The summary strip and the grouped table.
fn table(store: Store, id: TargetId) -> AnyElement {
    let console = store.console();
    let all = computed(move || groups_of(store, id));
    let groups = computed(move || filtered(all.get(), &console.network_query()));
    let stats = computed(move || stats_of(&all.get()));
    let cols = group_columns(store);
    let group_cols = cols.clone();
    let reported = computed(move || stats.with(|stats| stats.reported));
    view()
        .gap_4()
        .child(dynamic(move || {
            let with_peers = reported.get();
            untracked(|| summary(store, stats, with_peers).into_element())
        }))
        .child(
            data_table(&cols)
                .body(each(
                    move || groups.get(),
                    |group: &Group| group.name.clone(),
                    move |group, _| group_view(store, id, &group_cols, group),
                ))
                .empty(
                    move || groups.with(|groups| groups.is_empty()),
                    move || {
                        empty_panel(
                            Icon::Search,
                            tr(store, |c| c.no_match_title),
                            tr(store, |c| c.no_match_body),
                            Some(
                                action_button(
                                    ButtonKind::Secondary,
                                    tr(store, |c| c.search_clear),
                                    move || console.set_network_query(""),
                                )
                                .into_element(),
                            ),
                        )
                        .into_element()
                    },
                )
                .test_id("networks-table"),
        )
        .into_element()
}

/// The strip above the table: network names, instances running of all, and, once a machine reported them, the peers.
fn summary(store: Store, stats: Memo<Stats>, with_peers: bool) -> View {
    let mut cells = vec![
        summary_cell(tr(store, |c| c.sum_network_names), move || {
            stats.with(|stats| stats.names.to_string())
        })
        .test_id("summary-network-names"),
        summary_cell(tr(store, |c| c.sum_instances), move || {
            stats.with(|stats| stats.running.to_string())
        })
        .unit(move || {
            SharedString::from(fill(
                store.strings().console.sum_of_total,
                &[(
                    "total",
                    stats.with(|stats| stats.instances).to_string().as_str(),
                )],
            ))
        })
        .test_id("summary-instances"),
    ];
    if with_peers {
        cells.push(
            summary_cell(tr(store, |c| c.sum_peers), move || {
                stats.with(|stats| stats.peers.total.to_string())
            })
            .bar(
                Lamp::Good,
                move || stats.with(|stats| stats.peers.direct.to_string()),
                tr(store, |c| c.peers_direct),
            )
            .bar(
                Lamp::Degraded,
                move || stats.with(|stats| stats.peers.relayed.to_string()),
                tr(store, |c| c.peers_relayed),
            )
            .test_id("summary-peers"),
        );
    }
    summary_strip(cells)
}

/// The columns of the grouped table.
fn group_columns(store: Store) -> Columns {
    columns(vec![
        column(tr(store, |c| c.col_machine)).flex(),
        column(tr(store, |c| c.col_status)).width(120.0),
        column(tr(store, |c| c.col_virtual_ip))
            .width(144.0)
            .mono()
            .right(),
        column(tr(store, |c| c.col_peers))
            .width(224.0)
            .hint(tr(store, |c| c.col_peers_hint))
            .hide_below(1000.0),
        column("").width(136.0).actions(),
    ])
}

/// The columns of a machine's networks in its drawer.
pub(super) fn drawer_columns(store: Store) -> Columns {
    columns(vec![
        column(tr(store, |c| c.col_network)).flex(),
        column(tr(store, |c| c.col_status)).width(112.0),
        column(tr(store, |c| c.col_virtual_ip))
            .width(136.0)
            .mono()
            .right(),
        column(tr(store, |c| c.col_peers))
            .width(56.0)
            .mono()
            .right()
            .hint(tr(store, |c| c.col_peers_hint)),
        column("").width(128.0).actions(),
    ])
}

/// One network name: its header (name, how many machines, status bars) and its machines' rows.
fn group_view(store: Store, id: TargetId, cols: &Columns, group: ReadSignal<Group>) -> View {
    let name = group.get_untracked().name;
    let count = move || group.with(|group| group.members.len());
    let rows_columns = cols.clone();
    view()
        .child(
            table_group_header()
                .child(
                    text(SharedString::from(name))
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .number_of_lines(1)
                        .min_w_0()
                        .text_color(move || theme().foreground),
                )
                .child(neutral_chip(move || {
                    SharedString::from(fill(
                        store.strings().console.machines_count,
                        &[("count", count().to_string().as_str())],
                    ))
                }))
                .child(view().flex_1())
                .child(group_bars(store, group)),
        )
        .child(each(
            move || group.with(|group| group.members.clone()),
            |item: &NetworkItem| (item.machine, item.network.instance_id),
            move |item, _| network_row(store, id, &rows_columns, item, Placement::Group),
        ))
}

/// A group's status bars: how many of its networks run, are switched off or reported an error (none for zero).
fn group_bars(store: Store, group: ReadSignal<Group>) -> View {
    view().child(dynamic(move || {
        let (running, off, failed) = group.with(|group| {
            group
                .members
                .iter()
                .fold((0_usize, 0_usize, 0_usize), |sum, item| {
                    match state_of(&item.network) {
                        State::Running => (sum.0.saturating_add(1), sum.1, sum.2),
                        State::Off => (sum.0, sum.1.saturating_add(1), sum.2),
                        State::Failed => (sum.0, sum.1, sum.2.saturating_add(1)),
                    }
                })
        });
        Some(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_x_3()
                .gap_y_1()
                .child((running > 0).then(|| {
                    count_bar(app_color("success"), running, tr(store, |c| c.net_running))
                }))
                .child((off > 0).then(|| {
                    count_bar(theme().muted_foreground, off, tr(store, |c| c.net_disabled))
                }))
                .child(
                    (failed > 0).then(|| {
                        count_bar(theme().destructive, failed, tr(store, |c| c.net_error))
                    }),
                )
                .into_element(),
        )
    }))
}

/// A 3 x 14 pt bar in a state's hue, its tabular count and its word.
fn count_bar(hue: AdaptiveColor, count: usize, word: impl IntoReactive<SharedString>) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_1p5()
        .child(
            view()
                .w(Points(3.0))
                .h(Points(14.0))
                .flex_shrink_0()
                .rounded_full()
                .accessible(false)
                .bg(move || hue),
        )
        .child(machine_text(SharedString::from(count.to_string())).number_of_lines(1))
        .child(meta_text(word).number_of_lines(1))
}

/// What a network instance is doing: a switched-off one is idle; one that reported an error failed; one that runs
/// is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Running,
    Off,
    Failed,
}

fn state_of(network: &ServerNetwork) -> State {
    if network.disabled {
        State::Off
    } else if network.error.is_some() {
        State::Failed
    } else if network.running {
        State::Running
    } else {
        State::Off
    }
}

/// The row of one network on one machine: its state, address, peers and what can be done with it. Pressing it opens
/// the network's config editor, or the machine's drawer where the console may not edit the network.
pub(super) fn network_row(
    store: Store,
    id: TargetId,
    cols: &Columns,
    item: ReadSignal<NetworkItem>,
    placement: Placement,
) -> View {
    let console = store.console();
    let first = item.get_untracked();
    let (machine, instance) = (first.machine, first.network.instance_id);
    let editable = first.network.is_editable();
    let in_group = placement == Placement::Group;
    let acting = move || console.is_acting(instance);
    let state = move || item.with(|item| state_of(&item.network));
    let lamp = move || match state() {
        State::Running => Lamp::Good,
        State::Off => Lamp::Idle,
        State::Failed => Lamp::Failed,
    };
    let word = move || {
        let strings = store.strings().console;
        match state() {
            State::Running => strings.net_running,
            State::Off => strings.net_disabled,
            State::Failed => strings.net_error,
        }
    };
    let title = move || {
        item.with(|item| {
            if in_group {
                item.host.clone()
            } else {
                item.network.name.clone()
            }
        })
    };
    // The last read of this machine failed: the row keeps the older data and says how old it is.
    let stale_since = move || {
        console
            .all_networks(id)
            .get(&machine)
            .filter(|feed| feed.error.is_some())
            .and_then(|feed| feed.read_at_ms)
    };
    let name_cell = view()
        .flex_row()
        .items_center()
        .gap_2()
        .min_w_0()
        .child(
            body_text(move || SharedString::from(title()))
                .number_of_lines(1)
                .min_w_0()
                .font_weight(FontWeight::MEDIUM),
        )
        .child((!editable).then(|| neutral_chip(tr(store, |c| c.net_read_only))))
        .child(dynamic(move || {
            stale_since().is_some().then(|| {
                age_text(
                    stale_since,
                    tr(store, |c| c.net_stale),
                    tr(store, |c| c.net_stale_now),
                )
                .number_of_lines(1)
                .test_id(&format!("network-stale-{instance}"))
            })
        }));
    let toggle_label = move || {
        let strings = store.strings().console;
        if item.with(|item| item.network.disabled) {
            strings.net_enable
        } else {
            strings.net_disable
        }
    };
    let toggle = busy_button(ButtonKind::Secondary, toggle_label, acting, move || {
        let current = item.get_untracked();
        console.set_network_disabled(
            id,
            machine,
            instance,
            current.network.name.clone(),
            !current.network.disabled,
        );
    });
    let running = first.network.running;
    let mut menu: Vec<MenuAction> = Vec::new();
    if editable {
        menu.push(
            menu_action(Icon::Edit, tr(store, |c| c.net_edit_config), move || {
                console.open_edit(id, machine, instance);
            })
            .test_id(format!("network-edit-{instance}")),
        );
    }
    if running {
        menu.push(
            menu_action(Icon::Peers, tr(store, |c| c.net_peers), move || {
                let current = item.get_untracked();
                console.open_machine_peers(id, machine, instance, current.network.name);
            })
            .test_id(format!("network-peers-{instance}")),
        );
    }
    if in_group {
        menu.push(
            menu_action(Icon::Server, tr(store, |c| c.open_machine), move || {
                console.select_machine(id, Some(machine));
            })
            .test_id(format!("network-machine-{instance}")),
        );
    }
    if editable {
        menu.push(
            menu_action(Icon::Trash, store.tr(|s| s.common.delete), move || {
                confirm_delete(store, id, item);
            })
            .destructive()
            .separated()
            .test_id(format!("network-delete-{instance}")),
        );
    }
    let actions = view()
        .flex_row()
        .items_center()
        .gap_1()
        .child(editable.then_some(toggle))
        .child((!menu.is_empty()).then(|| {
            more_menu(tr(store, |c| c.more_actions), menu)
                .test_id(format!("network-more-{instance}"))
        }));
    let peers_cell = if in_group {
        peers_detail(store, item)
    } else {
        peers_total(item)
    };
    let row = table_row(cols)
        .cell(name_cell)
        .cell(status_chip(lamp, word))
        .text(move || {
            item.with(|item| {
                item.network
                    .virtual_ipv4
                    .map(|ip| ip.to_string())
                    .unwrap_or_default()
            })
        });
    // The wide table has a peers column in both placements; it is hidden below its breakpoint in the group table.
    let row = row
        .cell(peers_cell)
        .cell(actions)
        .detail(dynamic(move || {
            let error = item.with(|item| item.network.error.clone())?;
            Some(
                view()
                    .px_4()
                    .pb_3()
                    .child(callout(CalloutKind::Danger, SharedString::from(error)))
                    .into_element(),
            )
        }))
        .selected(move || {
            let editor = console.editor();
            editor.open && editor.mode == EditorMode::Edit { instance }
        })
        .label(move || SharedString::from(title()))
        .test_id(format!("network-{instance}"));
    let row = if editable {
        row.on_press(move || console.open_edit(id, machine, instance))
    } else {
        row.on_press(move || console.select_machine(id, Some(machine)))
    };
    row.build()
}

/// The peers of a network in a table cell: the total, then how many are direct and how many relayed.
fn peers_detail(store: Store, item: ReadSignal<NetworkItem>) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .min_w_0()
        .child(dynamic(move || {
            let Some(peers) = item.with(|item| item.network.peers) else {
                return Some(
                    machine_text("\u{2014}")
                        .text_color(move || theme().muted_foreground)
                        .into_element(),
                );
            };
            Some(
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(machine_text(SharedString::from(peers.total.to_string())))
                    .child(count_bar(
                        app_color("success"),
                        peers.direct,
                        tr(store, |c| c.peers_direct),
                    ))
                    .child(count_bar(
                        app_color("warning"),
                        peers.relayed,
                        tr(store, |c| c.peers_relayed),
                    ))
                    .into_element(),
            )
        }))
}

/// The peers of a network as one number, a muted dash when the machine did not report.
fn peers_total(item: ReadSignal<NetworkItem>) -> View {
    view().child(
        machine_text(move || {
            item.with(|item| {
                item.network
                    .peers
                    .map(|peers| peers.total.to_string())
                    .unwrap_or_else(|| "\u{2014}".to_owned())
            })
        })
        .number_of_lines(1)
        .text_color(move || {
            if item.with(|item| item.network.peers.is_some()) {
                theme().foreground
            } else {
                theme().muted_foreground
            }
        }),
    )
}

/// Asks, naming the network and its machine, then deletes it.
fn confirm_delete(store: Store, id: TargetId, item: ReadSignal<NetworkItem>) {
    let strings = store.strings();
    let current = item.get_untracked();
    let description = fill(
        strings.console.delete_network_body,
        &[
            ("name", current.network.name.as_str()),
            ("host", current.host.as_str()),
        ],
    );
    let asked = confirm(
        fill(
            strings.console.delete_network_title,
            &[("name", current.network.name.as_str())],
        ),
        description,
        strings.common.delete,
        strings.common.cancel,
        true,
    );
    let console = store.console();
    console.when_confirmed(asked, move || {
        console.delete_network(
            id,
            current.machine,
            current.network.instance_id,
            current.network.name,
        );
    });
}
