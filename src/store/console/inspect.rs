//! What the machine drawer reads from one running network of a machine through the server: the node's facts, its
//! routes, the foreign networks it relays, its metrics, and the mesh's peer map. Which running network the drawer's
//! per-network tabs are about is kept here too (the scope).
//!
//! Every read is a [`Feed`] under the key it asks about, so an answer for another network never lands in the one
//! shown. An answer also carries the epoch of the session it was asked in; signing out drops it.

use std::{collections::BTreeMap, time::Duration};

use nsh_core::{prelude::*, time::unix_ms};
use uuid::Uuid;

use super::{ConsoleStore, Feed};
use crate::core::{
    remote::{
        ForeignNetwork, ForeignNetworkHost, ForeignNetworkSummary, Metric, NodeFacts, PeerMapNode,
        RemoteClient, RemoteError, RouteEntry,
    },
    target::TargetId,
};

/// How long the stats tab says the Prometheus text was copied.
const COPIED_FOR: Duration = Duration::from_millis(1500);

/// A server, a machine and one of its network instances.
type Key = (TargetId, Uuid, Uuid);

/// A server and a machine.
type MachineKey = (TargetId, Uuid);

/// What was read from one network instance of a machine.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InstanceViews {
    /// The node's own facts.
    pub node: Feed<NodeFacts>,
    /// The routing table.
    pub routes: Feed<Vec<RouteEntry>>,
    /// The networks this node relays although it is no member.
    pub foreign: Feed<Vec<ForeignNetwork>>,
    /// The foreign networks relayed anywhere in the mesh.
    pub global_foreign: Feed<Vec<ForeignNetworkHost>>,
    /// How many foreign networks and peers each relaying node carries.
    pub foreign_summary: Feed<Vec<ForeignNetworkSummary>>,
    /// The metrics.
    pub stats: Feed<Vec<Metric>>,
}

impl InstanceViews {
    /// Ends every read shown as loading.
    fn stop_loading(&mut self) {
        self.node.loading = false;
        self.routes.loading = false;
        self.foreign.loading = false;
        self.global_foreign.loading = false;
        self.foreign_summary.loading = false;
        self.stats.loading = false;
    }
}

/// Where copying the Prometheus text stands.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CopyState {
    /// Nothing happened lately.
    #[default]
    Idle,
    /// The text is being read from the machine.
    Reading,
    /// The text is on the clipboard.
    Copied,
    /// The machine or the server did not give the text.
    Failed(RemoteError),
}

/// The signals of this module, one handle in [`ConsoleStore`].
#[derive(Clone, Copy)]
pub(super) struct InspectState {
    /// The running network the per-network tabs are about, per machine.
    scope: Signal<BTreeMap<MachineKey, Uuid>>,
    views: Signal<BTreeMap<Key, InstanceViews>>,
    peer_maps: Signal<BTreeMap<MachineKey, Feed<Vec<PeerMapNode>>>>,
    prometheus: Signal<CopyState>,
}

impl InspectState {
    /// Nothing read yet.
    pub(super) fn new() -> Self {
        Self {
            scope: signal(BTreeMap::new()),
            views: signal(BTreeMap::new()),
            peer_maps: signal(BTreeMap::new()),
            prometheus: signal(CopyState::Idle),
        }
    }
}

impl ConsoleStore {
    // --- The scope -------------------------------------------------------------------------------------------

    /// The network instance the per-network tabs of `machine` on `id` were pointed at (tracked), if any.
    pub fn scope_instance(&self, id: TargetId, machine: Uuid) -> Option<Uuid> {
        self.inspect
            .scope
            .with(|all| all.get(&(id, machine)).copied())
    }

    /// Points the per-network tabs of `machine` on `id` at network instance `instance`.
    pub fn pick_scope(&self, id: TargetId, machine: Uuid, instance: Uuid) {
        self.inspect.scope.update(|all| {
            all.insert((id, machine), instance);
        });
    }

    // --- Reading ---------------------------------------------------------------------------------------------

    /// One read of network `instance` of `machine` on `id`, chosen by `pick` (tracked).
    pub fn inspect_feed<T: Clone>(
        &self,
        id: TargetId,
        machine: Uuid,
        instance: Uuid,
        pick: fn(&InstanceViews) -> &Feed<T>,
    ) -> Feed<T> {
        self.inspect
            .views
            .with(|all| {
                all.get(&(id, machine, instance))
                    .map(|views| pick(views).clone())
            })
            .unwrap_or_default()
    }

    /// The peer map the core of `machine` on `id` reads from the mesh's peer center (tracked).
    pub fn peer_map(&self, id: TargetId, machine: Uuid) -> Feed<Vec<PeerMapNode>> {
        self.inspect
            .peer_maps
            .with(|all| all.get(&(id, machine)).cloned())
            .unwrap_or_default()
    }

    /// Reads the node facts of the network again.
    pub fn refresh_node(&self, id: TargetId, machine: Uuid, instance: Uuid) {
        self.read_view(
            id,
            machine,
            instance,
            |views| &mut views.node,
            move |client| async move { client.node_info(machine, instance).await },
        );
    }

    /// Reads the routing table of the network again.
    pub fn refresh_routes(&self, id: TargetId, machine: Uuid, instance: Uuid) {
        self.read_view(
            id,
            machine,
            instance,
            |views| &mut views.routes,
            move |client| async move { client.list_routes(machine, instance).await },
        );
    }

    /// Reads everything the relay tab shows again: the foreign networks here and across the mesh, the summary and
    /// the peer map.
    pub fn refresh_relay(&self, id: TargetId, machine: Uuid, instance: Uuid) {
        self.read_view(
            id,
            machine,
            instance,
            |views| &mut views.foreign,
            move |client| async move { client.list_foreign_networks(machine, instance).await },
        );
        self.read_view(
            id,
            machine,
            instance,
            |views| &mut views.global_foreign,
            move |client| async move {
                client
                    .list_global_foreign_networks(machine, instance)
                    .await
            },
        );
        self.read_view(
            id,
            machine,
            instance,
            |views| &mut views.foreign_summary,
            move |client| async move { client.foreign_network_summary(machine, instance).await },
        );
        self.refresh_peer_map(id, machine);
    }

    /// Reads the metrics of the network again.
    pub fn refresh_stats(&self, id: TargetId, machine: Uuid, instance: Uuid) {
        self.read_view(
            id,
            machine,
            instance,
            |views| &mut views.stats,
            move |client| async move { client.stats(machine, instance).await },
        );
    }

    /// Reads the mesh's peer map again.
    pub fn refresh_peer_map(&self, id: TargetId, machine: Uuid) {
        let Some(client) = self.client(id) else {
            return;
        };
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let mut started = false;
            this.inspect.peer_maps.update(|all| {
                let feed = all.entry((id, machine)).or_default();
                if !feed.loading {
                    feed.begin();
                    started = true;
                }
            });
            if !started {
                return;
            }
            let result = client.global_peer_map(machine).await;
            if !this.current_epoch(id, epoch) {
                return;
            }
            match result {
                Ok(map) => this.inspect.peer_maps.update(|all| {
                    all.entry((id, machine))
                        .or_default()
                        .succeed(map, unix_ms());
                }),
                Err(error) => {
                    this.inspect.peer_maps.update(|all| {
                        all.entry((id, machine)).or_default().fail(error.clone());
                    });
                    this.react_to(id, &error);
                }
            }
        });
    }

    /// Reads one view of an instance into its feed. The read starts in the task, so asking for it while a tab is
    /// being built never writes a signal there; a read already in flight is not repeated.
    fn read_view<T, Fut>(
        &self,
        id: TargetId,
        machine: Uuid,
        instance: Uuid,
        feed: fn(&mut InstanceViews) -> &mut Feed<T>,
        call: impl FnOnce(RemoteClient) -> Fut + 'static,
    ) where
        T: 'static,
        Fut: Future<Output = Result<T, RemoteError>> + 'static,
    {
        let Some(client) = self.client(id) else {
            return;
        };
        let key = (id, machine, instance);
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let mut started = false;
            this.inspect.views.update(|all| {
                let slot = feed(all.entry(key).or_default());
                if !slot.loading {
                    slot.begin();
                    started = true;
                }
            });
            if !started {
                return;
            }
            let result = call(client).await;
            if !this.current_epoch(id, epoch) {
                return;
            }
            match result {
                Ok(value) => this.inspect.views.update(|all| {
                    feed(all.entry(key).or_default()).succeed(value, unix_ms());
                }),
                Err(error) => {
                    this.inspect.views.update(|all| {
                        feed(all.entry(key).or_default()).fail(error.clone());
                    });
                    this.react_to(id, &error);
                }
            }
        });
    }

    // --- Copying the Prometheus text ---------------------------------------------------------------------------

    /// Where copying the Prometheus text stands (tracked).
    pub fn prometheus_copy(&self) -> CopyState {
        self.inspect.prometheus.get()
    }

    /// Reads the network's metrics in Prometheus text form from the machine and puts them on the clipboard.
    pub fn copy_prometheus(&self, id: TargetId, machine: Uuid, instance: Uuid) {
        let Some(client) = self.client(id) else {
            return;
        };
        if self
            .inspect
            .prometheus
            .with_untracked(|state| *state == CopyState::Reading)
        {
            return;
        }
        self.inspect.prometheus.set(CopyState::Reading);
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let result = client.prometheus_stats(machine, instance).await;
            if !this.current_epoch(id, epoch) {
                this.inspect.prometheus.set(CopyState::Idle);
                return;
            }
            match result {
                Ok(text) => {
                    set_clipboard(text);
                    this.inspect.prometheus.set(CopyState::Copied);
                    let _timer = set_timeout(COPIED_FOR, move || {
                        this.inspect.prometheus.set(CopyState::Idle);
                    });
                }
                Err(error) => {
                    this.react_to(id, &error);
                    this.inspect.prometheus.set(CopyState::Failed(error));
                }
            }
        });
    }

    // --- Forgetting ------------------------------------------------------------------------------------------

    /// Forgets what was read from `id`.
    pub(super) fn clear_inspect(&self, id: TargetId) {
        batch(|| {
            self.inspect
                .scope
                .update(|all| all.retain(|key, _| key.0 != id));
            self.inspect
                .views
                .update(|all| all.retain(|key, _| key.0 != id));
            self.inspect
                .peer_maps
                .update(|all| all.retain(|key, _| key.0 != id));
            self.inspect.prometheus.set(CopyState::Idle);
        });
    }

    /// Ends every read of `id` that is shown as loading (their answers are dropped with the epoch).
    pub(super) fn stop_inspect_loading(&self, id: TargetId) {
        batch(|| {
            self.inspect.views.update(|all| {
                for (_, views) in all.iter_mut().filter(|(key, _)| key.0 == id) {
                    views.stop_loading();
                }
            });
            self.inspect.peer_maps.update(|all| {
                for (_, feed) in all.iter_mut().filter(|(key, _)| key.0 == id) {
                    feed.loading = false;
                }
            });
            if self
                .inspect
                .prometheus
                .with_untracked(|state| *state == CopyState::Reading)
            {
                self.inspect.prometheus.set(CopyState::Idle);
            }
        });
    }
}
