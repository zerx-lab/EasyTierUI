//! Node: this device in the mesh.

use nsh_core::prelude::*;

use super::unavailable;
use crate::{components::empty_state, core::platform::Availability, store::Store};

/// The networks of this device.
pub fn networks(store: Store) -> AnyElement {
    match store.capabilities().node {
        Availability::Elsewhere(limitation) => unavailable(store, limitation).into_element(),
        Availability::Local(_) => empty_state(
            store.tr(|s| s.networks_empty_title),
            store.tr(|s| s.networks_empty_body),
        )
        .into_element(),
    }
}

/// The peers of this device's running networks.
pub fn peers(store: Store) -> AnyElement {
    match store.capabilities().node {
        Availability::Elsewhere(limitation) => unavailable(store, limitation).into_element(),
        Availability::Local(_) => empty_state(
            store.tr(|s| s.peers_empty_title),
            store.tr(|s| s.peers_empty_body),
        )
        .into_element(),
    }
}
