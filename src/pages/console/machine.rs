//! The machine drawer: one machine's facts, its networks with their actions, and the views of one of its running
//! networks (peers, node, routes, relay, stats, connections, forwarding, access) and of its core (logs). Opened by a
//! row of the machine table, or from a network's menu on the Networks page.

use std::rc::Rc;

use nsh_core::prelude::*;
use uuid::Uuid;

use super::{
    format::{format_ago, format_bytes, format_latency, format_loss},
    inspect::{self, feed_view, toolbar},
    machines::clock,
    networks::{NetworkItem, Placement, drawer_columns, load_failed, network_row},
    runtime, tr,
};
use crate::{
    components::{
        ButtonKind, CalloutKind, CopyLabels, Icon, Lamp, SelectOption, Thresholds, action_button,
        callout, column, columns, data_table, dialog_footer, drawer, empty_panel, fact, fact_mono,
        kv_grid, meta_text, segmented_inline, select, status_chip, table_row, threshold_bar,
    },
    core::{
        remote::{Machine, PeerLinks, RemoteError},
        target::TargetId,
    },
    store::{
        Store,
        console::{Feed, MachineTab},
    },
};

/// What the drawer's body shows for the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Detail {
    Loading,
    Known,
    Gone,
}

/// What a list inside the drawer shows.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Loading,
    Failed(RemoteError),
    Empty,
    Rows,
}

/// The drawer of the machine selected on the current server, mounted once beside the layout.
pub(super) fn drawer_view(store: Store) -> impl IntoElement {
    let console = store.console();
    let now = clock();
    let machine = computed(move || {
        let id = store.current_target_id();
        let selected = console.selected_machine(id)?;
        console
            .machines(id)
            .value?
            .into_iter()
            .find(|machine| machine.id == selected)
    });
    let online = move || {
        machine.with(|machine| {
            machine
                .as_ref()
                .is_some_and(|machine| machine.is_online(now.get()))
        })
    };
    drawer(
        move || {
            console
                .selected_machine(store.current_target_id())
                .is_some()
        },
        move || {
            machine.with(|machine| {
                machine
                    .as_ref()
                    .map(|machine| machine.hostname.clone())
                    .unwrap_or_default()
            })
        },
        store.tr(|s| s.common.close),
        move || console.select_machine(store.current_target_id(), None),
    )
    .kind(store.tr(|s| s.kind_machine))
    .status(
        move || if online() { Lamp::Good } else { Lamp::Idle },
        move || {
            let strings = store.strings().console;
            if online() {
                strings.machine_online
            } else {
                strings.machine_offline
            }
        },
    )
    .meta(move || facts(store, machine, now))
    .tabs(move || tab_switcher(store))
    .body(move || {
        let id = store.current_target_id();
        let Some(mid) = console.selected_machine(id) else {
            return view().into_element();
        };
        body(store, id, mid)
    })
    .footer(move || {
        let id = store.current_target_id();
        dialog_footer().child(action_button(
            ButtonKind::Primary,
            tr(store, |c| c.create_network),
            move || {
                if let Some(mid) = console.selected_machine(id) {
                    console.open_create(id, mid);
                }
            },
        ))
    })
    .test_id("machine-drawer")
}

/// The facts under the drawer's header.
fn facts(store: Store, machine: Memo<Option<Machine>>, now: Signal<f64>) -> View {
    let read = move |pick: fn(&Machine) -> String| {
        machine.with(|machine| machine.as_ref().map(pick).unwrap_or_default())
    };
    let labels = || CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied));
    kv_grid(vec![
        fact(tr(store, |c| c.col_os), move || read(|m| m.os.clone())),
        fact_mono(tr(store, |c| c.col_version), move || {
            read(|m| m.version.clone())
        }),
        fact(tr(store, |c| c.col_location), move || {
            read(|m| m.location.clone().unwrap_or_default())
        }),
        fact_mono(tr(store, |c| c.col_reported), move || {
            machine
                .with(|machine| machine.as_ref().and_then(|machine| machine.report_time_ms))
                .map(|reported| format_ago(store.strings(), now.get(), reported))
                .unwrap_or_default()
        }),
        fact_mono(tr(store, |c| c.machine_connected_from), move || {
            read(|m| m.client_url.clone())
        })
        .copyable(labels())
        .test_id("machine-client-url"),
        fact_mono(tr(store, |c| c.machine_id), move || {
            read(|m| m.id.to_string())
        })
        .copyable(labels())
        .test_id("machine-id"),
    ])
}

/// The body: the machine's networks or its peers, or why there is nothing to show.
fn body(store: Store, id: TargetId, mid: Uuid) -> AnyElement {
    let console = store.console();
    let state = computed(move || {
        let list = console.machines(id).value;
        match list {
            Some(machines) if machines.iter().any(|machine| machine.id == mid) => Detail::Known,
            Some(_) => Detail::Gone,
            None => Detail::Loading,
        }
    });
    dynamic(move || match state.get() {
        Detail::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
        Detail::Gone => empty_panel(
            Icon::AlertCircle,
            tr(store, |c| c.machine_gone_title),
            tr(store, |c| c.machine_gone_body),
            None,
        )
        .into_element(),
        Detail::Known => untracked(|| tab(store, id, mid)),
    })
}

/// The tab switcher: a strip for the views used most and a menu beside it for the others, so ten tabs fit the
/// drawer's width (the strip and the menu wrap onto two lines on a phone).
fn tab_switcher(store: Store) -> View {
    let console = store.console();
    let label = move |tab: MachineTab| -> &'static str {
        let words = store.strings().console;
        let inspect = store.strings().console_inspect;
        match tab {
            MachineTab::Networks => words.tab_networks,
            MachineTab::Peers => words.tab_peers,
            MachineTab::Node => inspect.tab_node,
            MachineTab::Routes => inspect.tab_routes,
            MachineTab::Relay => inspect.tab_relay,
            MachineTab::Stats => inspect.tab_stats,
            MachineTab::Connections => inspect.tab_connections,
            MachineTab::Forwarding => inspect.tab_forwarding,
            MachineTab::Access => inspect.tab_access,
            MachineTab::Logs => inspect.tab_logs,
        }
    };
    let show = move |id: &str| {
        if let Some(tab) = MachineTab::from_id(id) {
            console.show_machine_tab(tab);
        }
    };
    let mut more = vec![SelectOption::new(
        "",
        store.strings().console_inspect.tabs_more,
    )];
    more.extend(
        MachineTab::MORE
            .into_iter()
            .map(|tab| SelectOption::new(tab.id(), label(tab))),
    );
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(segmented_inline(
            tr(store, |c| c.tabs_label),
            MachineTab::MAIN
                .into_iter()
                .map(|tab| SelectOption::new(tab.id(), label(tab)))
                .collect(),
            move || SharedString::from(console.machine_tab().id()),
            show,
            "machine-tab",
        ))
        .child(
            select(
                store.tr(|s| s.console_inspect.tabs_more_label),
                more,
                move || {
                    let tab = console.machine_tab();
                    SharedString::from(if MachineTab::MORE.contains(&tab) {
                        tab.id()
                    } else {
                        ""
                    })
                },
                show,
            )
            .inline()
            .test_id("machine-tab-more")
            .build(),
        )
}

/// The part of the body the tab shows.
fn tab(store: Store, id: TargetId, mid: Uuid) -> AnyElement {
    let console = store.console();
    dynamic(move || {
        let tab = console.machine_tab();
        untracked(|| tab_content(store, id, mid, tab))
    })
}

/// What `tab` shows for machine `mid`: the per-network tabs inside the choice of the machine's running network.
pub(super) fn tab_content(store: Store, id: TargetId, mid: Uuid, tab: MachineTab) -> AnyElement {
    match tab {
        MachineTab::Networks => networks_tab(store, id, mid),
        MachineTab::Peers => peers_tab(store, id, mid),
        MachineTab::Node => instance_scope(store, id, mid, move |instance| {
            inspect::node_tab(store, id, mid, instance)
        }),
        MachineTab::Routes => instance_scope(store, id, mid, move |instance| {
            inspect::routes_tab(store, id, mid, instance)
        }),
        MachineTab::Relay => instance_scope(store, id, mid, move |instance| {
            inspect::relay_tab(store, id, mid, instance)
        }),
        MachineTab::Stats => instance_scope(store, id, mid, move |instance| {
            inspect::stats_tab(store, id, mid, instance)
        }),
        MachineTab::Connections => instance_scope(store, id, mid, move |instance| {
            runtime::connections_tab(store, id, mid, instance)
        }),
        MachineTab::Forwarding => instance_scope(store, id, mid, move |instance| {
            runtime::forwarding_tab(store, id, mid, instance)
        }),
        MachineTab::Access => instance_scope(store, id, mid, move |instance| {
            runtime::access_tab(store, id, mid, instance)
        }),
        MachineTab::Logs => runtime::logs_tab(store, id, mid),
    }
}

/// The machine's networks as a table with their actions.
pub(super) fn networks_tab(store: Store, id: TargetId, mid: Uuid) -> AnyElement {
    let console = store.console();
    let kind = computed(move || {
        let feed = console.networks_of(id, mid);
        match feed.value {
            None => match feed.error {
                Some(error) => Kind::Failed(error),
                None => Kind::Loading,
            },
            Some(networks) if networks.is_empty() => Kind::Empty,
            Some(_) => Kind::Rows,
        }
    });
    let stale = computed(move || {
        let feed = console.networks_of(id, mid);
        feed.value.and(feed.error)
    });
    let host = move || {
        console
            .machines(id)
            .value
            .and_then(|machines| machines.into_iter().find(|machine| machine.id == mid))
            .map(|machine| machine.hostname)
            .unwrap_or_default()
    };
    let cols = drawer_columns(store);
    view()
        .gap_4()
        .child(dynamic(move || {
            stale.get().map(|_| {
                callout(CalloutKind::Warning, tr(store, |c| c.refresh_failed))
                    .action(store.tr(|s| s.common.retry), move || {
                        console.refresh_networks(id, mid);
                    })
                    .into_element()
            })
        }))
        .child(dynamic(move || match kind.get() {
            Kind::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
            Kind::Failed(error) => {
                untracked(|| load_failed(store, &error, move || console.refresh_networks(id, mid)))
            }
            Kind::Empty => empty_panel(
                Icon::Network,
                tr(store, |c| c.machine_networks_empty_title),
                tr(store, |c| c.machine_networks_empty_body),
                None,
            )
            .into_element(),
            Kind::Rows => {
                let rows_columns = cols.clone();
                untracked(|| {
                    data_table(&cols)
                        .body(each(
                            move || {
                                let host = host();
                                console
                                    .networks_of(id, mid)
                                    .value
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|network| NetworkItem {
                                        machine: mid,
                                        host: host.clone(),
                                        network,
                                    })
                                    .collect::<Vec<_>>()
                            },
                            |item: &NetworkItem| item.network.instance_id,
                            move |item, _| {
                                network_row(store, id, &rows_columns, item, Placement::Drawer)
                            },
                        ))
                        .test_id("machine-networks")
                        .into_element()
                })
            }
        }))
        .into_element()
}

/// What a per-network tab shows before it has a network to show.
#[derive(Debug, Clone, PartialEq)]
enum ScopeKind {
    Loading,
    Failed(RemoteError),
    NoRunning,
    Ready,
}

/// The part of the machine's running networks the per-network tabs share: which network they are about, and the
/// words for there being none to show. `body(instance)` is the tab itself; it is built again only when the network
/// it is about changes, never when a poll reads the same networks again.
pub(super) fn instance_scope(
    store: Store,
    id: TargetId,
    mid: Uuid,
    body: impl Fn(Uuid) -> AnyElement + 'static,
) -> AnyElement {
    let console = store.console();
    let body = Rc::new(body);
    let running = computed(move || {
        console
            .networks_of(id, mid)
            .value
            .unwrap_or_default()
            .into_iter()
            .filter(|network| network.running)
            .map(|network| (network.instance_id, network.name))
            .collect::<Vec<_>>()
    });
    // The network picked by hand while it still runs, else the first one that runs.
    let chosen = computed(move || {
        let running = running.get();
        console
            .scope_instance(id, mid)
            .filter(|picked| running.iter().any(|(instance, _)| instance == picked))
            .or_else(|| running.first().map(|(instance, _)| *instance))
    });
    let kind = computed(move || {
        let feed = console.networks_of(id, mid);
        match feed.value {
            None => match feed.error {
                Some(error) => ScopeKind::Failed(error),
                None => ScopeKind::Loading,
            },
            Some(_) if running.with(|running| running.is_empty()) => ScopeKind::NoRunning,
            Some(_) => ScopeKind::Ready,
        }
    });
    dynamic(move || match kind.get() {
        ScopeKind::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
        ScopeKind::Failed(error) => {
            untracked(|| load_failed(store, &error, move || console.refresh_networks(id, mid)))
        }
        ScopeKind::NoRunning => empty_panel(
            Icon::Peers,
            tr(store, |c| c.credentials_no_network_title),
            store.tr(|s| s.console_inspect.scope_no_network_body),
            None,
        )
        .into_element(),
        ScopeKind::Ready => {
            let body = body.clone();
            untracked(|| {
                view()
                    .gap_4()
                    .child(dynamic(move || {
                        let options = running.get();
                        untracked(|| network_picker(store, id, mid, chosen, options))
                    }))
                    .child(dynamic(move || {
                        chosen.get().map(|instance| untracked(|| body(instance)))
                    }))
                    .into_element()
            })
        }
    })
}

/// The choice among the machine's running networks.
fn network_picker(
    store: Store,
    id: TargetId,
    mid: Uuid,
    chosen: Memo<Option<Uuid>>,
    networks: Vec<(Uuid, String)>,
) -> AnyElement {
    let console = store.console();
    let options = networks
        .into_iter()
        .map(|(instance, name)| SelectOption::new(instance.to_string(), name))
        .collect();
    select(
        tr(store, |c| c.credentials_network),
        options,
        move || {
            SharedString::from(
                chosen
                    .get()
                    .map(|chosen| chosen.to_string())
                    .unwrap_or_default(),
            )
        },
        move |value| {
            if let Ok(instance) = Uuid::parse_str(value) {
                console.pick_scope(id, mid, instance);
            }
        },
    )
    .test_id("machine-peers-network")
    .build()
    .into_element()
}

/// The peers of one of the machine's running networks: which network, refresh, and the table.
pub(super) fn peers_tab(store: Store, id: TargetId, mid: Uuid) -> AnyElement {
    instance_scope(store, id, mid, move |instance| {
        peers_body(store, id, mid, instance)
    })
}

/// The peer list of network `instance`: when it was read, refresh, and the table.
fn peers_body(store: Store, id: TargetId, mid: Uuid, instance: Uuid) -> AnyElement {
    let console = store.console();
    console.show_peers_of(id, mid, instance);
    let feed = move || {
        console
            .peers()
            .filter(|open| open.machine == mid && open.instance == instance)
            .map(|open| open.peers)
            .unwrap_or_default()
    };
    let refresh = move || console.refresh_peers();
    view()
        .gap_4()
        .child(toolbar(
            store,
            move || feed().read_at_ms,
            refresh,
            "machine-peers-refresh",
        ))
        .child(feed_view(
            store,
            feed,
            |peers: &Vec<PeerLinks>| peers.is_empty(),
            refresh,
            move || {
                empty_panel(
                    Icon::Peers,
                    tr(store, |c| c.peers_empty_title),
                    tr(store, |c| c.peers_empty_body),
                    None,
                )
                .into_element()
            },
            move || peers_table(store, feed),
        ))
        .into_element()
}

/// The peers of the open list as a table.
fn peers_table(
    store: Store,
    feed: impl Fn() -> Feed<Vec<PeerLinks>> + Copy + 'static,
) -> AnyElement {
    let cols = columns(vec![
        column(tr(store, |c| c.col_peer)).width(88.0).mono(),
        column(tr(store, |c| c.col_route)).width(144.0),
        column(tr(store, |c| c.col_latency))
            .width(112.0)
            .hint(tr(store, |c| c.col_latency_hint)),
        column(tr(store, |c| c.col_loss))
            .width(104.0)
            .hint(tr(store, |c| c.col_loss_hint)),
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
    ]);
    let rows_columns = cols.clone();
    data_table(&cols)
        .body(each(
            move || feed().value.unwrap_or_default(),
            |peer: &PeerLinks| peer.peer_id,
            move |peer, _| peer_row(store, &rows_columns, peer),
        ))
        .test_id("machine-peers")
        .into_element()
}

/// One peer: its id, how it is reached, latency and loss against their thresholds, and the bytes it moved.
fn peer_row(store: Store, cols: &crate::components::Columns, peer: ReadSignal<PeerLinks>) -> View {
    let open_links = move |f: fn(&PeerLinks) -> Option<f64>| peer.with(f);
    let latency = move || {
        open_links(|peer| {
            peer.links
                .iter()
                .filter(|link| !link.closed)
                .filter_map(|link| link.latency_ms)
                .reduce(f64::min)
        })
    };
    let loss = move || {
        open_links(|peer| {
            let mut open = peer.links.iter().filter(|link| !link.closed).peekable();
            open.peek()?;
            Some(open.fold(0.0_f64, |worst, link| worst.max(link.loss)))
        })
    };
    let direct = move || peer.with(|peer| peer.links.iter().any(|link| !link.closed));
    let route = move || {
        let strings = store.strings().console;
        if !direct() {
            return strings.peer_chip_relayed.to_owned();
        }
        let tunnels = peer.with(|peer| {
            let mut tunnels: Vec<String> = Vec::new();
            for link in peer.links.iter().filter(|link| !link.closed) {
                if !tunnels.contains(&link.tunnel) {
                    tunnels.push(link.tunnel.clone());
                }
            }
            tunnels.join(", ")
        });
        if tunnels.is_empty() {
            strings.peer_chip_direct.to_owned()
        } else {
            format!("{} \u{b7} {tunnels}", strings.peer_chip_direct)
        }
    };
    let total = move |pick: fn(&crate::core::remote::PeerLink) -> u64| {
        peer.with(|peer| {
            format_bytes(
                peer.links
                    .iter()
                    .fold(0_u64, |sum, link| sum.saturating_add(pick(link))),
            )
        })
    };
    table_row(cols)
        .text(move || peer.with(|peer| peer.peer_id.to_string()))
        .cell(status_chip(
            move || if direct() { Lamp::Good } else { Lamp::Degraded },
            route,
        ))
        .cell(threshold_bar(
            latency,
            move || latency().map(format_latency).unwrap_or_default(),
            Thresholds {
                warn: 100.0,
                bad: None,
                full: 300.0,
            },
        ))
        .cell(threshold_bar(
            loss,
            move || loss().map(format_loss).unwrap_or_default(),
            Thresholds {
                warn: 0.02,
                bad: Some(0.10),
                full: 0.25,
            },
        ))
        .text(move || total(|link| link.rx_bytes))
        .text(move || total(|link| link.tx_bytes))
        .build()
}
