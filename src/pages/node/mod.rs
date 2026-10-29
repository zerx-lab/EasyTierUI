//! Node: this device in the mesh — the Networks page, the Peers page and the node role's drawers and dialogs.

mod common;
mod detail;
mod failure;
mod flow;
mod format;
mod import;
mod managed;
mod networks;
mod panels;
mod peer_drawer;
mod peers;
mod tun;

use nsh_core::prelude::*;

use super::unavailable;
use crate::{
    core::platform::Availability,
    store::{Page, Store},
};

/// The actions this role's `page` puts in the shell's page header. None: the node pages keep their actions in their
/// toolbars (the primary one, "Create network", at the toolbar's trailing edge), so nothing is offered twice.
pub fn header_actions(_store: Store, _page: Page) -> Option<AnyElement> {
    None
}

/// The networks of this device.
pub fn networks(store: Store) -> AnyElement {
    match store.capabilities().node {
        Availability::Elsewhere(limitation) => unavailable(store, limitation).into_element(),
        Availability::Local(_) => networks::page(store).into_element(),
    }
}

/// The peers of this device's running networks.
pub fn peers(store: Store) -> AnyElement {
    match store.capabilities().node {
        Availability::Elsewhere(limitation) => unavailable(store, limitation).into_element(),
        Availability::Local(_) => peers::page(store).into_element(),
    }
}

/// The node role's drawers and dialogs: the network and peer drawers, create, join, import, enroll and the
/// administrator confirmation.
pub fn overlays(store: Store) -> View {
    view()
        .child(flow::dialog(store))
        .child(tun::dialog(store))
        .child(detail::drawer_view(store))
        .child(peer_drawer::drawer_view(store))
        .child(import::dialog(store))
        .child(managed::dialog(store))
}
