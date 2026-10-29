//! The console's parts the web build's Node pages reuse: the sign-in gate, the server toolbar, one machine's
//! networks and peers, and the failure banner. The Node pages manage a machine of the serving server, so they show
//! exactly what the machine drawer shows.

use nsh_core::prelude::*;
use uuid::Uuid;

use super::{account, gate, machine, machines, networks};
use crate::{
    core::{remote::RemoteError, target::TargetId},
    store::Store,
};

/// `ready(id)` while the selected server is signed in; otherwise what is missing, with a way forward.
pub(crate) fn gate(store: Store, ready: impl Fn(TargetId) -> AnyElement + 'static) -> AnyElement {
    gate::gate(store, ready)
}

/// The toolbar of a signed-in page: the server's identity, then `context`, and at the trailing edge how fresh the
/// data is, refresh and the account menu.
pub(crate) fn server_toolbar(
    store: Store,
    id: TargetId,
    context: Vec<AnyElement>,
    refresh: impl Fn() + 'static,
    read_at: impl Fn() -> Option<f64> + 'static,
) -> View {
    account::server_toolbar(store, id, context, true, refresh, read_at)
}

/// The banner of an action on `id` that failed.
pub(crate) fn failure_banner(store: Store, id: TargetId) -> AnyElement {
    account::failure_banner(store, id)
}

/// A failed read with its reason and a retry.
pub(crate) fn load_failed(
    store: Store,
    error: &RemoteError,
    retry: impl Fn() + 'static,
) -> AnyElement {
    networks::load_failed(store, error, retry)
}

/// A signal of the clock that ticks while the page shows, for judging "online".
pub(crate) fn clock() -> Signal<f64> {
    machines::clock()
}

/// One machine's networks as a table with their actions.
pub(crate) fn machine_networks(store: Store, id: TargetId, machine_id: Uuid) -> AnyElement {
    machine::networks_tab(store, id, machine_id)
}

/// The peers of one of one machine's running networks: which network, refresh, and the table.
pub(crate) fn machine_peers(store: Store, id: TargetId, machine_id: Uuid) -> AnyElement {
    machine::peers_tab(store, id, machine_id)
}
