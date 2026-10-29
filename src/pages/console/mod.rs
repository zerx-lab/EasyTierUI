//! Console: an easytier-web server's machines, networks and credentials.
//!
//! Every page shows through [`gate::gate`]: with no server selected it offers to add one, with a server that is not
//! signed in it shows the sign-in panel (and why: never signed in, session ended, unreachable), and only a signed-in
//! server shows the page itself: a toolbar with the server's identity, a table, and drawers for an entity's detail
//! and editing. The same pages serve the servers the user added, the hosted control plane and, in the web build, the
//! server that served the page.

mod account;
mod credentials;
mod dialogs;
mod editor;
pub(crate) mod embed;
mod format;
mod gate;
mod inspect;
mod machine;
mod machines;
mod networks;
mod runtime;

use nsh_core::prelude::*;

use crate::{
    i18n::{ConsoleStrings, fill},
    store::{Page, Store},
};

/// The actions this role's `page` puts in the shell's page header: importing a config on the Networks page,
/// generating a credential on the Credentials page. The Machines page's actions are in its drawer.
pub fn header_actions(store: Store, page: Page) -> Option<AnyElement> {
    match page {
        Page::ServerNetworks => Some(networks::header_actions(store)),
        Page::Credentials => Some(credentials::header_actions(store)),
        _ => None,
    }
}

/// Picks one string of the console's area.
type Pick = fn(&'static ConsoleStrings) -> &'static str;

/// A binding of the console string `pick` chooses, following the language as it changes.
fn tr(store: Store, pick: Pick) -> impl Fn() -> &'static str + Copy + 'static {
    move || pick(store.strings().console)
}

/// The console template `pick` chooses, filled with `args`, in the current language.
fn filled(store: Store, pick: Pick, args: &[(&str, &str)]) -> String {
    fill(pick(store.strings().console), args)
}

/// A server's machines.
pub fn machines(store: Store) -> AnyElement {
    gate::gate(store, move |id| machines::ready(store, id))
}

/// A server's networks.
pub fn networks(store: Store) -> AnyElement {
    gate::gate(store, move |id| networks::ready(store, id))
}

/// A server's credentials.
pub fn credentials(store: Store) -> AnyElement {
    gate::gate(store, move |id| credentials::ready(store, id))
}

/// `ready(id)` while the selected server is signed in, else the sign-in gate: for the web build's control-plane
/// pages, which are views of the server that serves the page.
pub(super) fn gated(
    store: Store,
    ready: impl Fn(crate::core::target::TargetId) -> AnyElement + 'static,
) -> AnyElement {
    gate::gate(store, ready)
}

/// The toolbar of a signed-in server's page (identity, freshness, refresh, account menu).
pub(super) fn server_toolbar(
    store: Store,
    id: crate::core::target::TargetId,
    live: bool,
    refresh: impl Fn() + 'static,
    read_at: impl Fn() -> Option<f64> + 'static,
) -> View {
    account::server_toolbar(store, id, Vec::new(), live, refresh, read_at)
}

/// A failed read of the server with its reason and a retry.
pub(super) fn load_failed(
    store: Store,
    error: &crate::core::remote::RemoteError,
    retry: impl Fn() + 'static,
) -> AnyElement {
    networks::load_failed(store, error, retry)
}

/// The console's overlays: the add-server and password dialogs and the show-once credential dialog, and the
/// drawers (a machine, the network editor, the credential form).
pub fn overlays(store: Store) -> View {
    view()
        .child(dialogs::add_server(store))
        .child(dialogs::change_password(store))
        .child(machine::drawer_view(store))
        .child(editor::drawer_view(store))
        .child(credentials::generate_drawer(store))
        .child(dialogs::issued(store))
}
