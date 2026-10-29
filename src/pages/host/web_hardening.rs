//! Hardening in the web build: what the serving easytier-web's REST API can verify or change, and honesty about
//! what it cannot. Changing the signed-in account's password is the one action; registration, the `/api/` proxy
//! switch, auto-created accounts and the seeded `user` password are fixed by how the server was deployed, so those
//! rows are guidance with a way to the Deployment page and claim no state. Nothing here signs in as the seeded
//! account: that would replace the browser's session cookie.

use nsh_core::prelude::*;

use super::parts::panel;
use crate::{
    components::{
        ButtonKind, CalloutKind, Lamp, action_button, callout, column, columns, data_table,
        meta_text, status_lamp, table_row,
    },
    core::target::TargetId,
    i18n::WebHostStrings,
    pages::console,
    store::{Page, Store},
};

/// A binding of the web host string `pick` chooses, following the language as it changes.
fn tr(
    store: Store,
    pick: fn(&'static WebHostStrings) -> &'static str,
) -> impl Fn() -> &'static str + Copy {
    move || pick(store.strings().web_host)
}

/// The hardening page, behind the sign-in gate.
pub fn page(store: Store) -> AnyElement {
    console::gated(store, move |id| ready(store, id))
}

/// The page of a signed-in server: the intro and the checklist.
fn ready(store: Store, id: TargetId) -> AnyElement {
    view()
        .gap_6()
        .child(callout(CalloutKind::Info, tr(store, |s| s.harden_intro)))
        .child(panel(tr(store, |s| s.section_checks), None).child(checklist(store, id)))
        .into_element()
}

/// What explains a check, under its row.
fn explanation(store: Store, detail: fn(&'static WebHostStrings) -> &'static str) -> View {
    view().px_4().pb_3().child(meta_text(tr(store, detail)))
}

/// A button in the actions column, found by `id`.
fn row_action(
    label: impl IntoReactive<SharedString>,
    id: &str,
    on_press: impl Fn() + 'static,
) -> View {
    view()
        .test_id(id)
        .child(action_button(ButtonKind::Secondary, label, on_press))
}

/// A row for something only the deployment controls: no state is claimed, and the action opens Deployment.
fn guidance_row(
    store: Store,
    cols: &crate::components::Columns,
    title: fn(&'static WebHostStrings) -> &'static str,
    detail: fn(&'static WebHostStrings) -> &'static str,
    key: &str,
) -> crate::components::TableRow {
    table_row(cols)
        .text(tr(store, title))
        .cell(status_lamp(
            || Lamp::Idle,
            tr(store, |s| s.state_deployment),
        ))
        .cell(row_action(
            tr(store, |s| s.action_deployment),
            &format!("web-open-deployment-{key}"),
            move || store.open(Page::Deployment),
        ))
        .detail(explanation(store, detail))
        .test_id(format!("web-check-{key}"))
}

/// The checklist: the password the API can change, then the four rows the deployment decides.
fn checklist(store: Store, id: TargetId) -> View {
    let console = store.console();
    let cols = columns(vec![
        column(tr(store, |s| s.col_check)).flex(),
        column(tr(store, |s| s.col_status)).width(200.0),
        column("").width(160.0).actions(),
    ]);
    data_table(&cols)
        .row(
            table_row(&cols)
                .text(tr(store, |s| s.check_password))
                .cell(status_lamp(|| Lamp::Idle, tr(store, |s| s.state_password)))
                .cell(row_action(
                    tr(store, |s| s.action_password),
                    "web-password-action",
                    move || console.open_change_password(id),
                ))
                .detail(explanation(store, |s| s.detail_password))
                .test_id("web-check-password"),
        )
        .row(guidance_row(
            store,
            &cols,
            |s| s.check_registration,
            |s| s.detail_registration,
            "registration",
        ))
        .row(guidance_row(
            store,
            &cols,
            |s| s.check_proxy,
            |s| s.detail_proxy,
            "proxy",
        ))
        .row(guidance_row(
            store,
            &cols,
            |s| s.check_auto_create,
            |s| s.detail_auto_create,
            "auto-create",
        ))
        .row(guidance_row(
            store,
            &cols,
            |s| s.check_seeded,
            |s| s.detail_seeded,
            "seeded",
        ))
        .test_id("web-hardening-checks")
        .build()
}
