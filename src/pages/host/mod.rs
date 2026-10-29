//! Control-plane host: running easytier-web here, or deploying it to a server.
//!
//! Status and Hardening act on the control plane this desktop hosts; on a phone or in the browser they show a
//! Capability Notice that leads to Deployment, which renders the server files on every platform.

mod deployment;
mod hardening;
mod parts;
mod status;

use nsh_core::prelude::*;

use super::unavailable;
use crate::{
    core::platform::Availability,
    store::{Page, Store},
};

/// The actions `page` puts in the shell's page header: Start / Restart / Stop on Status and Harden now on
/// Hardening, both only where this device can host; Deployment has none.
pub fn header_actions(store: Store, page: Page) -> Option<AnyElement> {
    let Availability::Local(_) = store.capabilities().host else {
        return None;
    };
    match page {
        Page::HostStatus => Some(status::actions(store)),
        Page::Hardening => Some(hardening::actions(store)),
        Page::Deployment
        | Page::Networks
        | Page::Peers
        | Page::Machines
        | Page::ServerNetworks
        | Page::Credentials
        | Page::Settings => None,
    }
}

/// The hosted control plane's state.
pub fn status(store: Store) -> AnyElement {
    match store.capabilities().host {
        Availability::Elsewhere(limitation) => unavailable(store, limitation).into_element(),
        Availability::Local(_) => status::page(store).into_element(),
    }
}

/// First-run hardening of the hosted control plane.
pub fn hardening(store: Store) -> AnyElement {
    match store.capabilities().host {
        Availability::Elsewhere(limitation) => unavailable(store, limitation).into_element(),
        Availability::Local(_) => hardening::page(store).into_element(),
    }
}

/// Server deployment files: rendered on every platform, applied by none.
pub fn deployment(store: Store) -> AnyElement {
    deployment::page(store).into_element()
}

/// The host role's dialogs: its confirmations are the platform's own alerts, so there is nothing to mount.
pub fn overlays(_store: Store) -> View {
    view()
}
