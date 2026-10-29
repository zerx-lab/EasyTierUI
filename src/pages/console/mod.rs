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
mod format;
mod gate;
mod machine;
mod machines;
mod networks;

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
