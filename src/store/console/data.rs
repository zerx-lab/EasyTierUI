//! Reading a server's machines and networks, and changing a network (switch it on or off, delete it).

use nsh_core::{prelude::*, time::unix_ms};
use uuid::Uuid;

use super::ConsoleStore;
use crate::core::{
    remote::{Machine, RemoteClient, RemoteError},
    target::TargetId,
};

/// What a failed action tried to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// Switch a network on.
    Enable,
    /// Switch a network off.
    Disable,
    /// Delete a network.
    Delete,
    /// Revoke a credential.
    Revoke,
}

/// An action the server refused or could not be asked, for the banner above the lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFailure {
    /// The server.
    pub target: TargetId,
    /// What was tried.
    pub kind: ActionKind,
    /// The network or credential it was tried on, by name.
    pub subject: String,
    /// Why it failed.
    pub error: RemoteError,
}

impl ConsoleStore {
    /// Forgets what was read from `id`.
    pub(super) fn clear_data(&self, id: TargetId) {
        batch(|| {
            self.machines.update(|all| {
                all.remove(&id);
            });
            self.networks.update(|all| {
                all.remove(&id);
            });
            self.selected.update(|all| {
                all.remove(&id);
            });
            self.credentials.update(|all| {
                all.remove(&id);
            });
            self.close_overlays_of(id);
        });
    }

    /// Ends every read of `id` that is shown as loading (their answers are dropped with the epoch).
    pub(super) fn stop_loading(&self, id: TargetId) {
        batch(|| {
            self.machines.update(|all| {
                if let Some(feed) = all.get_mut(&id) {
                    feed.loading = false;
                }
            });
            self.networks.update(|all| {
                if let Some(by_machine) = all.get_mut(&id) {
                    for feed in by_machine.values_mut() {
                        feed.loading = false;
                    }
                }
            });
            self.credentials.update(|all| {
                if let Some(state) = all.get_mut(&id) {
                    state.list.loading = false;
                }
            });
        });
    }

    /// Closes the dialogs that act on `id` (its session ended).
    pub(super) fn close_overlays_of(&self, id: TargetId) {
        if self
            .editor
            .with_untracked(|editor| editor.open && editor.target == id)
        {
            self.close_editor();
        }
        if self
            .peers
            .with_untracked(|peers| peers.as_ref().is_some_and(|peers| peers.target == id))
        {
            self.peers.set(None);
        }
        if self.password.open.get_untracked() == Some(id) {
            self.close_change_password();
        }
        if self.credentials.with_untracked(|all| all.contains_key(&id)) {
            self.credential.open.set(false);
        }
        self.issued.set(None);
    }

    // --- Machines --------------------------------------------------------------------------------------------

    /// Reads `id`'s machine list again.
    pub fn refresh_machines(&self, id: TargetId) {
        let Some(client) = self.client(id) else {
            return;
        };
        let loading = self
            .machines
            .with_untracked(|all| all.get(&id).is_some_and(|feed| feed.loading));
        if loading {
            return;
        }
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            this.load_machines(id, epoch, &client).await;
        });
    }

    /// Reads the machine list; the list on success.
    async fn load_machines(
        &self,
        id: TargetId,
        epoch: u64,
        client: &RemoteClient,
    ) -> Option<Vec<Machine>> {
        self.machines
            .update(|all| all.entry(id).or_default().begin());
        let result = client.machines().await;
        if !self.current_epoch(id, epoch) {
            return None;
        }
        match result {
            Ok(list) => {
                self.machines
                    .update(|all| all.entry(id).or_default().succeed(list.clone(), unix_ms()));
                Some(list)
            }
            Err(error) => {
                self.machines
                    .update(|all| all.entry(id).or_default().fail(error.clone()));
                self.react_to(id, &error);
                None
            }
        }
    }

    // --- Networks --------------------------------------------------------------------------------------------

    /// Reads the networks of `machine` on `id` again.
    pub fn refresh_networks(&self, id: TargetId, machine: Uuid) {
        let Some(client) = self.client(id) else {
            return;
        };
        let loading = self.networks.with_untracked(|all| {
            all.get(&id)
                .and_then(|by_machine| by_machine.get(&machine))
                .is_some_and(|feed| feed.loading)
        });
        if loading {
            return;
        }
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            this.load_networks(id, epoch, &client, machine).await;
        });
    }

    /// Reads the machines of `id`, then the networks of every machine, one machine after the other.
    pub fn refresh_all_networks(&self, id: TargetId) {
        let Some(client) = self.client(id) else {
            return;
        };
        let busy = self
            .machines
            .with_untracked(|all| all.get(&id).is_some_and(|feed| feed.loading))
            || self.networks.with_untracked(|all| {
                all.get(&id)
                    .is_some_and(|by_machine| by_machine.values().any(|feed| feed.loading))
            });
        if busy {
            return;
        }
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let Some(machines) = this.load_machines(id, epoch, &client).await else {
                return;
            };
            for machine in machines {
                if !this.current_epoch(id, epoch) {
                    return;
                }
                this.load_networks(id, epoch, &client, machine.id).await;
            }
        });
    }

    /// Reads one machine's networks into its feed.
    async fn load_networks(&self, id: TargetId, epoch: u64, client: &RemoteClient, machine: Uuid) {
        self.networks.update(|all| {
            all.entry(id)
                .or_default()
                .entry(machine)
                .or_default()
                .begin();
        });
        let result = client.networks(machine).await;
        if !self.current_epoch(id, epoch) {
            return;
        }
        match result {
            Ok(list) => self.networks.update(|all| {
                all.entry(id)
                    .or_default()
                    .entry(machine)
                    .or_default()
                    .succeed(list, unix_ms());
            }),
            Err(error) => {
                self.networks.update(|all| {
                    all.entry(id)
                        .or_default()
                        .entry(machine)
                        .or_default()
                        .fail(error.clone());
                });
                self.react_to(id, &error);
            }
        }
    }

    // --- Actions ---------------------------------------------------------------------------------------------

    /// Switches network `instance` of `machine` off (`disabled`) or on. `name` names it in a failure.
    pub fn set_network_disabled(
        &self,
        id: TargetId,
        machine: Uuid,
        instance: Uuid,
        name: String,
        disabled: bool,
    ) {
        let kind = if disabled {
            ActionKind::Disable
        } else {
            ActionKind::Enable
        };
        self.act(
            id,
            machine,
            instance,
            kind,
            name,
            move |client| async move {
                client
                    .set_network_disabled(machine, instance, disabled)
                    .await
            },
        );
    }

    /// Stops network `instance` of `machine` and deletes its saved config. `name` names it in a failure.
    pub fn delete_network(&self, id: TargetId, machine: Uuid, instance: Uuid, name: String) {
        self.act(
            id,
            machine,
            instance,
            ActionKind::Delete,
            name,
            move |client| async move { client.delete_network(machine, instance).await },
        );
    }

    /// Runs `call` against `id`'s server for a network instance, then reads that machine's networks again.
    fn act<Fut>(
        &self,
        id: TargetId,
        machine: Uuid,
        instance: Uuid,
        kind: ActionKind,
        subject: String,
        call: impl FnOnce(RemoteClient) -> Fut + 'static,
    ) where
        Fut: Future<Output = Result<(), RemoteError>> + 'static,
    {
        let Some(client) = self.client(id) else {
            return;
        };
        if self
            .acting
            .with_untracked(|acting| acting.contains(&instance))
        {
            return;
        }
        batch(|| {
            self.failure.set(None);
            self.acting.update(|acting| {
                acting.insert(instance);
            });
        });
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let result = call(client).await;
            this.acting.update(|acting| {
                acting.remove(&instance);
            });
            if !this.current_epoch(id, epoch) {
                return;
            }
            if let Err(error) = result {
                this.react_to(id, &error);
                this.failure.set(Some(ActionFailure {
                    target: id,
                    kind,
                    subject,
                    error,
                }));
            }
            this.refresh_networks(id, machine);
        });
    }
}
