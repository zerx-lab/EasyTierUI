//! Machines: the nodes enrolled in a server, as a summary and a table whose rows open the machine's drawer.

use std::{collections::BTreeMap, time::Duration};

use nsh_core::{prelude::*, time::unix_ms};

use super::{
    account::{failure_banner, server_toolbar},
    format::format_ago,
    networks::load_failed,
    tr,
};
use crate::{
    components::{
        ButtonKind, CalloutKind, Columns, Icon, Lamp, action_button, callout, column, columns,
        data_table, empty_panel, meta_text, search_field, status_chip, summary_cell, summary_strip,
        table_row,
    },
    core::{
        remote::{Machine, RemoteError},
        target::TargetId,
    },
    store::Store,
};

/// How often the online lamps are judged again against the clock.
const CLOCK_TICK: Duration = Duration::from_secs(5);

/// How often the machine list is read again while the page shows.
const POLL: Duration = Duration::from_secs(10);

/// What the page shows.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Loading,
    Failed(RemoteError),
    Table,
}

/// The figures of the summary strip.
#[derive(Debug, Clone, PartialEq)]
struct Stats {
    total: usize,
    online: usize,
    networks: usize,
    versions: usize,
    common_version: String,
}

impl Stats {
    /// Machines whose last report is too old to count as online.
    fn offline(&self) -> usize {
        self.total.saturating_sub(self.online)
    }
}

/// The page of a signed-in server.
pub(super) fn ready(store: Store, id: TargetId) -> AnyElement {
    let console = store.console();
    let now = clock();
    // Heartbeats age on the server: the list is read again so that "online" is judged on fresh reports.
    let _poll = set_interval(POLL, move || console.refresh_machines(id));
    let search = search_field(
        tr(store, |c| c.search_machines),
        move || console.machine_query(),
        move |query| console.set_machine_query(query),
    )
    .clear_label(tr(store, |c| c.search_clear))
    .test_id("machine-search");
    let kind = computed(move || {
        let feed = console.machines(id);
        match feed.value {
            None => match feed.error {
                Some(error) => Kind::Failed(error),
                None => Kind::Loading,
            },
            Some(_) => Kind::Table,
        }
    });
    view()
        .gap_4()
        .child(server_toolbar(
            store,
            id,
            vec![search.into_element()],
            true,
            move || console.refresh_machines(id),
            move || console.machines(id).read_at_ms,
        ))
        .child(failure_banner(store, id))
        .child(dynamic(move || match kind.get() {
            Kind::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
            Kind::Failed(error) => {
                untracked(|| load_failed(store, &error, move || console.refresh_machines(id)))
            }
            Kind::Table => untracked(|| table(store, id, now)),
        }))
        .into_element()
}

/// A signal of the clock that ticks while the page shows, for judging "online".
pub(super) fn clock() -> Signal<f64> {
    let now = signal(unix_ms());
    let _tick = set_interval(CLOCK_TICK, move || {
        now.try_set(unix_ms());
    });
    now
}

/// Whether `machine` matches the lowercase search `query`: its name, system, version or location holds it.
fn matches(machine: &Machine, query: &str) -> bool {
    query.is_empty()
        || [
            machine.hostname.as_str(),
            machine.os.as_str(),
            machine.version.as_str(),
            machine.location.as_deref().unwrap_or_default(),
        ]
        .iter()
        .any(|field| field.to_lowercase().contains(query))
}

/// The machines the search keeps, by hostname.
fn visible_of(machines: &[Machine], query: &str) -> Vec<Machine> {
    let query = query.trim().to_lowercase();
    let mut kept: Vec<Machine> = machines
        .iter()
        .filter(|machine| matches(machine, &query))
        .cloned()
        .collect();
    kept.sort_by(|a, b| a.hostname.cmp(&b.hostname));
    kept
}

/// The figures of `machines` judged at `now_ms`.
fn stats_of(machines: &[Machine], now_ms: f64) -> Stats {
    let mut versions: BTreeMap<&str, usize> = BTreeMap::new();
    for machine in machines {
        let count = versions.entry(machine.version.as_str()).or_insert(0);
        *count = count.saturating_add(1);
    }
    let common_version = versions
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(version, _)| (*version).to_owned())
        .unwrap_or_default();
    Stats {
        total: machines.len(),
        online: machines
            .iter()
            .filter(|machine| machine.is_online(now_ms))
            .count(),
        networks: machines.iter().fold(0_usize, |sum, machine| {
            sum.saturating_add(machine.running_networks.len())
        }),
        versions: versions.len(),
        common_version,
    }
}

/// The summary strip and the table of the machines.
fn table(store: Store, id: TargetId, now: Signal<f64>) -> AnyElement {
    let console = store.console();
    let machines = computed(move || console.machines(id).value.unwrap_or_default());
    let visible = computed(move || visible_of(&machines.get(), &console.machine_query()));
    let stale = computed(move || {
        let feed = console.machines(id);
        feed.value.and(feed.error)
    });
    let cols = columns(vec![
        column(tr(store, |c| c.col_machine)).flex(),
        column(tr(store, |c| c.col_os)).width(120.0),
        column(tr(store, |c| c.col_version)).width(144.0).mono(),
        column(tr(store, |c| c.col_networks))
            .width(88.0)
            .mono()
            .right(),
        column(tr(store, |c| c.col_location))
            .width(128.0)
            .hide_below(1040.0),
        column(tr(store, |c| c.col_reported))
            .width(112.0)
            .mono()
            .right()
            .hide_below(900.0),
        column(tr(store, |c| c.col_status)).width(104.0).actions(),
    ]);
    let rows_columns = cols.clone();
    view()
        .gap_4()
        .child(dynamic(move || {
            stale.get().map(|_| {
                callout(CalloutKind::Warning, tr(store, |c| c.refresh_failed))
                    .action(store.tr(|s| s.common.retry), move || {
                        console.refresh_machines(id)
                    })
                    .into_element()
            })
        }))
        .child(dynamic(move || {
            (!machines.with(|machines| machines.is_empty()))
                .then(|| untracked(|| summary(store, machines, now).into_element()))
        }))
        .child(
            data_table(&cols)
                .body(each(
                    move || visible.get(),
                    |machine: &Machine| machine.id,
                    move |machine, _| machine_row(store, id, &rows_columns, machine, now),
                ))
                .empty(
                    move || visible.with(|visible| visible.is_empty()),
                    move || {
                        if machines.with(|machines| machines.is_empty()) {
                            empty_panel(
                                Icon::Server,
                                tr(store, |c| c.machines_empty_title),
                                tr(store, |c| c.machines_empty_body),
                                None,
                            )
                            .into_element()
                        } else {
                            empty_panel(
                                Icon::Search,
                                tr(store, |c| c.no_match_title),
                                tr(store, |c| c.no_match_body),
                                Some(
                                    action_button(
                                        ButtonKind::Secondary,
                                        tr(store, |c| c.search_clear),
                                        move || console.set_machine_query(""),
                                    )
                                    .into_element(),
                                ),
                            )
                            .into_element()
                        }
                    },
                )
                .test_id("machines-table"),
        )
        .into_element()
}

/// The strip above the table: machines online and offline, networks running, core versions in use.
fn summary(store: Store, machines: Memo<Vec<Machine>>, now: Signal<f64>) -> View {
    let stats = computed(move || stats_of(&machines.get(), now.get()));
    summary_strip(vec![
        summary_cell(tr(store, |c| c.sum_machines), move || {
            stats.with(|stats| stats.total.to_string())
        })
        .bar(
            Lamp::Good,
            move || stats.with(|stats| stats.online.to_string()),
            tr(store, |c| c.machine_online),
        )
        .bar(
            Lamp::Idle,
            move || stats.with(|stats| stats.offline().to_string()),
            tr(store, |c| c.machine_offline),
        )
        .test_id("summary-machines"),
        summary_cell(tr(store, |c| c.sum_networks_running), move || {
            stats.with(|stats| stats.networks.to_string())
        })
        .test_id("summary-networks"),
        summary_cell(tr(store, |c| c.sum_versions), move || {
            stats.with(|stats| stats.versions.to_string())
        })
        .caption(move || stats.with(|stats| stats.common_version.clone()))
        .test_id("summary-versions"),
    ])
}

/// The online lamp and its word; `online()` is judged by the caller against the clock.
pub(super) fn online_chip(store: Store, online: impl Fn() -> bool + Copy + 'static) -> View {
    let lamp = move || if online() { Lamp::Good } else { Lamp::Idle };
    let word = move || {
        let strings = store.strings().console;
        if online() {
            strings.machine_online
        } else {
            strings.machine_offline
        }
    };
    status_chip(lamp, word)
}

/// One machine of the table; pressing it opens the machine's drawer.
fn machine_row(
    store: Store,
    id: TargetId,
    cols: &Columns,
    machine: ReadSignal<Machine>,
    now: Signal<f64>,
) -> View {
    let console = store.console();
    let mid = machine.get_untracked().id;
    table_row(cols)
        .text(move || machine.with(|machine| machine.hostname.clone()))
        .text(move || machine.with(|machine| machine.os.clone()))
        .text(move || machine.with(|machine| machine.version.clone()))
        .text(move || machine.with(|machine| machine.running_networks.len().to_string()))
        .text(move || machine.with(|machine| machine.location.clone().unwrap_or_default()))
        .text(move || {
            machine
                .with(|machine| machine.report_time_ms)
                .map(|reported| format_ago(store.strings(), now.get(), reported))
                .unwrap_or_default()
        })
        .cell(online_chip(store, move || {
            machine.with(|machine| machine.is_online(now.get()))
        }))
        .selected(move || console.selected_machine(id) == Some(mid))
        .on_press(move || console.select_machine(id, Some(mid)))
        .label(move || machine.with(|machine| machine.hostname.clone()))
        .test_id(format!("machine-{mid}"))
        .build()
}
