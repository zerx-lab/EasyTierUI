//! The "join a control plane" dialog.

use nsh_core::prelude::*;

use super::common::{error_of, save_failure_view, secret_labels, secrets_notice, tag};
use crate::{
    components::{
        ButtonKind, CalloutKind, action_button, busy_button, callout, dialog_footer, dialog_frame,
        field, machine_text, meta_text, secret_field,
    },
    core::secret::Secret,
    store::{Store, node::Field as FieldKind},
};

/// The dialog, on screen while `store.node().managed()` holds a draft.
pub(super) fn dialog(store: Store) -> If {
    let node = store.node();
    dialog_frame(
        move || node.managed().with(Option::is_some),
        store.tr(|s| s.node.import.managed_title),
        move || node.close_managed(),
    )
    .body(move || body(store))
    .footer(move || {
        dialog_footer()
            .child(tag(
                action_button(
                    ButtonKind::Ghost,
                    store.tr(|s| s.common.cancel),
                    move || node.close_managed(),
                ),
                "managed-cancel",
            ))
            .child(tag(
                busy_button(
                    ButtonKind::Primary,
                    store.tr(|s| s.node.import.managed_enroll),
                    move || {
                        node.managed()
                            .with(|draft| draft.as_ref().is_some_and(|d| d.saving))
                    },
                    move || node.managed_submit(),
                ),
                "managed-enroll",
            ))
    })
    .test_id("managed-dialog")
    .build()
}

/// The error line of `field`, once the form shows errors.
fn problem_of(store: Store, field: FieldKind) -> Option<SharedString> {
    store.node().managed().with(|draft| {
        draft
            .as_ref()
            .and_then(|d| error_of(store.strings(), &d.visible_problems(), field))
    })
}

/// The form.
fn body(store: Store) -> View {
    let node = store.node();
    view()
        .gap_4()
        .child(
            callout(
                CalloutKind::Info,
                store.tr(|s| s.node.import.managed_explain),
            )
            .test_id("managed-explain"),
        )
        .child(
            field(
                store.tr(|s| s.node.shared.field_name),
                move || {
                    node.managed()
                        .with(|draft| draft.as_ref().map(|d| d.name.clone()))
                        .unwrap_or_default()
                },
                move |value| {
                    node.edit_managed(|d| {
                        d.name = value.to_owned();
                        d.failure = None;
                    });
                },
            )
            .error(move || problem_of(store, FieldKind::Name))
            .test_id("managed-name"),
        )
        .child(
            secret_field(
                store.tr(|s| s.node.import.managed_server_label),
                move || {
                    node.managed()
                        .with(|draft| draft.as_ref().map(|d| d.config_server.expose().to_owned()))
                        .unwrap_or_default()
                },
                move |value| {
                    node.edit_managed(|d| {
                        d.config_server = Secret::new(value);
                        d.failure = None;
                    });
                },
                secret_labels(store),
            )
            .error(move || problem_of(store, FieldKind::ConfigServer))
            .test_id("managed-server"),
        )
        .child(
            view()
                .gap_1()
                .child(meta_text(store.tr(|s| s.node.import.managed_server_help)))
                .child(
                    machine_text(move || store.strings().node.import.managed_server_placeholder)
                        .test_id("managed-server-shape"),
                ),
        )
        .child(
            field(
                store.tr(|s| s.node.shared.field_hostname),
                move || {
                    node.managed()
                        .with(|draft| draft.as_ref().map(|d| d.hostname.clone()))
                        .unwrap_or_default()
                },
                move |value| {
                    node.edit_managed(|d| {
                        d.hostname = value.to_owned();
                        d.failure = None;
                    });
                },
            )
            .helper(store.tr(|s| s.node.import.managed_hostname_help))
            .error(move || problem_of(store, FieldKind::Hostname))
            .test_id("managed-hostname"),
        )
        .child(secrets_notice(store))
        .child(save_failure_view(store, move || {
            node.managed()
                .with(|draft| draft.as_ref().and_then(|d| d.failure.clone()))
        }))
}
