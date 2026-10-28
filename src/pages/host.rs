//! Control-plane host: running easytier-web here, or deploying it to a server.

use nsh_core::prelude::*;

use super::unavailable;
use crate::{components::empty_state, core::platform::Availability, store::Store};

/// The hosted control plane's state.
pub fn status(store: Store) -> AnyElement {
    match store.capabilities().host {
        Availability::Elsewhere(limitation) => unavailable(store, limitation).into_element(),
        Availability::Local(_) => empty_state(
            store.tr(|s| s.host_stopped_title),
            store.tr(|s| s.host_stopped_body),
        )
        .into_element(),
    }
}

/// First-run hardening of the hosted control plane.
pub fn hardening(store: Store) -> AnyElement {
    match store.capabilities().host {
        Availability::Elsewhere(limitation) => unavailable(store, limitation).into_element(),
        Availability::Local(_) => empty_state(
            store.tr(|s| s.hardening_idle_title),
            store.tr(|s| s.hardening_idle_body),
        )
        .into_element(),
    }
}

/// Server deployment files: rendered on every platform, applied by none.
pub fn deployment(store: Store) -> AnyElement {
    empty_state(
        store.tr(|s| s.deployment_empty_title),
        store.tr(|s| s.deployment_empty_body),
    )
    .into_element()
}
