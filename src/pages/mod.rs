//! One module per page, the same pages on every platform. A page asks the store what this platform and target can
//! do; a feature that cannot run here shows a Capability Notice with the remote alternative.

mod console;
mod host;
mod node;
mod settings;

use nsh_core::prelude::*;

use crate::{
    components::capability_notice,
    core::platform::Limitation,
    i18n::Pick,
    store::{Page, Store},
};

/// The content of `page` (its header is the shell's).
pub fn content(store: Store, page: Page) -> AnyElement {
    match page {
        Page::Networks => node::networks(store).into_element(),
        Page::Peers => node::peers(store).into_element(),
        Page::Machines | Page::ServerNetworks | Page::Credentials => {
            console::console(store).into_element()
        }
        Page::HostStatus => host::status(store).into_element(),
        Page::Hardening => host::hardening(store).into_element(),
        Page::Deployment => host::deployment(store).into_element(),
        Page::Settings => settings::settings(store).into_element(),
    }
}

/// The notice for a feature this device cannot run, with the page that does it remotely.
fn unavailable(store: Store, limitation: Limitation) -> View {
    let (message, action, alternative): (Pick, Pick, Page) = match limitation {
        Limitation::BrowserCannotJoinNetwork => (
            |s| s.notice_browser_node,
            |s| s.notice_browser_node_action,
            Page::Machines,
        ),
        Limitation::BrowserCannotRunPrograms => (
            |s| s.notice_browser_host,
            |s| s.notice_browser_host_action,
            Page::Deployment,
        ),
        Limitation::MobileCannotHost => (
            |s| s.notice_mobile_host,
            |s| s.notice_mobile_host_action,
            Page::Deployment,
        ),
    };
    capability_notice(store.tr(message), store.tr(action), move || {
        store.open(alternative)
    })
}
