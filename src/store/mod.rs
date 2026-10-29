//! App state: signals behind functions. Pages read through the getters (tracked where they are read) and change
//! state only through the functions here; each target keeps its own page, so switching targets loses nothing.
//!
//! Each role keeps its state in its own sub-store (`node`, `console`, `host`, `versions`), reached through
//! [`Store::node`] and friends; the root keeps what the shell needs: targets, pages, language, sidebar.

pub mod console;
pub mod host;
pub mod node;
pub mod versions;

use std::{collections::BTreeMap, sync::Arc};

use nsh_core::{prelude::*, storage::Storage};

pub use self::{console::ConsoleStore, host::HostStore, node::NodeStore, versions::VersionStore};
use crate::{
    core::{
        keychain::{SecretStore, system_store},
        node::ProgramSource,
        platform::{Capabilities, Platform},
        role::Role,
        target::{Target, TargetId, TargetKind, TargetStatus},
    },
    i18n::{LanguageChoice, Pick, Strings},
};

/// The storage key of the user's language choice.
const LANGUAGE_KEY: &str = "ui.language";
/// The storage key of the sidebar's state, set to [`SIDEBAR_COLLAPSED`] while the user keeps it hidden.
const SIDEBAR_KEY: &str = "ui.sidebar";
const SIDEBAR_COLLAPSED: &str = "collapsed";
/// The storage key of the highest server id ever handed out (ids are not reused).
const ID_HIGH_WATER_KEY: &str = "console.id-high-water.v1";
/// Test runs set this storage key to `memory` so secrets never reach the developer's keychain. Debug builds only:
/// a release build never lets plaintext storage choose where secrets go.
#[cfg(debug_assertions)]
const TEST_SECRET_STORE_KEY: &str = "test.secret-store";
/// Test runs set this storage key to a temporary directory used as the data directory (debug builds only).
#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
const TEST_DATA_DIR_KEY: &str = "test.data-dir";

/// Where secrets are kept: the system keychain, or in a debug build a test's in-memory store.
#[cfg(debug_assertions)]
fn secret_store() -> Arc<dyn SecretStore> {
    if Storage::get_item(TEST_SECRET_STORE_KEY).as_deref() == Some("memory") {
        Arc::new(crate::core::keychain::MemoryStore::default())
    } else {
        system_store()
    }
}

/// Where secrets are kept: the system keychain.
#[cfg(not(debug_assertions))]
fn secret_store() -> Arc<dyn SecretStore> {
    system_store()
}

/// What the sub-stores share, handed to each at construction.
#[derive(Clone)]
pub struct StoreEnv {
    /// Where the app runs.
    pub platform: Platform,
    /// Where secrets rest.
    pub secrets: Arc<dyn SecretStore>,
    /// The active EasyTier release on the desktops (`None` elsewhere).
    pub programs: Option<Arc<dyn ProgramSource>>,
}

/// A page of the shell: the same pages on every platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Page {
    /// Node: the networks of this device.
    Networks,
    /// Node: the peers of the running networks.
    Peers,
    /// Console: a server's machines.
    Machines,
    /// Console: a server's networks.
    ServerNetworks,
    /// Console: a server's credentials.
    Credentials,
    /// Host: the hosted control plane's state.
    HostStatus,
    /// Host: first-run hardening.
    Hardening,
    /// Host: server deployment files.
    Deployment,
    /// The app's settings.
    Settings,
}

impl Page {
    /// The pages of `role`, in the order the shell lists them.
    pub fn of_role(role: Role) -> &'static [Page] {
        match role {
            Role::Node => &[Page::Networks, Page::Peers],
            Role::Console => &[Page::Machines, Page::ServerNetworks, Page::Credentials],
            Role::Host => &[Page::HostStatus, Page::Hardening, Page::Deployment],
        }
    }

    /// Its title.
    pub fn title(self, strings: &'static Strings) -> &'static str {
        match self {
            Page::Networks => strings.page_networks,
            Page::Peers => strings.page_peers,
            Page::Machines => strings.page_machines,
            Page::ServerNetworks => strings.page_server_networks,
            Page::Credentials => strings.page_credentials,
            Page::HostStatus => strings.page_host_status,
            Page::Hardening => strings.page_hardening,
            Page::Deployment => strings.page_deployment,
            Page::Settings => strings.page_settings,
        }
    }

    /// A stable key for tests and traces (`page-<key>`), independent of the language.
    pub fn key(self) -> &'static str {
        match self {
            Page::Networks => "networks",
            Page::Peers => "peers",
            Page::Machines => "machines",
            Page::ServerNetworks => "server-networks",
            Page::Credentials => "credentials",
            Page::HostStatus => "host-status",
            Page::Hardening => "hardening",
            Page::Deployment => "deployment",
            Page::Settings => "settings",
        }
    }

    /// The page a target opens on: its node for this device, its console for a server.
    fn first_of(kind: TargetKind) -> Page {
        match kind {
            TargetKind::ThisDevice => Page::Networks,
            TargetKind::Server => Page::Machines,
            TargetKind::Hosted => Page::HostStatus,
        }
    }
}

/// The app's state: made once by the root and passed down to the shell and the pages (`Copy` handles).
#[derive(Clone, Copy)]
pub struct Store {
    platform: Platform,
    targets: Signal<Vec<Target>>,
    current: Signal<TargetId>,
    /// The page each target shows; a target without an entry shows its first page.
    pages: Signal<BTreeMap<TargetId, Page>>,
    /// Compact layouts: the pages pushed over the home page, bottom to top.
    stack: Signal<Vec<Page>>,
    language: Signal<LanguageChoice>,
    /// Wide layouts: the sidebar is shown.
    sidebar_open: Signal<bool>,
    /// Whether secrets survive a restart here (the keychain is reachable).
    secrets_persistent: bool,
    node: NodeStore,
    console: ConsoleStore,
    host: HostStore,
    versions: VersionStore,
}

impl Store {
    /// The state of a fresh start on `platform`, with the language choice and sidebar state the user kept; each
    /// role then restores what it kept (servers, running networks, the hosted control plane).
    pub fn new(platform: Platform) -> Self {
        let language = match Storage::get_item(LANGUAGE_KEY) {
            None => LanguageChoice::default(),
            Some(kept) => LanguageChoice::from_key(&kept).unwrap_or_else(|| {
                log::warn!("unknown language choice {kept:?} in storage; following the system");
                LanguageChoice::default()
            }),
        };
        let sidebar_open = Storage::get_item(SIDEBAR_KEY).as_deref() != Some(SIDEBAR_COLLAPSED);
        #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
        if let Some(dir) = Storage::get_item(TEST_DATA_DIR_KEY) {
            crate::core::paths::set_data_dir(std::path::PathBuf::from(dir));
        }
        let secrets = secret_store();
        let versions = VersionStore::new(platform);
        let env = StoreEnv {
            platform,
            secrets: secrets.clone(),
            programs: versions.programs(),
        };
        let store = Self {
            platform,
            targets: signal(Target::initial(platform)),
            current: signal(TargetId::PRIMARY),
            pages: signal(BTreeMap::new()),
            stack: signal(Vec::new()),
            language: signal(language),
            sidebar_open: signal(sidebar_open),
            secrets_persistent: secrets.persistent(),
            node: NodeStore::new(&env),
            console: ConsoleStore::new(&env),
            host: HostStore::new(&env),
            versions,
        };
        store.versions.restore(store);
        store.node.restore(store);
        store.console.restore(store);
        store.host.restore(store);
        store
    }

    /// The node role's state.
    pub fn node(&self) -> NodeStore {
        self.node
    }

    /// The console role's state.
    pub fn console(&self) -> ConsoleStore {
        self.console
    }

    /// The control-plane host role's state (hosted easytier-web, deployment files).
    pub fn host(&self) -> HostStore {
        self.host
    }

    /// The EasyTier programs on this desktop.
    pub fn versions(&self) -> VersionStore {
        self.versions
    }

    /// Whether secrets survive a restart on this device (the UI warns where they do not).
    pub fn secrets_persistent(&self) -> bool {
        self.secrets_persistent
    }

    /// Adds `target`; a target with its id already listed is replaced (restores and the hosted control plane
    /// reuse their ids).
    pub fn put_target(&self, target: Target) {
        self.targets.update(
            |targets| match targets.iter_mut().find(|t| t.id == target.id) {
                Some(existing) => *existing = target,
                None => targets.push(target),
            },
        );
    }

    /// Removes target `id` (never this device or the serving server); the selection falls back to the first.
    pub fn remove_target(&self, id: TargetId) {
        if id == TargetId::PRIMARY {
            log::warn!("the primary target cannot be removed");
            return;
        }
        batch(|| {
            self.targets
                .update(|targets| targets.retain(|t| t.id != id));
            self.pages.update(|pages| {
                pages.remove(&id);
            });
            if self.current.get_untracked() == id {
                self.current.set(TargetId::PRIMARY);
                self.stack.set(Vec::new());
            }
        });
    }

    /// Sets target `id`'s state as its backend reported it; an unknown id is ignored.
    pub fn set_target_status(&self, id: TargetId, status: TargetStatus) {
        let changed = self
            .targets
            .with_untracked(|targets| targets.iter().any(|t| t.id == id && t.status != status));
        if changed {
            self.targets.update(|targets| {
                if let Some(target) = targets.iter_mut().find(|t| t.id == id) {
                    target.status = status;
                }
            });
        }
    }

    /// Target `id` (tracked); `None` if it is not listed.
    pub fn target(&self, id: TargetId) -> Option<Target> {
        self.targets
            .with(|targets| targets.iter().find(|t| t.id == id).cloned())
    }

    /// The id a newly added server gets: past every listed id and past every id ever handed out, so a removed
    /// server's number never comes back (its kept session and settings can never be mistaken for the next one's).
    pub fn next_target_id(&self) -> Option<TargetId> {
        let high_water = Storage::get_item(ID_HIGH_WATER_KEY)
            .and_then(|kept| kept.parse::<u32>().ok())
            .unwrap_or(0);
        self.targets.with_untracked(|targets| {
            TargetId::next_after(targets.iter().map(|t| t.id), high_water)
        })
    }

    /// Remembers that `id` was handed out, so [`Self::next_target_id`] never returns it again.
    pub fn record_target_id(&self, id: TargetId) {
        let kept = Storage::get_item(ID_HIGH_WATER_KEY)
            .and_then(|kept| kept.parse::<u32>().ok())
            .unwrap_or(0);
        if id.number() <= kept {
            return;
        }
        if let Err(full) = Storage::set_item(ID_HIGH_WATER_KEY, id.number().to_string()) {
            log::warn!(
                "the highest server id was not kept: storage full at {} bytes",
                full.bytes
            );
        }
    }

    /// The selected target's id (tracked).
    pub fn current_target_id(&self) -> TargetId {
        self.current.get()
    }

    /// Wide layouts: whether the sidebar is shown (tracked).
    pub fn sidebar_open(&self) -> bool {
        self.sidebar_open.get()
    }

    /// Wide layouts: shows or hides the sidebar, and keeps the choice.
    pub fn toggle_sidebar(&self) {
        let open = !self.sidebar_open.get_untracked();
        self.sidebar_open.set(open);
        let kept = if open {
            Storage::remove_item(SIDEBAR_KEY);
            Ok(())
        } else {
            Storage::set_item(SIDEBAR_KEY, SIDEBAR_COLLAPSED)
        };
        if let Err(full) = kept {
            log::warn!(
                "sidebar state not kept: storage full at {} bytes",
                full.bytes
            );
        }
    }

    /// The platform the app runs on.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// What each role can do here.
    pub fn capabilities(&self) -> Capabilities {
        self.platform.capabilities()
    }

    /// Every target (tracked).
    pub fn targets(&self) -> Vec<Target> {
        self.targets.get()
    }

    /// The selected target (tracked); `None` only if it was removed.
    pub fn current_target(&self) -> Option<Target> {
        let id = self.current.get();
        self.targets
            .with(|targets| targets.iter().find(|t| t.id == id).cloned())
    }

    /// Selects target `id`; an id that is not in the list is ignored.
    pub fn select_target(&self, id: TargetId) {
        let known = self
            .targets
            .with_untracked(|targets| targets.iter().any(|t| t.id == id));
        if known {
            self.current.set(id);
        } else {
            log::warn!("no target {id:?} to select");
        }
    }

    /// The page the selected target shows (tracked).
    pub fn page(&self) -> Page {
        let id = self.current.get();
        let kept = self.pages.with(|pages| pages.get(&id).copied());
        kept.unwrap_or_else(|| {
            self.targets.with(|targets| {
                targets
                    .iter()
                    .find(|t| t.id == id)
                    .map_or(Page::Networks, |t| Page::first_of(t.kind))
            })
        })
    }

    /// Shows `page` for the selected target; compact layouts push it over the home page.
    pub fn open(&self, page: Page) {
        let id = self.current.get_untracked();
        batch(|| {
            self.pages.update(|pages| {
                pages.insert(id, page);
            });
            self.stack.set(vec![page]);
        });
    }

    /// Compact layouts: the pages over the home page (tracked).
    pub fn stack(&self) -> Vec<Page> {
        self.stack.get()
    }

    /// Compact layouts: the user left the page at `depth` (and those above it) natively.
    pub fn pop_to(&self, depth: usize) {
        self.stack.update(|stack| stack.truncate(depth));
    }

    /// The user's language choice (tracked).
    pub fn language(&self) -> LanguageChoice {
        self.language.get()
    }

    /// Switches the UI's language and keeps the choice.
    pub fn set_language(&self, choice: LanguageChoice) {
        self.language.set(choice);
        if let Err(full) = Storage::set_item(LANGUAGE_KEY, choice.as_key()) {
            log::warn!(
                "language choice not kept: storage full at {} bytes",
                full.bytes
            );
        }
    }

    /// The UI's strings in the chosen or the system's language (tracked).
    pub fn strings(&self) -> &'static Strings {
        self.language.get().resolve(locale().as_str()).strings()
    }

    /// A binding of the string `pick` chooses, following the language as it changes.
    pub fn tr(self, pick: Pick) -> impl Fn() -> &'static str + Copy + 'static {
        move || pick(self.strings())
    }
}
