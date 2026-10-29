//! Why a network is not running, in words, with what the person can do about it (DESIGN.md: every state designed).

use nsh_core::prelude::*;

use super::{common::tag, format::tail_chars};
use crate::{
    components::{ButtonKind, CalloutKind, action_button, callout, machine_text, meta_text},
    core::state::FailReason,
    profile::ProfileId,
    store::{
        Page, Store,
        node::{DetailTab, Failure},
    },
};

/// The longest tail of the core's output shown in a card.
const LAST_OUTPUT_CHARS: usize = 400;

/// The failure `failure` of network `id`: a danger callout saying what happened, the machine data it concerns
/// beside it, and the actions that fix it (open the app's Settings, edit the network, run without TUN, restart,
/// try again, show the logs).
pub(super) fn panel(store: Store, id: ProfileId, failure: Failure) -> View {
    let node = store.node();
    let message = move |pick: fn(&'static crate::i18n::node::SharedStrings) -> &'static str| {
        move || pick(store.strings().node.shared)
    };
    let mut details: Vec<AnyElement> = Vec::new();
    let mut actions: Vec<AnyElement> = Vec::new();
    let retry = |store: Store, id: ProfileId| {
        let label = store.tr(|s| s.node.shared.failure_action_retry);
        tag(
            action_button(ButtonKind::Primary, label, move || {
                store.node().start(id.clone());
            }),
            "failure-retry",
        )
        .into_element()
    };
    let restart = |store: Store, id: ProfileId| {
        let label = store.tr(|s| s.node.shared.failure_action_restart);
        tag(
            action_button(ButtonKind::Primary, label, move || {
                store.node().restart(id.clone());
            }),
            "failure-restart",
        )
        .into_element()
    };
    let secondary = |store: Store,
                     id: ProfileId,
                     tab: DetailTab,
                     label: fn(&'static crate::i18n::node::SharedStrings) -> &'static str,
                     test_id: &'static str| {
        tag(
            action_button(
                ButtonKind::Secondary,
                move || label(store.strings().node.shared),
                move || store.node().open_detail(id.clone(), tab),
            ),
            test_id,
        )
        .into_element()
    };
    let pick = match failure {
        Failure::SecretMissing => {
            actions.push(secondary(
                store,
                id.clone(),
                DetailTab::Settings,
                |w| w.failure_action_secret,
                "failure-secret",
            ));
            message(|w| w.failure_secret_missing)
        }
        Failure::Core(FailReason::NoPrivileges) => {
            let can_drop_tun = node
                .profile(&id)
                .is_some_and(|profile| profile.tun && !profile.is_managed());
            if can_drop_tun {
                let for_press = id.clone();
                actions.push(
                    tag(
                        action_button(
                            ButtonKind::Secondary,
                            store.tr(|s| s.node.shared.failure_action_no_tun),
                            move || store.node().start_without_tun(for_press.clone()),
                        ),
                        "failure-no-tun",
                    )
                    .into_element(),
                );
            }
            actions.push(retry(store, id.clone()));
            message(|w| w.failure_no_privileges)
        }
        Failure::Core(FailReason::CoreMissing) => {
            actions.push(settings_action(store));
            message(|w| w.failure_core_missing)
        }
        Failure::Core(FailReason::CoreUnsupported(version)) => {
            details.push(machine_text(version.to_string()).into_element());
            actions.push(settings_action(store));
            message(|w| w.failure_core_unsupported)
        }
        Failure::Core(FailReason::ConfigRejected(reason)) => {
            details.push(machine_text(tail_chars(&reason, LAST_OUTPUT_CHARS)).into_element());
            actions.push(secondary(
                store,
                id.clone(),
                DetailTab::Settings,
                |w| w.failure_action_edit,
                "failure-edit",
            ));
            message(|w| w.failure_config_rejected)
        }
        Failure::Core(FailReason::Exited { code, last_log }) => {
            if let Some(code) = code {
                details.push(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(meta_text(store.tr(|s| s.node.shared.failure_exit_code)))
                        .child(machine_text(code.to_string()))
                        .into_element(),
                );
            }
            if !last_log.trim().is_empty() {
                details.push(
                    view()
                        .gap_1()
                        .child(meta_text(store.tr(|s| s.node.shared.failure_last_output)))
                        .child(
                            machine_text(tail_chars(&last_log, LAST_OUTPUT_CHARS)).selectable(true),
                        )
                        .into_element(),
                );
            }
            actions.push(restart(store, id.clone()));
            actions.push(secondary(
                store,
                id.clone(),
                DetailTab::Logs,
                |w| w.failure_action_logs,
                "failure-logs",
            ));
            message(|w| w.failure_exited)
        }
        Failure::Core(FailReason::Unreachable(reason)) => {
            if !reason.trim().is_empty() {
                details.push(machine_text(tail_chars(&reason, LAST_OUTPUT_CHARS)).into_element());
            }
            actions.push(restart(store, id.clone()));
            actions.push(secondary(
                store,
                id.clone(),
                DetailTab::Logs,
                |w| w.failure_action_logs,
                "failure-logs",
            ));
            message(|w| w.failure_unreachable)
        }
        Failure::Core(FailReason::ElevationDenied) => {
            actions.push(retry(store, id.clone()));
            message(|w| w.failure_elevation_denied)
        }
        Failure::Core(FailReason::Other(reason)) => {
            if !reason.trim().is_empty() {
                details.push(machine_text(tail_chars(&reason, LAST_OUTPUT_CHARS)).into_element());
            }
            actions.push(retry(store, id.clone()));
            actions.push(secondary(
                store,
                id.clone(),
                DetailTab::Logs,
                |w| w.failure_action_logs,
                "failure-logs",
            ));
            message(|w| w.failure_other)
        }
    };
    view()
        .gap_2()
        .child(callout(CalloutKind::Danger, pick).test_id("network-failure"))
        .child(details)
        .when(!actions.is_empty(), |column| {
            column.child(view().flex_row().flex_wrap().gap_2().child(actions))
        })
}

/// A secondary action that opens the app's Settings page (EasyTier versions).
fn settings_action(store: Store) -> AnyElement {
    tag(
        action_button(
            ButtonKind::Secondary,
            store.tr(|s| s.node.shared.failure_action_settings),
            move || store.open(Page::Settings),
        ),
        "failure-settings",
    )
    .into_element()
}

/// Why the last stop failed: a danger callout with the core's message in monospace beside it.
pub(super) fn stop_error(store: Store, reason: FailReason) -> View {
    let text = match reason {
        FailReason::ConfigRejected(text)
        | FailReason::Unreachable(text)
        | FailReason::Other(text) => text,
        FailReason::Exited { last_log, .. } => last_log,
        FailReason::CoreUnsupported(version) => version.to_string(),
        FailReason::NoPrivileges | FailReason::CoreMissing | FailReason::ElevationDenied => {
            String::new()
        }
    };
    view()
        .gap_1()
        .child(
            callout(
                CalloutKind::Danger,
                store.tr(|s| s.node.shared.failure_stop),
            )
            .test_id("network-stop-error"),
        )
        .when(!text.trim().is_empty(), |column| {
            column.child(machine_text(tail_chars(&text, LAST_OUTPUT_CHARS)))
        })
}
