//! The console's dialogs: adding a server, changing the password and showing a new credential once. These are
//! short, protected-focus flows; everything an entity owns (machines, networks, credentials) opens in a drawer.

use nsh_core::prelude::*;

use super::{format::remote_error_text, tr};
use crate::{
    components::{
        ButtonKind, CalloutKind, CopyLabels, HintLevel, SecretLabels, action_button, busy_button,
        callout, copy_value, dialog_footer, dialog_frame, field, meta_text, secret_field,
    },
    core::secret::{Secret, Strength},
    i18n::fill,
    store::{
        Store,
        console::{AddError, PasswordError},
    },
};

/// The words for a server that was not added.
fn add_error_text(store: Store, error: &AddError) -> String {
    let c = store.strings().console;
    match error {
        AddError::Address(crate::core::remote::RemoteError::InsecureTransport) => {
            c.add_error_insecure.to_owned()
        }
        AddError::Address(_) => c.add_error_invalid.to_owned(),
        AddError::Duplicate(name) => fill(c.add_error_duplicate, &[("name", name.as_str())]),
        AddError::NoRoom => c.add_error_full.to_owned(),
    }
}

/// The add-server dialog.
pub(super) fn add_server(store: Store) -> impl IntoElement {
    let console = store.console();
    let fields = console.add_fields();
    dialog_frame(
        move || fields.open.get(),
        tr(store, |c| c.add_server_title),
        move || console.close_add_server(),
    )
    .test_id("add-server")
    .body(move || {
        view()
            .gap_4()
            .child(
                field(
                    tr(store, |c| c.add_server_url),
                    move || fields.url.get(),
                    move |text| {
                        fields.url.set(text.to_owned());
                        fields.error.set(None);
                    },
                )
                .placeholder("https://et.example.com")
                .helper(tr(store, |c| c.add_server_url_help))
                .error(move || {
                    fields
                        .error
                        .get()
                        .map(|error| SharedString::from(add_error_text(store, &error)))
                })
                .mono()
                .test_id("add-server-url"),
            )
            .child(
                field(
                    tr(store, |c| c.add_server_name),
                    move || fields.name.get(),
                    move |text| fields.name.set(text.to_owned()),
                )
                .helper(tr(store, |c| c.add_server_name_help))
                .test_id("add-server-name"),
            )
    })
    .footer(move || {
        dialog_footer()
            .child(action_button(
                ButtonKind::Ghost,
                store.tr(|s| s.common.cancel),
                move || console.close_add_server(),
            ))
            .child(action_button(
                ButtonKind::Primary,
                tr(store, |c| c.add_server_submit),
                move || console.add_server(),
            ))
    })
}

/// The words for a password that was not changed.
fn password_error_text(store: Store, error: &PasswordError) -> String {
    let c = store.strings().console;
    match error {
        PasswordError::Empty => c.password_empty.to_owned(),
        PasswordError::Mismatch => c.password_mismatch.to_owned(),
        PasswordError::Remote(error) => remote_error_text(store.strings(), error),
    }
}

/// The change-password dialog.
pub(super) fn change_password(store: Store) -> impl IntoElement {
    let console = store.console();
    let fields = console.password_fields();
    let labels = move || {
        SecretLabels::new(
            store.tr(|s| s.common.show),
            store.tr(|s| s.common.hide),
            CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied)),
        )
    };
    dialog_frame(
        move || fields.open.get().is_some(),
        tr(store, |c| c.change_password),
        move || console.close_change_password(),
    )
    .test_id("change-password")
    .body(move || {
        view()
            .gap_4()
            .child(callout(CalloutKind::Info, tr(store, |c| c.password_note)))
            .child(
                secret_field(
                    tr(store, |c| c.password_new),
                    move || fields.new_password.get(),
                    move |text| {
                        fields.new_password.set(text.to_owned());
                        fields.error.set(None);
                    },
                    labels(),
                )
                .hint(
                    move || {
                        let value = fields.new_password.get();
                        if value.is_empty() {
                            return SharedString::default();
                        }
                        let common = store.strings().common;
                        SharedString::from(match Secret::new(value).strength() {
                            Strength::Weak => common.strength_weak,
                            Strength::Fair => common.strength_fair,
                            Strength::Strong => common.strength_strong,
                        })
                    },
                    move || match Secret::new(fields.new_password.get()).strength() {
                        Strength::Weak => HintLevel::Weak,
                        Strength::Fair => HintLevel::Fair,
                        Strength::Strong => HintLevel::Strong,
                    },
                )
                .test_id("password-new"),
            )
            .child(
                secret_field(
                    tr(store, |c| c.password_confirm),
                    move || fields.confirm.get(),
                    move |text| {
                        fields.confirm.set(text.to_owned());
                        fields.error.set(None);
                    },
                    labels(),
                )
                .error(move || {
                    fields
                        .error
                        .get()
                        .map(|error| SharedString::from(password_error_text(store, &error)))
                })
                .test_id("password-confirm"),
            )
    })
    .footer(move || {
        dialog_footer()
            .child(action_button(
                ButtonKind::Ghost,
                store.tr(|s| s.common.cancel),
                move || console.close_change_password(),
            ))
            .child(busy_button(
                ButtonKind::Primary,
                store.tr(|s| s.common.save),
                move || fields.busy.get(),
                move || console.change_password(),
            ))
    })
}

/// The credential that was just created: its secret is shown once.
pub(super) fn issued(store: Store) -> impl IntoElement {
    let console = store.console();
    let labels =
        move || CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied));
    dialog_frame(
        move || console.issued().is_some(),
        tr(store, |c| c.issued_title),
        move || console.dismiss_issued(),
    )
    .test_id("credential-issued")
    .body(move || {
        view()
            .gap_4()
            .child(callout(
                CalloutKind::Warning,
                tr(store, |c| c.issued_warning),
            ))
            .child(
                view()
                    .gap_1()
                    .child(meta_text(tr(store, |c| c.issued_id)))
                    .child(copy_value(
                        "issued-id",
                        move || console.issued().map(|issued| issued.id).unwrap_or_default(),
                        labels(),
                    )),
            )
            .child(
                view()
                    .gap_1()
                    .child(meta_text(tr(store, |c| c.issued_secret)))
                    .child(copy_value(
                        "issued-secret",
                        move || {
                            console
                                .issued()
                                .map(|issued| issued.secret.expose().to_owned())
                                .unwrap_or_default()
                        },
                        labels(),
                    )),
            )
    })
    .footer(move || {
        dialog_footer().child(action_button(
            ButtonKind::Primary,
            tr(store, |c| c.issued_done),
            move || console.dismiss_issued(),
        ))
    })
}
