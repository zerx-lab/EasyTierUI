//! What the console asks a machine's core through the server: a network's peers and its credentials.

use nsh_core::{prelude::*, time::unix_ms};
use uuid::Uuid;

use super::{ActionFailure, ActionKind, ConsoleStore, Feed, MachineTab};
use crate::core::{
    remote::{Credential, CredentialRequest, PeerLinks, RemoteError},
    secret::Secret,
    target::TargetId,
};

/// How long a new credential stays valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifetime {
    /// One hour.
    OneHour,
    /// One day.
    OneDay,
    /// Seven days.
    SevenDays,
    /// Thirty days.
    ThirtyDays,
}

impl Lifetime {
    /// Every choice, shortest first.
    pub const ALL: [Lifetime; 4] = [
        Self::OneHour,
        Self::OneDay,
        Self::SevenDays,
        Self::ThirtyDays,
    ];

    /// A stable id for option lists.
    pub fn id(self) -> &'static str {
        match self {
            Self::OneHour => "1h",
            Self::OneDay => "1d",
            Self::SevenDays => "7d",
            Self::ThirtyDays => "30d",
        }
    }

    /// The choice with option id `id`.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.id() == id)
    }

    /// The lifetime in seconds.
    pub fn seconds(self) -> u64 {
        match self {
            Self::OneHour => 3_600,
            Self::OneDay => 86_400,
            Self::SevenDays => 604_800,
            Self::ThirtyDays => 2_592_000,
        }
    }
}

/// The credential picker and list of one server.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CredentialsState {
    /// The machine whose network is picked.
    pub machine: Option<Uuid>,
    /// The picked network instance.
    pub instance: Option<Uuid>,
    /// Its credentials.
    pub list: Feed<Vec<Credential>>,
}

/// The "Generate credential" form's fields.
#[derive(Clone, Copy)]
pub struct CredentialFields {
    /// The form shows.
    pub open: Signal<bool>,
    /// How long the credential stays valid.
    pub lifetime: Signal<Lifetime>,
    /// ACL groups for the node that uses it, separated by commas or lines.
    pub groups: Signal<String>,
    /// Whether that node may relay other nodes' traffic.
    pub relay: Signal<bool>,
    /// Whether several nodes may use it at once.
    pub reusable: Signal<bool>,
    /// The request is in flight.
    pub busy: Signal<bool>,
    /// Why it failed.
    pub error: Signal<Option<RemoteError>>,
}

impl CredentialFields {
    pub(super) fn new() -> Self {
        Self {
            open: signal(false),
            lifetime: signal(Lifetime::OneDay),
            groups: signal(String::new()),
            relay: signal(false),
            reusable: signal(false),
            busy: signal(false),
            error: signal(None),
        }
    }
}

/// A credential just created: its secret is shown once and cannot be read again.
#[derive(Debug, Clone, PartialEq)]
pub struct Issued {
    /// The credential's id.
    pub id: String,
    /// The private key a node joins with.
    pub secret: Secret,
}

/// The peer list dialog of one network.
#[derive(Debug, Clone, PartialEq)]
pub struct PeersDialog {
    /// The server.
    pub target: TargetId,
    /// The machine.
    pub machine: Uuid,
    /// The network instance.
    pub instance: Uuid,
    /// The network's name.
    pub network: String,
    /// The machine's hostname.
    pub host: String,
    /// The peers and their links.
    pub peers: Feed<Vec<PeerLinks>>,
}

/// The ACL groups typed into the credential form.
fn groups_of(text: &str) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for group in text
        .split([',', '\n'])
        .map(str::trim)
        .filter(|group| !group.is_empty())
    {
        if !groups.iter().any(|known| known == group) {
            groups.push(group.to_owned());
        }
    }
    groups
}

impl ConsoleStore {
    // --- Peers -------------------------------------------------------------------------------------------------

    /// Opens the peer list of network `instance` of `machine`; `network` and `host` title it.
    pub fn open_peers(
        &self,
        id: TargetId,
        machine: Uuid,
        instance: Uuid,
        network: String,
        host: String,
    ) {
        self.peers.set(Some(PeersDialog {
            target: id,
            machine,
            instance,
            network,
            host,
            peers: Feed::default(),
        }));
        self.refresh_peers();
    }

    /// Closes the peer list.
    pub fn close_peers(&self) {
        self.peers.set(None);
    }

    /// `machine`'s hostname on `id`, empty when the list does not hold it.
    fn machine_host(&self, id: TargetId, machine: Uuid) -> String {
        self.machines
            .with_untracked(|all| {
                all.get(&id)
                    .and_then(|feed| feed.value.as_ref())
                    .and_then(|list| list.iter().find(|known| known.id == machine))
                    .map(|known| known.hostname.clone())
            })
            .unwrap_or_default()
    }

    /// Shows `tab` in the drawer of `machine` on `id`. The peers tab starts on the machine's first running network
    /// when no peer list of this machine is open yet.
    pub fn show_machine_tab(&self, id: TargetId, machine: Uuid, tab: MachineTab) {
        self.machine_tab.set(tab);
        if tab != MachineTab::Peers {
            return;
        }
        let open_here = self
            .peers
            .with_untracked(|peers| peers.as_ref().is_some_and(|peers| peers.machine == machine));
        if open_here {
            return;
        }
        let first = self.networks.with_untracked(|all| {
            all.get(&id)
                .and_then(|by_machine| by_machine.get(&machine))
                .and_then(|feed| feed.value.as_ref())
                .and_then(|networks| {
                    networks
                        .iter()
                        .find(|network| network.running)
                        .map(|network| (network.instance_id, network.name.clone()))
                })
        });
        let Some((instance, network)) = first else {
            return;
        };
        let host = self.machine_host(id, machine);
        self.open_peers(id, machine, instance, network, host);
    }

    /// Opens `machine`'s drawer on `id` at the peers of network `instance` (named `network`).
    pub fn open_machine_peers(&self, id: TargetId, machine: Uuid, instance: Uuid, network: String) {
        batch(|| {
            self.selected.update(|all| {
                all.insert(id, machine);
            });
            self.machine_tab.set(MachineTab::Peers);
        });
        self.refresh_networks(id, machine);
        let host = self.machine_host(id, machine);
        self.open_peers(id, machine, instance, network, host);
    }

    /// Reads the open peer list again.
    pub fn refresh_peers(&self) {
        let Some(dialog) = self.peers.get_untracked() else {
            return;
        };
        let id = dialog.target;
        let Some(client) = self.client(id) else {
            return;
        };
        if dialog.peers.loading {
            return;
        }
        self.peers.update(|peers| {
            if let Some(peers) = peers {
                peers.peers.begin();
            }
        });
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let result = client.list_peers(dialog.machine, dialog.instance).await;
            if !this.current_epoch(id, epoch) {
                return;
            }
            let still_open = |open: &PeersDialog| {
                open.instance == dialog.instance && open.machine == dialog.machine
            };
            match result {
                Ok(list) => this.peers.update(|peers| {
                    if let Some(peers) = peers.as_mut().filter(|open| still_open(open)) {
                        peers.peers.succeed(list, unix_ms());
                    }
                }),
                Err(error) => {
                    this.peers.update(|peers| {
                        if let Some(peers) = peers.as_mut().filter(|open| still_open(open)) {
                            peers.peers.fail(error.clone());
                        }
                    });
                    this.react_to(id, &error);
                }
            }
        });
    }

    // --- Credentials -------------------------------------------------------------------------------------------

    /// Picks the machine whose networks the credential list is about (reads its networks).
    pub fn pick_credential_machine(&self, id: TargetId, machine: Uuid) {
        self.credentials.update(|all| {
            let state = all.entry(id).or_default();
            *state = CredentialsState {
                machine: Some(machine),
                ..CredentialsState::default()
            };
        });
        self.refresh_networks(id, machine);
    }

    /// Picks the network whose credentials are listed (reads them).
    pub fn pick_credential_network(&self, id: TargetId, instance: Uuid) {
        self.credentials.update(|all| {
            let state = all.entry(id).or_default();
            state.instance = Some(instance);
            state.list = Feed::default();
        });
        self.refresh_credentials(id);
    }

    /// Reads the picked network's credentials again.
    pub fn refresh_credentials(&self, id: TargetId) {
        let state = self.credentials.with_untracked(|all| all.get(&id).cloned());
        let Some(CredentialsState {
            machine: Some(machine),
            instance: Some(instance),
            list,
        }) = state
        else {
            return;
        };
        let Some(client) = self.client(id) else {
            return;
        };
        if list.loading {
            return;
        }
        self.credentials
            .update(|all| all.entry(id).or_default().list.begin());
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let result = client.list_credentials(machine, instance).await;
            if !this.current_epoch(id, epoch) {
                return;
            }
            let picked = |state: &CredentialsState| {
                state.machine == Some(machine) && state.instance == Some(instance)
            };
            match result {
                Ok(list) => this.credentials.update(|all| {
                    if let Some(state) = all.get_mut(&id).filter(|state| picked(state)) {
                        state.list.succeed(list, unix_ms());
                    }
                }),
                Err(error) => {
                    this.credentials.update(|all| {
                        if let Some(state) = all.get_mut(&id).filter(|state| picked(state)) {
                            state.list.fail(error.clone());
                        }
                    });
                    this.react_to(id, &error);
                }
            }
        });
    }

    /// Opens the "Generate credential" form with its defaults.
    pub fn open_credential_form(&self) {
        let form = self.credential;
        batch(|| {
            form.lifetime.set(Lifetime::OneDay);
            form.groups.set(String::new());
            form.relay.set(false);
            form.reusable.set(false);
            form.busy.set(false);
            form.error.set(None);
            form.open.set(true);
        });
    }

    /// Closes the "Generate credential" form.
    pub fn close_credential_form(&self) {
        self.credential.open.set(false);
    }

    /// Creates a credential for the picked network from the form. The secret is shown once, in [`Self::issued`].
    pub fn generate_credential(&self, id: TargetId) {
        let form = self.credential;
        if form.busy.get_untracked() {
            return;
        }
        let state = self.credentials.with_untracked(|all| all.get(&id).cloned());
        let Some(CredentialsState {
            machine: Some(machine),
            instance: Some(instance),
            ..
        }) = state
        else {
            return;
        };
        let Some(client) = self.client(id) else {
            return;
        };
        let mut request = CredentialRequest::new(form.lifetime.get_untracked().seconds());
        request.groups = groups_of(&form.groups.get_untracked());
        request.allow_relay = form.relay.get_untracked();
        request.reusable = form.reusable.get_untracked();
        batch(|| {
            form.busy.set(true);
            form.error.set(None);
        });
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let result = client
                .generate_credential(machine, instance, &request)
                .await;
            form.busy.set(false);
            if !this.current_epoch(id, epoch) {
                return;
            }
            match result {
                Ok(created) => {
                    batch(|| {
                        this.issued.set(Some(Issued {
                            id: created.id,
                            secret: created.secret,
                        }));
                        form.open.set(false);
                    });
                    this.refresh_credentials(id);
                }
                Err(error) => {
                    form.error.set(Some(error.clone()));
                    this.react_to(id, &error);
                }
            }
        });
    }

    /// Forgets the credential secret that was shown.
    pub fn dismiss_issued(&self) {
        self.issued.set(None);
    }

    /// Revokes credential `credential_id` of the picked network.
    pub fn revoke_credential(&self, id: TargetId, credential_id: String) {
        let state = self.credentials.with_untracked(|all| all.get(&id).cloned());
        let Some(CredentialsState {
            machine: Some(machine),
            instance: Some(instance),
            ..
        }) = state
        else {
            return;
        };
        let Some(client) = self.client(id) else {
            return;
        };
        self.failure.set(None);
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let result = client
                .revoke_credential(machine, instance, &credential_id)
                .await;
            if !this.current_epoch(id, epoch) {
                return;
            }
            match result {
                Ok(true) => {}
                Ok(false) => log::info!("credential of server {} was already gone", id.number()),
                Err(error) => {
                    this.react_to(id, &error);
                    this.failure.set(Some(ActionFailure {
                        target: id,
                        kind: ActionKind::Revoke,
                        subject: credential_id,
                        error,
                    }));
                }
            }
            this.refresh_credentials(id);
        });
    }
}
