//! The control plane's status in the web build: what the serving easytier-web's REST API reports. Behind the
//! console's sign-in gate, it shows whether the API answers and when it was read, the account's device count, the
//! core versions the machines run, and every machine session.

use std::{collections::BTreeSet, time::Duration};

use nsh_core::prelude::*;
use uuid::Uuid;

use super::parts::panel;
use crate::{
    components::{
        ButtonKind, CalloutKind, Icon, action_button, callout, column, columns, data_table,
        empty_panel, meta_text, summary_cell, summary_strip, table_row,
    },
    core::{remote::RemoteError, target::TargetId},
    i18n::WebHostStrings,
    pages::console,
    store::Store,
};

/// How often the figures are read again while the page shows.
const POLL: Duration = Duration::from_secs(10);

/// What the page shows.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    /// Nothing was read yet.
    Loading,
    /// The first read failed.
    Failed(RemoteError),
    /// There is something to show.
    Ready,
}

/// One machine session of the table.
#[derive(Debug, Clone, PartialEq)]
struct SessionRow {
    machine: Uuid,
    hostname: String,
    account: i64,
    location: String,
}

/// A binding of the web host string `pick` chooses, following the language as it changes.
fn tr(
    store: Store,
    pick: fn(&'static WebHostStrings) -> &'static str,
) -> impl Fn() -> &'static str + Copy {
    move || pick(store.strings().web_host)
}

/// The status page, behind the sign-in gate.
pub fn page(store: Store) -> AnyElement {
    console::gated(store, move |id| ready(store, id))
}

/// The page of a signed-in server.
fn ready(store: Store, id: TargetId) -> AnyElement {
    let console = store.console();
    console.refresh_overview(id);
    let _poll = set_interval(POLL, move || console.refresh_overview(id));
    let kind = computed(move || {
        let overview = console.overview(id);
        if overview.summary.value.is_some() || overview.sessions.value.is_some() {
            return Kind::Ready;
        }
        match overview.summary.error.or(overview.sessions.error) {
            Some(error) => Kind::Failed(error),
            None => Kind::Loading,
        }
    });
    view()
        .gap_4()
        .child(console::server_toolbar(
            store,
            id,
            true,
            move || console.refresh_overview(id),
            move || console.overview(id).summary.read_at_ms,
        ))
        .child(dynamic(move || match kind.get() {
            Kind::Loading => meta_text(tr(store, |s| s.loading)).into_element(),
            Kind::Failed(error) => untracked(|| {
                console::load_failed(store, &error, move || console.refresh_overview(id))
            }),
            Kind::Ready => untracked(|| body(store, id)),
        }))
        .into_element()
}

/// The stale-read warning, the summary strip and the sessions.
fn body(store: Store, id: TargetId) -> AnyElement {
    let console = store.console();
    let stale = computed(move || {
        let overview = console.overview(id);
        let failed = overview.summary.error.or(overview.sessions.error);
        failed.filter(|_| overview.summary.value.is_some() || overview.sessions.value.is_some())
    });
    view()
        .gap_4()
        .child(dynamic(move || {
            stale.get().map(|_| {
                callout(CalloutKind::Warning, tr(store, |s| s.refresh_failed))
                    .action(store.tr(|s| s.common.retry), move || {
                        console.refresh_overview(id)
                    })
                    .into_element()
            })
        }))
        .child(summary(store, id))
        .child(sessions(store, id))
        .into_element()
}

/// The distinct core versions the machines report, sorted.
fn versions_of(store: Store, id: TargetId) -> Vec<String> {
    let found: BTreeSet<String> = store
        .console()
        .machines(id)
        .value
        .unwrap_or_default()
        .into_iter()
        .map(|machine| machine.version)
        .filter(|version| !version.is_empty())
        .collect();
    found.into_iter().collect()
}

/// The strip: the API, the account's devices, every session, the core versions in use. A figure the server has not
/// reported is left out.
fn summary(store: Store, id: TargetId) -> AnyElement {
    let console = store.console();
    let shape = computed(move || {
        let overview = console.overview(id);
        (
            overview.summary.value.is_some(),
            overview.sessions.value.is_some(),
            !versions_of(store, id).is_empty(),
        )
    });
    dynamic(move || {
        let (has_summary, has_sessions, has_versions) = shape.get();
        let mut cells = vec![
            summary_cell(tr(store, |s| s.sum_api), move || {
                let overview = console.overview(id);
                let reachable =
                    overview.summary.error.is_none() && overview.summary.value.is_some();
                let words = store.strings().web_host;
                String::from(if reachable {
                    words.api_reachable
                } else {
                    words.api_unreachable
                })
            })
            .caption(tr(store, |s| s.api_caption))
            .test_id("web-summary-api"),
        ];
        if has_summary {
            cells.push(
                summary_cell(tr(store, |s| s.sum_devices), move || {
                    console
                        .overview(id)
                        .summary
                        .value
                        .map(|summary| summary.device_count.to_string())
                        .unwrap_or_default()
                })
                .caption(tr(store, |s| s.devices_caption))
                .test_id("web-summary-devices"),
            );
        }
        if has_sessions {
            cells.push(
                summary_cell(tr(store, |s| s.sum_sessions), move || {
                    console
                        .overview(id)
                        .sessions
                        .value
                        .map(|sessions| sessions.len().to_string())
                        .unwrap_or_default()
                })
                .caption(tr(store, |s| s.sessions_caption))
                .test_id("web-summary-sessions"),
            );
        }
        if has_versions {
            cells.push(
                summary_cell(tr(store, |s| s.sum_versions), move || {
                    versions_of(store, id).len().to_string()
                })
                .caption(move || versions_of(store, id).join(", "))
                .test_id("web-summary-versions"),
            );
        }
        summary_strip(cells).into_element()
    })
    .into_element()
}

/// The sessions with each machine's hostname and location from the machine list.
fn rows_of(store: Store, id: TargetId) -> Vec<SessionRow> {
    let console = store.console();
    let machines = console.machines(id).value.unwrap_or_default();
    let mut rows: Vec<SessionRow> = console
        .overview(id)
        .sessions
        .value
        .unwrap_or_default()
        .into_iter()
        .map(|session| {
            let machine = machines.iter().find(|m| m.id == session.machine_id);
            SessionRow {
                machine: session.machine_id,
                hostname: machine.map(|m| m.hostname.clone()).unwrap_or_default(),
                account: session.user_id,
                location: machine.and_then(|m| m.location.clone()).unwrap_or_default(),
            }
        })
        .collect();
    rows.sort_by_key(|row| row.machine);
    rows
}

/// The sessions table.
fn sessions(store: Store, id: TargetId) -> AnyElement {
    let rows = computed(move || rows_of(store, id));
    let cols = columns(vec![
        column(tr(store, |s| s.col_machine)).flex().mono(),
        column(tr(store, |s| s.col_host)).width(200.0),
        column(tr(store, |s| s.col_account))
            .width(96.0)
            .mono()
            .right(),
        column(tr(store, |s| s.col_location))
            .width(160.0)
            .hide_below(900.0),
    ]);
    let row_columns = cols.clone();
    panel(tr(store, |s| s.section_sessions), None)
        .child(
            data_table(&cols)
                .body(each(
                    move || rows.get(),
                    |row: &SessionRow| row.machine,
                    move |row, _| {
                        let machine = row.get_untracked().machine;
                        table_row(&row_columns)
                            .text(move || row.with(|row| row.machine.to_string()))
                            .text(move || row.with(|row| row.hostname.clone()))
                            .text(move || row.with(|row| row.account.to_string()))
                            .text(move || row.with(|row| row.location.clone()))
                            .label(move || row.with(|row| row.machine.to_string()))
                            .test_id(format!("session-{machine}"))
                            .build()
                    },
                ))
                .empty(
                    move || rows.with(|rows| rows.is_empty()),
                    move || {
                        empty_panel(
                            Icon::Server,
                            tr(store, |s| s.sessions_empty_title),
                            tr(store, |s| s.sessions_empty_body),
                            Some(
                                action_button(
                                    ButtonKind::Secondary,
                                    store.tr(|s| s.common.refresh),
                                    move || store.console().refresh_overview(id),
                                )
                                .into_element(),
                            ),
                        )
                        .into_element()
                    },
                )
                .test_id("web-sessions-table"),
        )
        .into_element()
}
