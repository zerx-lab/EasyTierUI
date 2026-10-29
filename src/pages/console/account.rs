//! The server a console page is about: its identity, the toolbar every signed-in page starts with (identity, search,
//! freshness, refresh, account menu), and the banner of an action that failed.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    filled,
    format::{is_removable, remote_error_text, server_address, server_name},
    gate::session_lamp,
    tr,
};
use crate::{
    components::{
        CalloutKind, ICON_SIZE, Icon, Lamp, age_text, callout, confirm, control_radius, icon,
        icon_button, kind_chip, live_dot, machine_text, menu_action, meta_text, more_menu,
        status_lamp, toolbar, toolbar_spacer,
    },
    core::target::{TargetId, TargetKind},
    i18n::fill,
    store::{
        Store,
        console::{ActionKind, Session},
    },
};

/// The server's name now (in the current language).
fn name_of(store: Store, id: TargetId) -> String {
    store
        .target(id)
        .map(|target| server_name(&target, store.strings()))
        .unwrap_or_default()
}

/// The server's name as a line that is cut with an ellipsis rather than wrapped.
fn name_line(store: Store, id: TargetId) -> Text {
    text(move || SharedString::from(name_of(store, id)))
        .font_weight(FontWeight::SEMIBOLD)
        .number_of_lines(1)
        .min_w_0()
        .text_color(move || theme().foreground)
}

/// The server's address as a muted machine line, when it has one.
fn address_line(store: Store, id: TargetId) -> Option<Text> {
    let address = store
        .target(id)
        .and_then(|target| server_address(&target))?;
    Some(
        machine_text(SharedString::from(address))
            .text_xs()
            .number_of_lines(1)
            .min_w_0()
            .text_color(move || theme().muted_foreground),
    )
}

/// A server's identity for the sign-in panel: its icon, name, address and kind, and the lamp of its session.
pub(super) fn identity(store: Store, id: TargetId) -> View {
    let hosted = store
        .target(id)
        .is_some_and(|target| target.kind == TargetKind::Hosted);
    let tile = view()
        .size_10()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .rounded(control_radius)
        .bg(move || theme().muted)
        .child(icon(Icon::Server, ICON_SIZE));
    let kind = if hosted {
        kind_chip(store.tr(|s| s.kind_hosted))
    } else {
        kind_chip(store.tr(|s| s.kind_server))
    };
    view()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(tile)
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(name_line(store, id).text_base())
                        .child(address_line(store, id)),
                )
                .child(kind),
        )
        .child(session_lamp(store, id))
}

/// A signed-in server in the toolbar: icon, name over address, the lamp and the account.
fn toolbar_identity(store: Store, id: TargetId) -> View {
    let account = move || match store.console().session(id) {
        Session::SignedIn { account } => account,
        _ => String::new(),
    };
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .min_w_0()
        .child(icon(Icon::Server, ICON_SIZE))
        .child(
            view()
                .min_w_0()
                .child(name_line(store, id).text_sm())
                .child(address_line(store, id)),
        )
        .child(status_lamp(
            || Lamp::Good,
            store.tr(|s| s.common.state_online),
        ))
        .child(
            machine_text(move || SharedString::from(account()))
                .text_xs()
                .number_of_lines(1)
                .text_color(move || theme().muted_foreground),
        )
}

/// The account's menu: change the password, sign out, remove the server. A hosted control plane has no account
/// here, and only a server the user added can be removed; with nothing to offer there is no menu.
fn account_menu(store: Store, id: TargetId) -> Option<AnyElement> {
    let target = store.target(id);
    let hosted = target
        .as_ref()
        .is_some_and(|target| target.kind == TargetKind::Hosted);
    let removable = target.as_ref().is_some_and(is_removable);
    let console = store.console();
    let mut actions = Vec::new();
    if !hosted {
        actions.push(
            menu_action(Icon::Lock, tr(store, |c| c.change_password), move || {
                console.open_change_password(id);
            })
            .test_id("account-password"),
        );
        actions.push(
            menu_action(Icon::External, tr(store, |c| c.sign_out), move || {
                confirm_sign_out(store, id);
            })
            .test_id("account-sign-out"),
        );
    }
    if removable {
        actions.push(
            menu_action(Icon::Trash, tr(store, |c| c.remove_server), move || {
                confirm_remove(store, id);
            })
            .destructive()
            .separated()
            .test_id("account-remove"),
        );
    }
    if actions.is_empty() {
        return None;
    }
    Some(
        more_menu(tr(store, |c| c.account_menu), actions)
            .test_id("account-menu")
            .into_element(),
    )
}

/// The toolbar of a signed-in page: the server's identity, then `context` (search, pickers), and at the trailing
/// edge how fresh the data is (`live` pulses the dot while the page keeps polling), refresh and the account menu.
pub(super) fn server_toolbar(
    store: Store,
    id: TargetId,
    context: Vec<AnyElement>,
    live: bool,
    refresh: impl Fn() + 'static,
    read_at: impl Fn() -> Option<f64> + 'static,
) -> View {
    toolbar()
        .child(toolbar_identity(store, id))
        .child(context)
        .child(toolbar_spacer())
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(live_dot(move || live))
                .child(age_text(
                    read_at,
                    tr(store, |c| c.read_age),
                    tr(store, |c| c.read_now),
                )),
        )
        .child(
            icon_button(Icon::Refresh, store.tr(|s| s.common.refresh), refresh)
                .test_id("console-refresh"),
        )
        .child(account_menu(store, id))
}

/// Asks, naming the server, then signs out of it.
fn confirm_sign_out(store: Store, id: TargetId) {
    let strings = store.strings();
    let name = name_of(store, id);
    let asked = confirm(
        fill(strings.console.sign_out_title, &[("server", name.as_str())]),
        strings.console.sign_out_body,
        strings.console.sign_out,
        strings.common.cancel,
        false,
    );
    store
        .console()
        .when_confirmed(asked, move || store.console().sign_out(id));
}

/// Asks, naming the server, then removes it.
pub(super) fn confirm_remove(store: Store, id: TargetId) {
    let strings = store.strings();
    let name = name_of(store, id);
    let asked = confirm(
        fill(strings.console.remove_title, &[("server", name.as_str())]),
        strings.console.remove_body,
        strings.console.remove_server,
        strings.common.cancel,
        true,
    );
    store
        .console()
        .when_confirmed(asked, move || store.console().remove_server(id));
}

/// The banner of an action on `id` that failed (switching a network, deleting it, revoking a credential), with the
/// server's own words under it.
pub(super) fn failure_banner(store: Store, id: TargetId) -> AnyElement {
    dynamic(move || {
        let failure = store
            .console()
            .failure()
            .filter(|failure| failure.target == id)?;
        let lead = filled(
            store,
            match failure.kind {
                ActionKind::Enable => |c| c.failed_enable,
                ActionKind::Disable => |c| c.failed_disable,
                ActionKind::Delete => |c| c.failed_delete,
                ActionKind::Revoke => |c| c.failed_revoke,
            },
            &[("name", failure.subject.as_str())],
        );
        let detail = remote_error_text(store.strings(), &failure.error);
        Some(
            view()
                .gap_1()
                .child(
                    callout(CalloutKind::Danger, SharedString::from(lead)).action(
                        store.tr(|s| s.common.close),
                        move || {
                            store.console().dismiss_failure();
                        },
                    ),
                )
                .child(meta_text(SharedString::from(detail)))
                .into_element(),
        )
    })
}
