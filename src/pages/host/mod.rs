//! Control-plane host: running easytier-web here, or deploying it to a server.
//!
//! Status and Hardening act on the control plane this desktop hosts. In the browser they are views of the serving
//! easytier-web through its REST API (behind the console sign-in gate); on a phone they show a Capability Notice
//! that leads to Deployment, which renders the server files on every platform.

mod deployment;
mod hardening;
mod parts;
mod status;
mod web_hardening;
mod web_status;

use nsh_core::prelude::*;

use super::mobile_host_notice;
use crate::{
    core::platform::{Availability, Limitation},
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
        Availability::Elsewhere(Limitation::MobileCannotHost) => {
            mobile_host_notice(store).into_element()
        }
        Availability::Elsewhere(_) => web_status::page(store),
        Availability::Local(_) => status::page(store).into_element(),
    }
}

/// First-run hardening of the hosted control plane.
pub fn hardening(store: Store) -> AnyElement {
    match store.capabilities().host {
        Availability::Elsewhere(Limitation::MobileCannotHost) => {
            mobile_host_notice(store).into_element()
        }
        Availability::Elsewhere(_) => web_hardening::page(store),
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
