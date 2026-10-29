//! The Networks page: a toolbar (search, state filter, the four ways to get a network), a summary strip of what the
//! core reported, and a table of this device's networks (DESIGN.md "Data table"). A row opens the network's drawer; a
//! failed network says why under its row, with the action that fixes it.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    common::{
        ask_delete, icon_label_button, lamp_of, responsive, save_failure_text, status_word, tag,
    },
    failure,
    format::{address_text, rate_parts},
};
use crate::{
    components::{
        ButtonKind, CalloutKind, ChartUnit, Columns, Icon, Lamp, SelectOption, SeriesColor,
        TableRow, action_button, app_color, body_text, busy_button, callout, column, columns,
        data_table, empty_panel, machine_text, menu_action, meta_text, more_menu, neutral_chip,
        search_field, segmented_inline, sparkline, status_lamp, summary_cell, summary_strip,
        table_row, toolbar, toolbar_spacer,
    },
    i18n::fill,
    profile::ProfileId,
    store::{
        Page, Store,
        node::{CoreStatus, DetailTab, FlowKind, NetworkFilter, Notice, Status},
    },
};

/// The page.
pub(super) fn page(store: Store) -> View {
    view()
        .gap_6()
        .child(toolbar_row(store))
        .child(dynamic(move || notice(store)))
        .child(dynamic(move || core_callout(store)))
        .child(summary(store))
        .child(table(store))
}

// ----- the toolbar -----

/// Search and the state filter on the left; import, join and create on the right, the primary one last.
fn toolbar_row(store: Store) -> View {
    let node = store.node();
    let words = store.strings().node.list;
    let filter = segmented_inline(
        store.tr(|s| s.node.list.filter_label),
        vec![
            SelectOption::new(NetworkFilter::All.key(), words.filter_all),
            SelectOption::new(NetworkFilter::Running.key(), words.filter_running),
            SelectOption::new(NetworkFilter::Stopped.key(), words.filter_stopped),
        ],
        move || SharedString::from(node.networks_filter().key()),
        move |picked| {
            if let Some(filter) = NetworkFilter::from_key(picked) {
                node.set_networks_filter(filter);
            }
        },
        "networks-filter",
    );
    toolbar()
        .child(
            search_field(
                store.tr(|s| s.node.list.search_placeholder),
                move || node.networks_query(),
                move |query| node.set_networks_query(query),
            )
            .clear_label(store.tr(|s| s.node.list.search_clear))
            .test_id("networks-search"),
        )
        .child(filter)
        .child(toolbar_spacer())
        .child(actions(store))
}

/// The four ways to get a network: import, enroll in a control plane, join, create.
fn actions(store: Store) -> View {
    let node = store.node();
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(tag(
            icon_label_button(
                ButtonKind::Ghost,
                Icon::Import,
                store.tr(|s| s.node.list.action_import),
                move || node.open_import(),
            ),
            "networks-import",
        ))
        .child(tag(
            icon_label_button(
                ButtonKind::Ghost,
                Icon::Globe,
                store.tr(|s| s.node.list.action_managed),
                move || node.open_managed(store.strings().node.list.managed_default_name),
            ),
            "networks-managed",
        ))
        .child(tag(
            icon_label_button(
                ButtonKind::Secondary,
                Icon::Link,
                store.tr(|s| s.node.list.action_join),
                move || node.open_flow(FlowKind::Join),
            ),
            "networks-join",
        ))
        .child(tag(
            icon_label_button(
                ButtonKind::Primary,
                Icon::Plus,
                store.tr(|s| s.node.list.action_create),
                move || node.open_flow(FlowKind::Create),
            ),
            "networks-create",
        ))
}

// ----- messages above the table -----

/// The message above the list, if the stored list or an action of it went wrong.
fn notice(store: Store) -> Option<View> {
    let node = store.node();
    let notice = node.notice()?;
    let strings = store.strings();
    let (kind, message) = match notice {
        Notice::Damaged => (
            CalloutKind::Danger,
            strings.node.list.notice_damaged.to_owned(),
        ),
        Notice::NewerSchema => (
            CalloutKind::Warning,
            strings.node.list.notice_newer.to_owned(),
        ),
        Notice::Skipped(count) => (
            CalloutKind::Warning,
            fill(
                strings.node.list.notice_skipped,
                &[("count", &count.to_string())],
            ),
        ),
        Notice::Failed(failure) => (
            CalloutKind::Danger,
            save_failure_text(strings.node.shared, &failure),
        ),
    };
    Some(
        callout(kind, message)
            .action(store.tr(|s| s.common.close), move || node.dismiss_notice())
            .test_id("networks-notice")
            .build(),
    )
}

/// Where the EasyTier core is missing or too old: say so, with the version found and the way to Settings.
fn core_callout(store: Store) -> Option<View> {
    let (message, version): (
        fn(&'static crate::i18n::Strings) -> &'static str,
        Option<String>,
    ) = match store.node().core_status() {
        CoreStatus::Missing => (|s| s.node.list.core_missing, None),
        CoreStatus::Unsupported(version) => {
            (|s| s.node.list.core_unsupported, Some(version.to_string()))
        }
        CoreStatus::Checking | CoreStatus::Ready(_) | CoreStatus::NotNeeded => return None,
    };
    Some(
        view()
            .gap_1()
            .child(
                callout(CalloutKind::Warning, move || message(store.strings()))
                    .action(store.tr(|s| s.node.list.core_action), move || {
                        store.open(Page::Settings);
                    })
                    .test_id("core-callout"),
            )
            .when_some(version, |column, version| {
                column.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_4()
                        .child(meta_text(store.tr(|s| s.node.list.core_detected)))
                        .child(machine_text(version).test_id("core-version")),
                )
            }),
    )
}

// ----- the summary strip -----

/// The core's (desktops) or the library's (phones) version, if one is known: the installed release, or what a
/// running network's node reports.
fn engine_version(store: Store) -> Option<String> {
    let node = store.node();
    match node.core_status() {
        CoreStatus::Ready(version) => Some(version.to_string()),
        CoreStatus::NotNeeded => node.ids().into_iter().find_map(|id| {
            node.snapshot(&id)
                .map(|snapshot| snapshot.node.version)
                .filter(|version| !version.is_empty())
        }),
        CoreStatus::Checking | CoreStatus::Missing | CoreStatus::Unsupported(_) => None,
    }
}

/// Which optional cells of the strip have data. The strip is made again when this changes, not at every poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shape {
    peers: bool,
    rates: bool,
    version: bool,
}

/// Networks running, peers online, aggregate rates and the core version; nothing while there are no networks.
fn summary(store: Store) -> View {
    let node = store.node();
    let shape = computed(move || Shape {
        peers: node.total_peer_mix().is_some(),
        rates: node.aggregate_rate().is_some(),
        version: engine_version(store).is_some(),
    });
    let any = computed(move || !node.ids().is_empty());
    view().child(dynamic(move || {
        if !any.get() {
            return None;
        }
        let shape = shape.get();
        Some(untracked(move || strip(store, shape)))
    }))
}

/// The strip itself, its values following the store.
fn strip(store: Store, shape: Shape) -> View {
    let node = store.node();
    let words = store.strings().node.list;
    let mut cells = vec![
        summary_cell(words.summary_running, move || {
            node.running_count().to_string()
        })
        .unit(move || {
            fill(
                store.strings().node.list.summary_running_of,
                &[("total", &node.ids().len().to_string())],
            )
        })
        .test_id("summary-running"),
    ];
    if shape.peers {
        let mix = move || node.total_peer_mix().unwrap_or_default();
        cells.push(
            summary_cell(words.summary_peers, move || mix().online.to_string())
                .bar(
                    Lamp::Good,
                    move || mix().direct.to_string(),
                    store.tr(|s| s.node.list.summary_direct),
                )
                .bar(
                    Lamp::Degraded,
                    move || mix().relayed.to_string(),
                    store.tr(|s| s.node.list.summary_relayed),
                )
                .test_id("summary-peers"),
        );
    }
    if shape.rates {
        let rate = move |pick: fn(&crate::store::node::RatePoint) -> f64| {
            move || {
                let point = node.aggregate_rate().unwrap_or_default();
                rate_parts(pick(&point))
            }
        };
        let rx = rate(|point| point.rx_bps);
        let tx = rate(|point| point.tx_bps);
        cells.push(
            summary_cell(words.summary_rx, move || rx().0)
                .unit(move || rx().1)
                .test_id("summary-rx"),
        );
        cells.push(
            summary_cell(words.summary_tx, move || tx().0)
                .unit(move || tx().1)
                .test_id("summary-tx"),
        );
    }
    if shape.version {
        cells.push(
            summary_cell(
                move || {
                    let words = store.strings().node.list;
                    if matches!(store.node().core_status(), CoreStatus::NotNeeded) {
                        words.summary_library
                    } else {
                        words.summary_core
                    }
                },
                move || engine_version(store).unwrap_or_default(),
            )
            .test_id("summary-version"),
        );
    }
    summary_strip(cells)
}

// ----- the table -----

/// The table of networks, with its empty states.
fn table(store: Store) -> View {
    let node = store.node();
    let cols = columns(vec![
        column(store.tr(|s| s.node.list.col_name)).flex(),
        column(store.tr(|s| s.node.list.col_status)).width(104.0),
        column(store.tr(|s| s.node.list.virtual_ip))
            .width(124.0)
            .mono()
            .right(),
        responsive(
            column(store.tr(|s| s.node.list.col_peers))
                .width(112.0)
                .right(),
            900.0,
        ),
        responsive(
            column(store.tr(|s| s.node.list.col_rx))
                .width(144.0)
                .right(),
            1360.0,
        ),
        responsive(
            column(store.tr(|s| s.node.list.col_tx))
                .width(144.0)
                .right(),
            1360.0,
        ),
        responsive(column(SharedString::default()).width(76.0), 1000.0),
        column(SharedString::default()).width(120.0).actions(),
    ]);
    let rows = {
        let cols = cols.clone();
        each(
            move || node.visible_ids(),
            |id: &ProfileId| id.clone(),
            move |id, _| network_row(store, &cols, id.get_untracked()).build(),
        )
    };
    view().child(
        data_table(&cols)
            .body(rows)
            .empty(
                move || node.visible_ids().is_empty(),
                move || empty(store).into_element(),
            )
            .test_id("networks-table"),
    )
}

/// Nothing to list: no networks yet (and how to make one), or none that the search and filter keep.
fn empty(store: Store) -> View {
    let node = store.node();
    view().child(
        when(
            move || node.ids().is_empty(),
            move || {
                empty_panel(
                    Icon::Network,
                    store.tr(|s| s.networks_empty_title),
                    store.tr(|s| s.networks_empty_body),
                    Some(
                        tag(
                            icon_label_button(
                                ButtonKind::Primary,
                                Icon::Plus,
                                store.tr(|s| s.node.list.action_create),
                                move || node.open_flow(FlowKind::Create),
                            ),
                            "networks-empty-create",
                        )
                        .into_element(),
                    ),
                )
            },
        )
        .otherwise(move || {
            empty_panel(
                Icon::Search,
                store.tr(|s| s.node.list.none_match_title),
                store.tr(|s| s.node.list.none_match_body),
                Some(
                    tag(
                        action_button(
                            ButtonKind::Secondary,
                            store.tr(|s| s.node.list.none_match_action),
                            move || node.clear_networks_narrowing(),
                        ),
                        "networks-clear-filter",
                    )
                    .into_element(),
                ),
            )
        }),
    )
}

/// One network's row.
fn network_row(store: Store, cols: &Columns, id: ProfileId) -> TableRow {
    let node = store.node();
    let status = {
        let id = id.clone();
        computed(move || node.status(&id))
    };
    let profile = {
        let id = id.clone();
        computed(move || node.profile(&id))
    };
    let name = move || profile.get().map(|p| p.name).unwrap_or_default();
    let ip = {
        let id = id.clone();
        move || {
            node.snapshot(&id)
                .map(|snapshot| address_text(snapshot.node.virtual_ipv4))
                .unwrap_or_default()
        }
    };
    let selected = {
        let id = id.clone();
        move || node.detail().as_ref() == Some(&id)
    };
    let open = {
        let id = id.clone();
        move || node.open_detail(id.clone(), DetailTab::Overview)
    };
    table_row(cols)
        .cell(name_cell(profile))
        .cell(status_lamp(
            move || lamp_of(&status.get()),
            move || status_word(store.strings(), &status.get()),
        ))
        .text(ip)
        .cell(peers_cell(store, id.clone()))
        .cell(rate_cell(store, id.clone(), Direction::Received))
        .cell(rate_cell(store, id.clone(), Direction::Sent))
        .cell(mode_chip(store, profile))
        .cell(row_actions(store, id.clone(), status, profile))
        .detail(problems(store, id.clone(), status))
        .selected(selected)
        .on_press(open)
        .label(move || {
            SharedString::from(fill(
                store.strings().node.list.card_label,
                &[("name", &name())],
            ))
        })
        .test_id(format!("network-{id}"))
}

/// The network's name over its EasyTier network name.
fn name_cell(profile: Memo<Option<crate::profile::NetworkProfile>>) -> View {
    let name = move || profile.get().map(|p| p.name).unwrap_or_default();
    let network_name = move || {
        profile
            .get()
            .filter(|p| !p.is_managed())
            .map(|p| p.network_name)
            .unwrap_or_default()
    };
    view()
        .min_w_0()
        .flex_shrink(1.0)
        .child(
            body_text(name)
                .font_weight(FontWeight::MEDIUM)
                .number_of_lines(1),
        )
        .child(meta_text(network_name).number_of_lines(1))
}

/// Peers online with how they are reached, as drawn bars and counts; a dash while the network does not run.
fn peers_cell(store: Store, id: ProfileId) -> View {
    let node = store.node();
    let mix = computed(move || node.peer_mix(&id));
    view()
        .flex_row()
        .items_center()
        .justify_end()
        .gap_2()
        .child(dynamic(move || {
            let Some(mix) = mix.get() else {
                return machine_text("\u{2014}")
                    .text_color(move || theme().muted_foreground)
                    .into_element();
            };
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(machine_text(mix.online.to_string()).number_of_lines(1))
                .child(bar_count("success", mix.direct))
                .child(bar_count("warning", mix.relayed))
                .into_element()
        }))
}

/// A 3 x 14 pt bar in the color token `hue` and a tabular count.
fn bar_count(hue: &'static str, count: usize) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            view()
                .w(Points(3.0))
                .h(Points(14.0))
                .flex_shrink_0()
                .rounded_full()
                .accessible(false)
                .bg(move || app_color(hue)),
        )
        .child(
            machine_text(count.to_string())
                .number_of_lines(1)
                .text_color(move || theme().muted_foreground),
        )
}

/// Which way traffic goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Received,
    Sent,
}

/// A rate with its trend, from the samples the history kept; a dash while the network does not run.
fn rate_cell(store: Store, id: ProfileId, direction: Direction) -> View {
    let node = store.node();
    let latest = {
        let id = id.clone();
        computed(move || node.latest_rate(&id))
    };
    let samples = {
        let id = id.clone();
        move || match direction {
            Direction::Received => node.rx_rates(&id),
            Direction::Sent => node.tx_rates(&id),
        }
    };
    let color = match direction {
        Direction::Received => SeriesColor::Received,
        Direction::Sent => SeriesColor::Sent,
    };
    let running = computed(move || node.status(&id) == Status::Running);
    view()
        .flex_row()
        .items_center()
        .justify_end()
        .gap_2()
        .child(when(
            move || running.get(),
            move || sparkline(samples.clone(), color),
        ))
        .child(
            machine_text(move || {
                latest
                    .get()
                    .map(|point| match direction {
                        Direction::Received => point.rx_bps,
                        Direction::Sent => point.tx_bps,
                    })
                    .map(|rate| ChartUnit::BytesPerSecond.format(rate))
                    .unwrap_or_else(|| "\u{2014}".to_owned())
            })
            .number_of_lines(1)
            .min_w(Points(64.0))
            .text_right()
            .text_color(move || theme().foreground),
        )
}

/// How the network runs: managed by a control plane, or with / without a virtual interface.
fn mode_chip(store: Store, profile: Memo<Option<crate::profile::NetworkProfile>>) -> View {
    view().flex_row().child(neutral_chip(move || {
        let words = store.strings().node.list;
        match profile.get() {
            Some(profile) if profile.is_managed() => words.chip_managed,
            Some(profile) if !profile.tun => words.chip_no_tun,
            Some(_) => words.chip_tun,
            None => "",
        }
    }))
}

/// Start or Stop as the state allows, and the more menu.
fn row_actions(
    store: Store,
    id: ProfileId,
    status: Memo<Status>,
    profile: Memo<Option<crate::profile::NetworkProfile>>,
) -> View {
    let node = store.node();
    let busy = {
        let id = id.clone();
        computed(move || node.is_busy(&id))
    };
    let primary = {
        let id = id.clone();
        dynamic(move || {
            let id = id.clone();
            // Busy is read here, not bound inside the button: the status and busy flag change in one update, and a
            // binding inside a button this render is about to replace would run after its owner is gone.
            let busy_now = busy.get();
            match status.get() {
                Status::Stopped | Status::Failed(_) => tag(
                    busy_button(
                        ButtonKind::Secondary,
                        store.tr(|s| s.node.list.action_start),
                        move || busy_now,
                        move || node.start(id.clone()),
                    ),
                    "network-start",
                ),
                Status::Starting | Status::Running | Status::Stopping | Status::Removing => tag(
                    busy_button(
                        ButtonKind::Ghost,
                        store.tr(|s| s.node.list.action_stop),
                        move || busy_now,
                        move || node.stop(id.clone()),
                    ),
                    "network-stop",
                ),
            }
        })
    };
    let managed = untracked(|| profile.get()).is_some_and(|p| p.is_managed());
    let name = move || profile.get().map(|p| p.name).unwrap_or_default();
    let open = move |id: &ProfileId, tab: DetailTab| {
        let id = id.clone();
        move || node.open_detail(id.clone(), tab)
    };
    let mut items = vec![
        menu_action(
            Icon::Settings,
            store.tr(|s| s.node.list.action_settings),
            open(&id, DetailTab::Settings),
        )
        .test_id("network-menu-settings"),
        menu_action(
            Icon::Terminal,
            store.tr(|s| s.node.list.action_logs),
            open(&id, DetailTab::Logs),
        )
        .test_id("network-menu-logs"),
    ];
    if !managed {
        items.push(
            menu_action(
                Icon::Download,
                store.tr(|s| s.node.list.action_export),
                open(&id, DetailTab::Export),
            )
            .test_id("network-menu-export"),
        );
    }
    let delete_label: fn(&'static crate::i18n::Strings) -> &'static str = if managed {
        |s| s.node.list.leave_action
    } else {
        |s| s.common.delete
    };
    items.push(
        menu_action(
            Icon::Trash,
            move || delete_label(store.strings()),
            move || ask_delete(store, id.clone()),
        )
        .destructive()
        .separated()
        .test_id("network-menu-delete"),
    );
    view()
        .flex_row()
        .items_center()
        .justify_end()
        .gap_1()
        .child(primary)
        .child(
            more_menu(
                move || {
                    SharedString::from(fill(
                        store.strings().node.list.action_more,
                        &[("name", &name())],
                    ))
                },
                items,
            )
            .test_id("network-more"),
        )
}

/// Under the row, outside its press target: why the network is not running or did not stop, and the way out.
fn problems(store: Store, id: ProfileId, status: Memo<Status>) -> AnyElement {
    let node = store.node();
    dynamic(move || {
        let mut column = view().gap_2().px_4().pb_3();
        let mut any = false;
        let failure_id = id.clone();
        match status.get() {
            Status::Failed(found) => {
                column = column.child(failure::panel(store, failure_id, found));
                any = true;
            }
            Status::Starting | Status::Running => {
                if let Some(found) = node.start_failure(&id) {
                    column = column.child(failure::panel(store, failure_id, found));
                    any = true;
                }
            }
            Status::Stopped | Status::Stopping | Status::Removing => {}
        }
        if let Some(reason) = node.stop_error(&id) {
            column = column.child(failure::stop_error(store, reason));
            any = true;
        }
        any.then_some(column)
    })
    .into_element()
}
