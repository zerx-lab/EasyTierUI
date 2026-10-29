//! The Logs tab: the log level of the machine's core. The core has one level for all its networks, and a change is
//! not saved: the core is back on its own setting after a restart.

use nsh_core::prelude::*;

use super::{failure_banner, loaded, section, tr};
use crate::{
    components::{CalloutKind, Icon, SelectOption, callout, icon_button, meta_text, select},
    core::remote::LogLevel,
    store::{Store, console::Scope},
};

/// The tab: reads the level and offers the six levels the core knows. `scope` is the machine's.
pub(super) fn tab(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    console.begin_runtime_view();
    console.refresh_log_level(scope);
    let level = loaded(
        store,
        move || console.log_level(scope),
        move || console.refresh_log_level(scope),
        move || level_choice(store, scope),
    );
    view()
        .gap_6()
        .child(
            callout(CalloutKind::Info, tr(store, |c| c.logs_note))
                .test_id("logs-note")
                .build(),
        )
        .child(failure_banner(store, scope))
        .child(section(
            store,
            |c| c.logs_title,
            Some(|c| c.logs_hint),
            "logs-section",
            vec![level],
        ))
        .into_element()
}

/// The word for a level.
fn level_name(store: Store, level: LogLevel) -> &'static str {
    let c = store.strings().console_runtime;
    match level {
        LogLevel::Disabled => c.level_disabled,
        LogLevel::Error => c.level_error,
        LogLevel::Warning => c.level_warning,
        LogLevel::Info => c.level_info,
        LogLevel::Debug => c.level_debug,
        LogLevel::Trace => c.level_trace,
    }
}

/// The select over the levels, the refresh button and what is in flight.
fn level_choice(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let options = LogLevel::ALL
        .into_iter()
        .map(|level| SelectOption::new(level.code().to_string(), level_name(store, level)))
        .collect::<Vec<_>>();
    let picker = select(
        tr(store, |c| c.field_log_level),
        options,
        move || {
            SharedString::from(
                console
                    .log_level(scope)
                    .value
                    .map(|level| level.code().to_string())
                    .unwrap_or_default(),
            )
        },
        move |id| {
            let level = id.parse::<i64>().ok().and_then(LogLevel::from_code);
            if let Some(level) = level {
                console.set_log_level(scope, level);
            }
        },
    )
    .test_id("log-level");
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .items_end()
                .gap_2()
                .child(view().flex_1().child(picker))
                .child(
                    icon_button(Icon::Refresh, store.tr(|s| s.common.refresh), move || {
                        console.refresh_log_level(scope);
                    })
                    .test_id("log-level-refresh"),
                ),
        )
        .child(meta_text(tr(store, |c| c.log_level_help)))
        .child(dynamic(move || {
            console.is_applying(scope).then(|| {
                meta_text(tr(store, |c| c.applying))
                    .test_id("log-level-applying")
                    .into_element()
            })
        }))
        .into_element()
}
