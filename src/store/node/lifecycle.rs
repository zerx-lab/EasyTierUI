//! Starting, stopping, restarting and deleting networks, and the agreement a first TUN start waits for.
//!
//! Each action marks the network busy at a new revision, runs against the backend off the engine thread, and applies
//! its answer only if the revision is still the one it was started at.

use nsh_core::{prelude::*, storage::Storage};

use super::{Failure, NodeStore, Notice, Op, SaveFailure, Status};
use crate::{
    core::{
        acl::GroupSecrets,
        keychain::SecretStore,
        node::{NodeBackend, NodeError, Privilege},
        state::{FailReason, NetworkState},
        work::WorkError,
    },
    profile::{NetworkProfile, NshStorage, ProfileId, repo},
};

/// The storage key remembering that the person agreed to the administrator prompt for a network.
fn agreement_key(id: &ProfileId) -> String {
    format!("node.tun-agreed.{id}")
}

/// What a start left behind: the backend's reading afterwards, and why the start failed, if it did.
struct Started {
    state: NetworkState,
    failure: Option<Failure>,
}

/// A failure the core's own words describe.
fn core_failure(error: NodeError) -> Failure {
    Failure::Core(error.into_fail_reason())
}

/// Blocking: loads the secret, renders and checks the config, starts the instance, then reads it.
fn run_start(
    backend: &dyn NodeBackend,
    secrets: &dyn SecretStore,
    profile: &NetworkProfile,
    machine_id: &str,
) -> Started {
    let outcome = try_start(backend, secrets, profile, machine_id);
    let state = profile
        .id
        .instance_key()
        .map(|key| backend.poll(&key))
        .unwrap_or_default();
    Started {
        state,
        failure: outcome.err(),
    }
}

fn try_start(
    backend: &dyn NodeBackend,
    secrets: &dyn SecretStore,
    profile: &NetworkProfile,
    machine_id: &str,
) -> Result<(), Failure> {
    let secret = match repo::load_secret(secrets, profile) {
        Ok(Some(secret)) => secret,
        Ok(None) => return Err(Failure::SecretMissing),
        Err(error) => return Err(Failure::Core(FailReason::Other(error.to_string()))),
    };
    let groups = if profile.is_managed() {
        GroupSecrets::default()
    } else {
        repo::load_group_secrets(secrets, profile)
            .map_err(|error| Failure::Core(FailReason::Other(error.to_string())))?
    };
    let launch = profile
        .launch(&secret, &groups, machine_id)
        .map_err(|error| Failure::Core(FailReason::ConfigRejected(error.to_string())))?;
    // An installed OS service cannot be started as the user's child: switching from TUN to no TUN removes it first.
    if launch.privilege == Privilege::User && backend.service_installed(&launch.key) {
        backend.remove(&launch.key).map_err(core_failure)?;
    }
    backend.check(&launch).map_err(core_failure)?;
    backend.start(&launch).map_err(core_failure)
}

/// Blocking: removes the instance's service and files, then forgets its secrets.
fn run_remove(
    backend: Option<&dyn NodeBackend>,
    secrets: &dyn SecretStore,
    id: &ProfileId,
) -> Result<(), SaveFailure> {
    if let (Some(backend), Ok(key)) = (backend, id.instance_key()) {
        backend
            .remove(&key)
            .map_err(|error| SaveFailure::Remove(error.to_string()))?;
    }
    let accounts = [
        id.network_secret_account(),
        id.config_server_account(),
        id.acl_group_secrets_account(),
    ];
    for account in accounts {
        secrets.delete(&account).map_err(|error| {
            log::warn!("the keychain would not forget {account}: {error}");
            SaveFailure::Keychain
        })?;
    }
    Ok(())
}

impl NodeStore {
    /// The Start button: starts network `id`, first asking to agree to the administrator prompt if it is a TUN
    /// network that never ran. Does nothing while it runs or an action is in flight.
    pub fn start(&self, id: ProfileId) {
        if !self.alive() {
            return;
        }
        match self.status_untracked(&id) {
            Status::Running | Status::Starting | Status::Stopping | Status::Removing => {}
            Status::Stopped | Status::Failed(_) => self.start_or_ask(id),
        }
    }

    /// Restarts network `id` with its current profile (after an edit, or after it exited).
    pub fn restart(&self, id: ProfileId) {
        if !self.alive() {
            return;
        }
        match self.status_untracked(&id) {
            Status::Stopping | Status::Removing => {}
            Status::Running | Status::Starting | Status::Stopped | Status::Failed(_) => {
                self.start_or_ask(id);
            }
        }
    }

    fn start_or_ask(&self, id: ProfileId) {
        let Some(profile) = self.profile_untracked(&id) else {
            return;
        };
        let needs_agreement = self.platform.is_desktop()
            && profile.privilege() == Privilege::System
            && Storage::get_item(&agreement_key(&id)).is_none();
        if needs_agreement {
            self.close_dialogs();
            self.tun_confirm.set(Some(id));
        } else {
            self.start_now(id);
        }
    }

    /// Starts `id` now, without asking.
    pub(super) fn start_now(&self, id: ProfileId) {
        if !self.alive() {
            return;
        }
        let Some(profile) = self.profile_untracked(&id) else {
            return;
        };
        let Some(backend) = self.backend.get() else {
            // No way to run a network here (no EasyTier program wired up): the card says the core is missing.
            self.change(&id, |runtime| {
                runtime.failure = Some(Failure::Core(FailReason::CoreMissing));
            });
            return;
        };
        let machine_id = if profile.is_managed() {
            match repo::machine_id(&NshStorage) {
                Ok(machine_id) => machine_id,
                Err(error) => {
                    log::error!("no machine id for {id}: {error}");
                    self.change(&id, |runtime| {
                        runtime.failure = Some(Failure::Core(FailReason::Other(error.to_string())));
                    });
                    return;
                }
            }
        } else {
            String::new()
        };
        let revision = self.begin_op(&id, Op::Starting);
        let secrets = self.secrets.get();
        let this = *self;
        spawn(async move {
            let answer = crate::core::work::off_thread(move || {
                run_start(backend.as_ref(), secrets.as_ref(), &profile, &machine_id)
            })
            .await;
            this.finish_start(&id, revision, answer);
        });
    }

    fn finish_start(&self, id: &ProfileId, revision: u64, answer: Result<Started, WorkError>) {
        self.change(id, |runtime| {
            if runtime.revision != revision {
                return;
            }
            runtime.op = None;
            match answer {
                Ok(started) => {
                    runtime.failure = started.failure;
                    // The core has not said "running" yet: until it does the card says "starting".
                    runtime.state = match (started.state, runtime.failure.is_some()) {
                        (NetworkState::Stopped, false) => NetworkState::Starting,
                        (state, _) => state,
                    };
                }
                Err(error) => {
                    runtime.failure = Some(Failure::Core(FailReason::Other(error.to_string())));
                }
            }
        });
    }

    /// Marks `id` busy with `op` at a new revision and forgets the last failure; returns the revision.
    fn begin_op(&self, id: &ProfileId, op: Op) -> u64 {
        let mut revision = 0;
        self.change(id, |runtime| {
            runtime.revision = runtime.revision.wrapping_add(1);
            runtime.op = Some(op);
            runtime.failure = None;
            runtime.stop_error = None;
            revision = runtime.revision;
        });
        revision
    }

    /// The Stop button.
    pub fn stop(&self, id: ProfileId) {
        let this = *self;
        spawn(async move {
            this.stop_task(id).await;
        });
    }

    /// Stops `id`; `true` when it is stopped afterwards.
    async fn stop_task(self, id: ProfileId) -> bool {
        if !self.alive() {
            return false;
        }
        if matches!(
            self.status_untracked(&id),
            Status::Stopping | Status::Removing
        ) {
            return false;
        }
        let (Some(backend), Ok(key)) = (self.backend.get(), id.instance_key()) else {
            return false;
        };
        let revision = self.begin_op(&id, Op::Stopping);
        let answer = crate::core::work::off_thread(move || {
            let stopped = backend.stop(&key);
            (stopped, backend.poll(&key))
        })
        .await;
        let mut stopped_ok = false;
        self.change(&id, |runtime| {
            if runtime.revision != revision {
                return;
            }
            runtime.op = None;
            match answer {
                Ok((Ok(()), _)) => {
                    runtime.state = NetworkState::Stopped;
                    stopped_ok = true;
                }
                Ok((Err(error), state)) => {
                    runtime.stop_error = Some(error.into_fail_reason());
                    runtime.state = state;
                }
                Err(error) => {
                    runtime.stop_error = Some(FailReason::Other(error.to_string()));
                }
            }
        });
        stopped_ok
    }

    /// Stops every network that runs or is starting, one after another (each TUN network may ask for the
    /// administrator), and resolves with the ones that stopped, to start again with [`NodeStore::start_profiles`]
    /// (an EasyTier version switch).
    pub fn stop_all_running(&self) -> impl Future<Output = Vec<ProfileId>> + 'static {
        let this = *self;
        async move {
            let running: Vec<ProfileId> = if this.alive() {
                this.runtime.with_untracked(|map| {
                    map.iter()
                        .filter(|(_, runtime)| runtime.state.is_active() && runtime.op.is_none())
                        .map(|(id, _)| id.clone())
                        .collect()
                })
            } else {
                Vec::new()
            };
            let mut stopped = Vec::new();
            for id in running {
                if this.stop_task(id.clone()).await {
                    stopped.push(id);
                }
            }
            stopped
        }
    }

    /// Starts the networks `ids` again, without asking again (they ran before).
    pub fn start_profiles(&self, ids: Vec<ProfileId>) {
        for id in ids {
            self.start_now(id);
        }
    }

    /// Deletes network `id`: stops it, removes its OS service and files, forgets its secret and profile. If a step
    /// fails the network stays and the list says why.
    pub fn delete(&self, id: ProfileId) {
        if !self.alive() {
            return;
        }
        if matches!(self.status_untracked(&id), Status::Removing) {
            return;
        }
        let revision = self.begin_op(&id, Op::Removing);
        let backend = self.backend.get();
        let secrets = self.secrets.get();
        let this = *self;
        spawn(async move {
            let removed = {
                let id = id.clone();
                crate::core::work::off_thread(move || {
                    run_remove(backend.as_deref(), secrets.as_ref(), &id)
                })
                .await
            };
            if !this.alive() {
                return;
            }
            let outcome = match removed {
                Ok(Ok(())) => repo::remove_profile(&NshStorage, &id)
                    .map(|_| ())
                    .map_err(super::forms::save_failure),
                Ok(Err(failure)) => Err(failure),
                Err(error) => {
                    log::warn!("deleting {id} was lost: {error}");
                    Err(SaveFailure::Background)
                }
            };
            match outcome {
                Ok(()) => this.forget(&id),
                Err(failure) => {
                    this.change(&id, |runtime| {
                        if runtime.revision == revision {
                            runtime.op = None;
                        }
                    });
                    this.notice.set(Some(Notice::Failed(failure)));
                }
            }
        });
    }

    /// Drops everything the app holds of the deleted network `id`.
    fn forget(&self, id: &ProfileId) {
        Storage::remove_item(&agreement_key(id));
        self.polling.with(|set| {
            set.remove(id);
        });
        if self.detail.get_untracked().as_ref() == Some(id) {
            self.close_dialogs();
        }
        self.profiles
            .update(|profiles| profiles.retain(|p| &p.id != id));
        self.runtime.update(|map| {
            map.remove(id);
        });
        self.sync_target();
        self.clear_history(id);
    }

    /// The person agreed to the administrator prompt and the service: remembers it and starts `id`.
    pub fn confirm_tun(&self, id: ProfileId) {
        if !self.alive() {
            return;
        }
        if let Err(full) = Storage::set_item(agreement_key(&id), "1") {
            log::warn!(
                "the agreement was not kept: storage full at {} bytes",
                full.bytes
            );
        }
        self.tun_confirm.set(None);
        self.start_now(id);
    }

    /// The alternative to the prompt: saves `id` without TUN and starts it as the user's own process.
    pub fn start_without_tun(&self, id: ProfileId) {
        if !self.alive() {
            return;
        }
        self.tun_confirm.set(None);
        let Some(mut profile) = self.profile_untracked(&id) else {
            return;
        };
        profile.tun = false;
        let this = *self;
        spawn(async move {
            match this.persist(profile, None, None).await {
                Ok(()) => this.start_now(id),
                Err(failure) => {
                    if this.alive() {
                        this.notice.set(Some(Notice::Failed(failure)));
                    }
                }
            }
        });
    }

    /// The person declined the administrator prompt: nothing starts.
    pub fn cancel_tun(&self) {
        if self.alive() {
            self.tun_confirm.set(None);
        }
    }
}
