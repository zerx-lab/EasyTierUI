//! Settings → Desktop: keeping to the tray when the window closes, starting at login, starting quietly. Phones and
//! the web have no tray, window or login item of their own and say so.

use nsh_core::prelude::*;

use crate::{
    components::{CalloutKind, callout, card, meta_text, switch_row},
    i18n::fill,
    store::{Store, desktop::LoginStatus},
};

/// The section's body.
pub(super) fn desktop(store: Store) -> AnyElement {
    let desktop = store.desktop();
    if !desktop.available() {
        return callout(CalloutKind::Info, store.tr(|s| s.desktop.unsupported))
            .test_id("desktop-unsupported")
            .into_element();
    }
    let no_tray_host = move || desktop.tray_host() == Some(false);
    let login_busy = move || {
        matches!(
            desktop.login_status(),
            LoginStatus::Reading | LoginStatus::Changing
        )
    };
    let rows = vec![
        switch_row(
            store.tr(|s| s.desktop.keep_in_tray),
            move || desktop.keep_in_tray(),
            move |on| desktop.set_keep_in_tray(on),
        )
        .description(store.tr(|s| s.desktop.keep_in_tray_body))
        .disabled(no_tray_host)
        .test_id("desktop-tray"),
        switch_row(
            store.tr(|s| s.desktop.login),
            move || desktop.login(),
            move |on| desktop.set_login(on),
        )
        .description(store.tr(|s| s.desktop.login_body))
        .disabled(login_busy)
        .test_id("desktop-login"),
        switch_row(
            store.tr(|s| s.desktop.quiet_start),
            move || desktop.quiet_start(),
            move |on| desktop.set_quiet_start(on),
        )
        .description(store.tr(|s| s.desktop.quiet_start_body))
        .disabled(move || !desktop.keep_in_tray())
        .test_id("desktop-quiet"),
    ];
    view()
        .gap_3()
        .child(card().child(rows))
        .child(when(no_tray_host, move || {
            meta_text(store.tr(|s| s.desktop.no_tray_host)).test_id("desktop-no-tray")
        }))
        .child(when(
            move || !no_tray_host() && !desktop.keep_in_tray(),
            move || {
                meta_text(store.tr(|s| s.desktop.quiet_start_needs_tray))
                    .test_id("desktop-quiet-needs-tray")
            },
        ))
        .child(dynamic(move || match desktop.login_status() {
            LoginStatus::Failed(reason) => Some(
                callout(
                    CalloutKind::Danger,
                    SharedString::from(fill(
                        store.strings().desktop.login_failed,
                        &[("reason", &reason)],
                    )),
                )
                .test_id("desktop-login-failed")
                .into_element(),
            ),
            LoginStatus::Reading | LoginStatus::Ready | LoginStatus::Changing => None,
        }))
        .into_element()
}
