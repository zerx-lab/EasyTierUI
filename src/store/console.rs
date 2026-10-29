//! The console role's state: the easytier-web servers the user added (kept as targets), each one's session,
//! machines, networks and credentials, read through `core::remote`.
//!
//! Every server is a [`Target`](crate::core::target::Target) of the root store; this store keeps what the console
//! knows about it, per target id:
//!
//! - the **session** ([`Session`]) and a [`RemoteClient`] (engine thread only). The session cookie rests in the
//!   keychain (`server/<id>/session`) where the platform has one and is checked with `check_session` before it is
//!   trusted; a password is never stored, and a 401 anywhere ends the session ([`Session::Expired`]).
//! - the **machines**, the **networks** of each machine and the **credentials** of the picked network, each a
//!   [`Feed`] that keeps its last good value while it loads again or fails.
//!
//! Work against a server runs in a task under the store's owner. A result carries the epoch of the session it
//! answers; signing out, removing the server or changing its address bumps the epoch and drops what is in flight.

mod data;
mod editor;
mod inspect;
mod node_pick;
mod overview;
mod rpc;
mod runtime;
mod session;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use nsh_core::prelude::*;
use uuid::Uuid;

pub use self::{
    data::{ActionFailure, ActionKind},
    editor::{
        ENCRYPTION_IDS, Editor, EditorError, EditorFields, EditorMode, FormProblem,
        NetworkingChoice,
    },
    inspect::{CopyState, InstanceViews},
    overview::Overview,
    rpc::{CredentialFields, CredentialsState, Issued, Lifetime, PeersDialog},
    runtime::{Form, ProxyNet, RunningFacts, RuntimeFailure, RuntimeFields, Scope, VpnSummary},
    session::{AddError, AddFields, PasswordError, PasswordFields, SignInError, SignInFields},
};
use super::{Store, StoreEnv};
use crate::core::{
    keychain::SecretStore,
    platform::Platform,
    remote::{Machine, RemoteClient, RemoteError, ServerNetwork},
    target::TargetId,
};

/// Where a server's session stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Session {
    /// Not signed in (never was, signed out, or the app has not looked yet).
    #[default]
    NotSignedIn,
    /// Restoring a kept session or signing in.
    SigningIn,
    /// Signed in; the console reads and changes the server as this account.
    SignedIn {
        /// The account's name; empty when the app does not know it (a browser that kept the session across a
        /// reload).
        account: String,
    },
    /// The server ended the session (a 401): sign in again.
    Expired,
    /// The server did not answer.
    Unreachable {
        /// What went wrong, in the client's words.
        message: String,
    },
}

/// One read of a server's data: the last good value stays while a newer read loads or fails.
#[derive(Debug, Clone, PartialEq)]
pub struct Feed<T> {
    /// The last good value; `None` before the first read succeeded.
    pub value: Option<T>,
    /// A read is in flight.
    pub loading: bool,
    /// Why the last read failed; cleared by the next success.
    pub error: Option<RemoteError>,
    /// Reads failed in a row since the last success: one failure is often a blip (easytier-web gives a machine's
    /// core 5 s per proxied call), so pages warn only when it repeats.
    pub failures: u32,
    /// When the value was read, in Unix milliseconds.
    pub read_at_ms: Option<f64>,
}

impl<T> Default for Feed<T> {
    fn default() -> Self {
        Self {
            value: None,
            loading: false,
            error: None,
            failures: 0,
            read_at_ms: None,
        }
    }
}

impl<T> Feed<T> {
    /// A read starts.
    pub(crate) fn begin(&mut self) {
        self.loading = true;
    }

    /// The read succeeded at `now_ms`.
    pub(crate) fn succeed(&mut self, value: T, now_ms: f64) {
        self.value = Some(value);
        self.loading = false;
        self.error = None;
        self.failures = 0;
        self.read_at_ms = Some(now_ms);
    }

    /// The read failed; the last good value stays.
    pub(crate) fn fail(&mut self, error: RemoteError) {
        self.loading = false;
        self.error = Some(error);
        self.failures = self.failures.saturating_add(1);
    }
}

/// The network feeds of one server, by machine.
pub type NetworkFeeds = BTreeMap<Uuid, Feed<Vec<ServerNetwork>>>;

/// The part of a machine's drawer that shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MachineTab {
    /// The machine's networks.
    #[default]
    Networks,
    /// The peers of one of its running networks.
    Peers,
    /// The node's own facts in one of its running networks.
    Node,
    /// The routing table of a running network.
    Routes,
    /// The foreign networks a running network relays, and the mesh's peer map.
    Relay,
    /// The metrics of a running network.
    Stats,
    /// The manual connectors, mapped listeners and VPN portal of a running network.
    Connections,
    /// The port forwards and proxied subnets of a running network.
    Forwarding,
    /// The access rules and open ports of a running network.
    Access,
    /// The core's log level (the whole machine's, not a network's).
    Logs,
}

impl MachineTab {
    /// Every tab, in the order the drawer lists them.
    pub const ALL: [MachineTab; 10] = [
        Self::Networks,
        Self::Peers,
        Self::Node,
        Self::Routes,
        Self::Relay,
        Self::Stats,
        Self::Connections,
        Self::Forwarding,
        Self::Access,
        Self::Logs,
    ];

    /// The tabs the drawer shows as a strip; the others are chosen from a menu beside it.
    pub const MAIN: [MachineTab; 4] = [Self::Networks, Self::Peers, Self::Node, Self::Routes];

    /// The tabs behind the menu beside the strip.
    pub const MORE: [MachineTab; 6] = [
        Self::Relay,
        Self::Stats,
        Self::Connections,
        Self::Forwarding,
        Self::Access,
        Self::Logs,
    ];

    /// A stable id for option lists.
    pub fn id(self) -> &'static str {
        match self {
            Self::Networks => "networks",
            Self::Peers => "peers",
            Self::Node => "node",
            Self::Routes => "routes",
            Self::Relay => "relay",
            Self::Stats => "stats",
            Self::Connections => "connections",
            Self::Forwarding => "forwarding",
            Self::Access => "access",
            Self::Logs => "logs",
        }
    }

    /// The tab with option id `id`.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// See the module docs. A `Copy` handle of signals, like [`Store`].
#[derive(Clone, Copy)]
pub struct ConsoleStore {
    /// The owner tasks run under: the app's, so a re-run effect or a closed page never cancels a request.
    owner: Owner,
    /// The root store (set by `restore`); a `Store` holds this one, so it cannot be a plain field.
    root: Stored<Option<Store>>,
    secrets: Stored<Arc<dyn SecretStore>>,
    /// Whether the keychain keeps secrets across runs (the session cookie is stored only then).
    persistent: bool,
    /// Whether the session outlives the app here: the keychain, or the browser's own cookie.
    keeps_session: bool,
    /// Whether this device has a source of randomness for generated secrets (a browser has none).
    can_generate: bool,
    clients: Stored<BTreeMap<TargetId, RemoteClient>>,
    /// Targets whose first connection was started.
    started: Stored<BTreeSet<TargetId>>,
    /// Per target, a counter that changes whenever its session is replaced, so a late answer is dropped.
    epochs: Stored<BTreeMap<TargetId, u64>>,
    /// When the hosted control plane was last signed in to automatically.
    auto_at: Stored<BTreeMap<TargetId, f64>>,
    sessions: Signal<BTreeMap<TargetId, Session>>,
    machines: Signal<BTreeMap<TargetId, Feed<Vec<Machine>>>>,
    /// The summary and sessions behind the web build's control-plane pages.
    overview: Signal<BTreeMap<TargetId, Overview>>,
    networks: Signal<BTreeMap<TargetId, NetworkFeeds>>,
    selected: Signal<BTreeMap<TargetId, Uuid>>,
    /// The machine the web build's Node pages manage, per server (kept in `Storage`, see `node_pick`).
    node_picks: Signal<BTreeMap<TargetId, Uuid>>,
    machine_tab: Signal<MachineTab>,
    /// What the machine search field holds.
    machine_query: Signal<String>,
    /// What the network search field holds.
    network_query: Signal<String>,
    /// Network instances with an enable / disable / delete in flight.
    acting: Signal<BTreeSet<Uuid>>,
    failure: Signal<Option<ActionFailure>>,
    /// The server whose password was just changed (its sign-in form says so).
    password_changed: Signal<Option<TargetId>>,
    sign_in: SignInFields,
    add: AddFields,
    password: PasswordFields,
    editor: Signal<Editor>,
    fields: EditorFields,
    credentials: Signal<BTreeMap<TargetId, CredentialsState>>,
    credential: CredentialFields,
    issued: Signal<Option<Issued>>,
    peers: Signal<Option<PeersDialog>>,
    /// What the machine drawer's runtime tabs read and change on a running network.
    runtime: runtime::RuntimeState,
    /// What the machine drawer's inspect tabs read from a running network, and which network they are about.
    inspect: inspect::InspectState,
}

impl ConsoleStore {
    /// Its state at start.
    pub(super) fn new(env: &StoreEnv) -> Self {
        Self {
            owner: Owner::current(),
            root: stored(None),
            secrets: stored(env.secrets.clone()),
            persistent: env.secrets.persistent(),
            keeps_session: env.platform == Platform::Web || env.secrets.persistent(),
            can_generate: env.platform != Platform::Web,
            clients: stored(BTreeMap::new()),
            started: stored(BTreeSet::new()),
            epochs: stored(BTreeMap::new()),
            auto_at: stored(BTreeMap::new()),
            sessions: signal(BTreeMap::new()),
            machines: signal(BTreeMap::new()),
            overview: signal(BTreeMap::new()),
            networks: signal(BTreeMap::new()),
            selected: signal(BTreeMap::new()),
            node_picks: signal(BTreeMap::new()),
            machine_tab: signal(MachineTab::default()),
            machine_query: signal(String::new()),
            network_query: signal(String::new()),
            acting: signal(BTreeSet::new()),
            failure: signal(None),
            password_changed: signal(None),
            sign_in: SignInFields::new(),
            add: AddFields::new(),
            password: PasswordFields::new(),
            editor: signal(Editor::closed()),
            fields: EditorFields::new(),
            credentials: signal(BTreeMap::new()),
            credential: CredentialFields::new(),
            issued: signal(None),
            peers: signal(None),
            runtime: runtime::RuntimeState::new(),
            inspect: inspect::InspectState::new(),
        }
    }

    /// Restores what the role kept across runs, once the root store exists: the servers the user added become
    /// targets, and the selected server connects.
    pub(super) fn restore(&self, store: Store) {
        self.root.with(|root| *root = Some(store));
        self.restore_servers(store);
        self.watch(store);
    }

    /// The root store, once `restore` has run.
    pub(super) fn root(&self) -> Option<Store> {
        self.root.get()
    }

    /// Runs `work` as a task under the store's owner.
    pub(super) fn go(&self, work: impl Future<Output = ()> + 'static) {
        self.owner.run(|| {
            let _task = spawn(work);
        });
    }

    /// Runs `then` once the person confirmed (`asked` resolves `true`): the confirmation's alert is opened by the
    /// caller where the press happened, and the answer is awaited under the store's owner so a row that goes away
    /// meanwhile does not cancel it.
    pub fn when_confirmed(
        &self,
        asked: impl Future<Output = bool> + 'static,
        then: impl FnOnce() + 'static,
    ) {
        self.go(async move {
            if asked.await {
                then();
            }
        });
    }

    /// Whether a signed-in session survives a restart here; where it does not, the sign-in form says so.
    pub fn keeps_session(&self) -> bool {
        self.keeps_session
    }

    /// `id`'s session (tracked).
    pub fn session(&self, id: TargetId) -> Session {
        self.sessions
            .with(|sessions| sessions.get(&id).cloned())
            .unwrap_or_default()
    }

    /// `id`'s machine list (tracked).
    pub fn machines(&self, id: TargetId) -> Feed<Vec<Machine>> {
        self.machines
            .with(|all| all.get(&id).cloned())
            .unwrap_or_default()
    }

    /// The networks of `machine` on `id` (tracked).
    pub fn networks_of(&self, id: TargetId, machine: Uuid) -> Feed<Vec<ServerNetwork>> {
        self.networks
            .with(|all| {
                all.get(&id)
                    .and_then(|by_machine| by_machine.get(&machine))
                    .cloned()
            })
            .unwrap_or_default()
    }

    /// Every machine's network feed on `id` (tracked).
    pub fn all_networks(&self, id: TargetId) -> NetworkFeeds {
        self.networks
            .with(|all| all.get(&id).cloned())
            .unwrap_or_default()
    }

    /// The machine shown in detail on `id` (tracked).
    pub fn selected_machine(&self, id: TargetId) -> Option<Uuid> {
        self.selected.with(|all| all.get(&id).copied())
    }

    /// Shows `machine` in its drawer on `id` (or closes the drawer with `None`), and reads its networks. A drawer
    /// opens on the machine's networks; the peer list of another machine is dropped. The tab is reset on opening,
    /// not on closing: the closing drawer's tab control must not see its value change as it goes away.
    pub fn select_machine(&self, id: TargetId, machine: Option<Uuid>) {
        batch(|| {
            self.selected.update(|all| match machine {
                Some(machine) => {
                    all.insert(id, machine);
                }
                None => {
                    all.remove(&id);
                }
            });
            if machine.is_some() {
                self.machine_tab.set(MachineTab::Networks);
            }
            self.peers.set(None);
        });
        if let Some(machine) = machine {
            self.refresh_networks(id, machine);
        }
    }

    /// The part of the machine drawer that shows (tracked).
    pub fn machine_tab(&self) -> MachineTab {
        self.machine_tab.get()
    }

    /// What the machine search field holds (tracked).
    pub fn machine_query(&self) -> String {
        self.machine_query.get()
    }

    /// Sets what the machine list is filtered by.
    pub fn set_machine_query(&self, query: &str) {
        self.machine_query.set(query.to_owned());
    }

    /// What the network search field holds (tracked).
    pub fn network_query(&self) -> String {
        self.network_query.get()
    }

    /// Sets what the network list is filtered by.
    pub fn set_network_query(&self, query: &str) {
        self.network_query.set(query.to_owned());
    }

    /// Whether an action on network instance `instance` is in flight (tracked).
    pub fn is_acting(&self, instance: Uuid) -> bool {
        self.acting.with(|acting| acting.contains(&instance))
    }

    /// The action that failed last, for a banner (tracked).
    pub fn failure(&self) -> Option<ActionFailure> {
        self.failure.get()
    }

    /// Hides the failure banner.
    pub fn dismiss_failure(&self) {
        self.failure.set(None);
    }

    /// Whether `id`'s password was just changed (tracked): the sign-in form then says the session ended.
    pub fn password_just_changed(&self, id: TargetId) -> bool {
        self.password_changed.with(|changed| *changed == Some(id))
    }

    /// The sign-in form's fields.
    pub fn sign_in_fields(&self) -> SignInFields {
        self.sign_in
    }

    /// The add-server dialog's fields.
    pub fn add_fields(&self) -> AddFields {
        self.add
    }

    /// The change-password dialog's fields.
    pub fn password_fields(&self) -> PasswordFields {
        self.password
    }

    /// The network editor's fields.
    pub fn editor_fields(&self) -> EditorFields {
        self.fields
    }

    /// The network editor's state (tracked).
    pub fn editor(&self) -> Editor {
        self.editor.get()
    }

    /// The credential form's fields.
    pub fn credential_fields(&self) -> CredentialFields {
        self.credential
    }

    /// A credential just created, shown once (tracked).
    pub fn issued(&self) -> Option<Issued> {
        self.issued.get()
    }

    /// The peer list dialog (tracked).
    pub fn peers(&self) -> Option<PeersDialog> {
        self.peers.get()
    }

    /// `id`'s credential picker and list (tracked).
    pub fn credentials(&self, id: TargetId) -> CredentialsState {
        self.credentials
            .with(|all| all.get(&id).cloned())
            .unwrap_or_default()
    }
}
