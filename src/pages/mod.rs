//! One module per page, the same pages on every platform. A page asks the store what this platform and target can
//! do; where neither this device nor a server's API can do it, it shows a Capability Notice with the alternative.

mod config;
mod console;
mod host;
mod node;
mod settings;
mod welcome;

use nsh_core::prelude::*;
pub use welcome::welcome;

use crate::{
    components::capability_notice,
    store::{Page, Store},
};

/// The content of `page` (its header is the shell's).
pub fn content(store: Store, page: Page) -> AnyElement {
    match page {
        Page::Networks => node::networks(store).into_element(),
        Page::Peers => node::peers(store).into_element(),
        Page::Machines => console::machines(store).into_element(),
        Page::ServerNetworks => console::networks(store).into_element(),
        Page::Credentials => console::credentials(store).into_element(),
        Page::HostStatus => host::status(store).into_element(),
        Page::Hardening => host::hardening(store).into_element(),
        Page::Deployment => host::deployment(store).into_element(),
        Page::Settings => settings::settings(store).into_element(),
    }
}

/// The actions `page` puts at the trailing edge of the shell's page header (its primary action, secondary actions
/// before it), or `None` when it has none. The shell renders them on wide layouts and in the compact header.
pub fn header_actions(store: Store, page: Page) -> Option<AnyElement> {
    match page {
        Page::Networks | Page::Peers => node::header_actions(store, page),
        Page::Machines | Page::ServerNetworks | Page::Credentials => {
            console::header_actions(store, page)
        }
        Page::HostStatus | Page::Hardening | Page::Deployment => host::header_actions(store, page),
        Page::Settings => settings::header_actions(store, page),
    }
}

/// Dialogs any page or the shell can open (adding a server from the target switcher, confirmations): mounted once
/// beside the layout, shown while their role's store asks for them.
pub fn overlays(store: Store) -> View {
    view()
        .child(node::overlays(store))
        .child(console::overlays(store))
        .child(host::overlays(store))
}

/// The notice on a phone or tablet, which can neither host a control plane nor reach one's process through an API:
/// it leads to the deployment files.
fn mobile_host_notice(store: Store) -> View {
    capability_notice(
        store.tr(|s| s.notice_mobile_host),
        store.tr(|s| s.notice_mobile_host_action),
        move || store.open(Page::Deployment),
    )
}
