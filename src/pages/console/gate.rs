//! What every console page shows before there is a signed-in server to act on: no server selected, the sign-in form
//! (never signed in, session ended, unreachable, signing in), or the page itself.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    account::{self, confirm_remove},
    filled, tr,
};
use crate::{
    components::{
        ButtonKind, CalloutKind, CopyLabels, Icon, Lamp, SecretLabels, action_button, body_text,
        callout, empty_panel, field, machine_text, meta_text, secret_field, status_lamp,
    },
    core::target::{TargetId, TargetOrigin},
    store::{
        Page, Store,
        console::{Session, SignInError},
    },
};

/// The width of the centered panels in points.
const PANEL_WIDTH: f32 = 440.0;

/// Why the sign-in form is shown.
#[derive(Debug, Clone, PartialEq)]
enum Why {
    /// Never signed in to this server in this run.
    Fresh,
    /// The app is restoring or signing in (the form is busy).
    Connecting,
    /// The server ended the session.
    Expired,
    /// The password was just changed, which ends the session.
    PasswordChanged,
    /// The server did not answer.
    Unreachable(String),
}

/// What the page shows.
#[derive(Debug, Clone, PartialEq)]
enum Phase {
    /// The selected target is not a server.
    NoServer,
    /// A server that is not signed in.
    SignIn(TargetId, Why),
    /// A signed-in server.
    Ready(TargetId),
}

/// `ready(id)` while the selected server is signed in; otherwise what is missing, with a way forward.
pub(super) fn gate(store: Store, ready: impl Fn(TargetId) -> AnyElement + 'static) -> AnyElement {
    let phase = computed(move || {
        let Some(target) = store.current_target() else {
            return Phase::NoServer;
        };
        if !target.is_server() {
            return Phase::NoServer;
        }
        let id = target.id;
        let console = store.console();
        match console.session(id) {
            Session::SignedIn { .. } => Phase::Ready(id),
            Session::NotSignedIn if console.password_just_changed(id) => {
                Phase::SignIn(id, Why::PasswordChanged)
            }
            Session::NotSignedIn => Phase::SignIn(id, Why::Fresh),
            Session::SigningIn => Phase::SignIn(id, Why::Connecting),
            Session::Expired => Phase::SignIn(id, Why::Expired),
            Session::Unreachable { message } => Phase::SignIn(id, Why::Unreachable(message)),
        }
    });
    dynamic(move || match phase.get() {
        Phase::NoServer => untracked(|| no_server(store).into_element()),
        Phase::SignIn(id, why) => untracked(|| sign_in(store, id, why).into_element()),
        Phase::Ready(id) => untracked(|| ready(id)),
    })
}

/// The id of the selected target while it is a signed-in server (tracked): what the page header's actions need.
pub(super) fn signed_in(store: Store) -> Option<TargetId> {
    let target = store.current_target()?;
    if !target.is_server() {
        return None;
    }
    matches!(store.console().session(target.id), Session::SignedIn { .. }).then_some(target.id)
}

/// The frame of a centered panel: the container fill, a hairline and the container radius, flat at rest.
fn frame() -> View {
    view()
        .w(Points(PANEL_WIDTH))
        .max_w(relative(1.0))
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
}

/// `panel` centered in the page, with room above and below.
fn centered(panel: View) -> View {
    view().items_center().py_8().child(panel)
}

/// The selected target is this device (or nothing): the console needs a server.
fn no_server(store: Store) -> View {
    let hosting = store.capabilities().host.is_local();
    let actions = view()
        .flex_row()
        .flex_wrap()
        .justify_center()
        .gap_2()
        .child(action_button(
            ButtonKind::Primary,
            store.tr(|s| s.target_add_server),
            move || store.console().request_add_server(),
        ))
        .child(hosting.then(|| {
            action_button(
                ButtonKind::Secondary,
                store.tr(|s| s.target_host_here),
                move || store.open(Page::HostStatus),
            )
        }));
    centered(frame().child(empty_panel(
        Icon::Server,
        store.tr(|s| s.console_no_server_title),
        store.tr(|s| s.console_no_server_body),
        Some(actions.into_element()),
    )))
}

/// The sign-in panel of server `id`, with what happened to the last session.
fn sign_in(store: Store, id: TargetId, why: Why) -> View {
    let console = store.console();
    let fields = console.sign_in_fields();
    if matches!(why, Why::Connecting) {
        // Restoring or signing in: the person waits; the form comes back if the server refuses.
        return centered(
            frame()
                .p_6()
                .gap_4()
                .child(account::identity(store, id))
                .child(meta_text(tr(store, |c| c.signing_in))),
        );
    }
    let removable = move || {
        store
            .target(id)
            .is_some_and(|target| matches!(target.origin, TargetOrigin::Url(_)))
    };
    let notice = match &why {
        Why::Fresh | Why::Connecting => None,
        Why::Expired => Some(callout(
            CalloutKind::Warning,
            tr(store, |c| c.sign_in_expired),
        )),
        Why::PasswordChanged => Some(callout(
            CalloutKind::Info,
            tr(store, |c| c.sign_in_password_changed),
        )),
        Why::Unreachable(_) => Some(
            callout(CalloutKind::Danger, tr(store, |c| c.unreachable))
                .action(store.tr(|s| s.common.retry), move || console.connect(id)),
        ),
    };
    let detail = match &why {
        Why::Unreachable(message) => Some(machine_text(message.clone())),
        _ => None,
    };
    let labels = SecretLabels::new(
        store.tr(|s| s.common.show),
        store.tr(|s| s.common.hide),
        CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied)),
    );
    let form = view()
        .gap_4()
        .child(
            field(
                tr(store, |c| c.username),
                move || fields.username.get(),
                move |text| fields.username.set(text.to_owned()),
            )
            .test_id("sign-in-username"),
        )
        .child(
            secret_field(
                tr(store, |c| c.password),
                move || fields.password.get(),
                move |text| fields.password.set(text.to_owned()),
                labels,
            )
            .error(move || {
                fields
                    .error
                    .get()
                    .map(|error| SharedString::from(sign_in_error_text(store, &error)))
            })
            .test_id("sign-in-password"),
        )
        .child(meta_text(tr(store, |c| c.sign_in_digest_note)))
        .child((!console.keeps_session()).then(|| meta_text(tr(store, |c| c.session_not_kept))))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(action_button(
                    ButtonKind::Primary,
                    tr(store, |c| c.sign_in),
                    move || console.sign_in(id),
                ))
                .child(removable().then(|| {
                    action_button(
                        ButtonKind::Ghost,
                        tr(store, |c| c.remove_server),
                        move || confirm_remove(store, id),
                    )
                })),
        );
    centered(
        frame()
            .p_6()
            .gap_4()
            .child(account::identity(store, id))
            .child(
                view()
                    .gap_1()
                    .child(
                        body_text(tr(store, |c| c.sign_in_title)).font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(meta_text(store.tr(|s| s.console_not_connected_body))),
            )
            .child(notice)
            .child(detail)
            .child(form),
    )
}

/// The words for a failed sign-in.
fn sign_in_error_text(store: Store, error: &SignInError) -> String {
    match error {
        SignInError::Missing => tr(store, |c| c.sign_in_missing)().to_owned(),
        SignInError::Remote(error) => super::format::remote_error_text(store.strings(), error),
    }
}

/// The lamp of a server's session, with its word.
pub(super) fn session_lamp(store: Store, id: TargetId) -> View {
    let lamp = move || match store.console().session(id) {
        Session::SignedIn { .. } => Lamp::Good,
        Session::SigningIn => Lamp::Busy,
        Session::Unreachable { .. } => Lamp::Failed,
        Session::Expired => Lamp::Degraded,
        Session::NotSignedIn => Lamp::Idle,
    };
    let word = move || {
        let strings = store.strings();
        match store.console().session(id) {
            Session::SignedIn { .. } => strings.common.state_online,
            Session::SigningIn => strings.common.state_connecting,
            Session::Unreachable { .. } => strings.console.state_unreachable,
            Session::Expired => strings.console.state_expired,
            Session::NotSignedIn => strings.status_not_connected,
        }
    };
    status_lamp(lamp, word)
}

/// A template of the console's strings filled with `args`, for a binding.
pub(super) fn template(
    store: Store,
    pick: fn(&'static crate::i18n::ConsoleStrings) -> &'static str,
    args: Vec<(&'static str, String)>,
) -> impl Fn() -> SharedString + 'static {
    move || {
        let args: Vec<(&str, &str)> = args
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        SharedString::from(filled(store, pick, &args))
    }
}
