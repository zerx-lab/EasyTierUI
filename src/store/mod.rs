//! App state: signals behind functions. Pages read through the getters (tracked where they are read) and change
//! state only through the functions here; each target keeps its own page, so switching targets loses nothing.
//!
//! Each role keeps its state in its own sub-store (`node`, `console`, `host`, `versions`), reached through
//! [`Store::node`] and friends, and the desktop app's own behavior (tray, login item) is `desktop`; the root keeps what
//! the shell needs: targets, pages, language, work mode, sidebar.

pub mod console;
pub mod desktop;
pub mod host;
pub mod node;
pub mod versions;

use std::{collections::BTreeMap, sync::Arc};

use nsh_core::{prelude::*, storage::Storage};

pub use self::{
    console::ConsoleStore, desktop::DesktopStore, host::HostStore, node::NodeStore,
    versions::VersionStore,
};
use crate::{
    core::{
        keychain::{SecretStore, system_store},
        node::ProgramSource,
        platform::{Capabilities, Platform},
        role::{Role, RoleSet, WorkMode},
        target::{Target, TargetId, TargetKind, TargetStatus},
    },
    i18n::{LanguageChoice, Pick, Strings},
};

/// The storage key of the user's language choice.
const LANGUAGE_KEY: &str = "ui.language";
/// The storage key of the user's work mode ([`WorkMode::as_key`]); absent until the first run's choice.
const MODE_KEY: &str = "ui.mode";
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

    /// The role whose section lists it; `None` for the settings, which every mode shows.
    pub fn role(self) -> Option<Role> {
        match self {
            Page::Networks | Page::Peers => Some(Role::Node),
            Page::Machines | Page::ServerNetworks | Page::Credentials => Some(Role::Console),
            Page::HostStatus | Page::Hardening | Page::Deployment => Some(Role::Host),
            Page::Settings => None,
        }
    }

    /// Whether the shell lists it under `roles`.
    fn listed_under(self, roles: RoleSet) -> bool {
        self.role().is_none_or(|role| roles.contains(role))
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

    /// The page a target opens on under `roles`: its node for this device, its console for a server, else the
    /// first page the shell lists.
    fn first_of(kind: TargetKind, roles: RoleSet) -> Page {
        let first = match kind {
            TargetKind::ThisDevice => Page::Networks,
            TargetKind::Server => Page::Machines,
            TargetKind::Hosted => Page::HostStatus,
        };
        if first.listed_under(roles) {
            return first;
        }
        roles
            .roles()
            .into_iter()
            .find_map(|role| Page::of_role(role).first().copied())
            .unwrap_or(Page::Settings)
    }
}

/// Whether the target switcher lists `target` under `roles`: servers serve the console, the hosted control plane the
/// host and the console; the primary target is always listed.
fn target_listed(target: &Target, roles: RoleSet) -> bool {
    target.id == TargetId::PRIMARY
        || match target.kind {
            TargetKind::ThisDevice => roles.contains(Role::Node),
            TargetKind::Server => roles.contains(Role::Console),
            TargetKind::Hosted => roles.contains(Role::Host) || roles.contains(Role::Console),
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
    /// The user's work mode; `None` until the first run's choice.
    mode: Signal<Option<WorkMode>>,
    /// Wide layouts: the sidebar is shown.
    sidebar_open: Signal<bool>,
    /// Whether secrets survive a restart here (the keychain is reachable).
    secrets_persistent: bool,
    node: NodeStore,
    console: ConsoleStore,
    host: HostStore,
    desktop: DesktopStore,
    versions: VersionStore,
}

impl Store {
    /// The state of a fresh start on `platform`, with the language choice, work mode and sidebar state the user
    /// kept; each role then restores what it kept (servers, running networks, the hosted control plane).
    pub fn new(platform: Platform) -> Self {
        let language = match Storage::get_item(LANGUAGE_KEY) {
            None => LanguageChoice::default(),
            Some(kept) => LanguageChoice::from_key(&kept).unwrap_or_else(|| {
                log::warn!("unknown language choice {kept:?} in storage; following the system");
                LanguageChoice::default()
            }),
        };
        let mode = match Storage::get_item(MODE_KEY) {
            None => WorkMode::implied_by(platform),
            Some(kept) => WorkMode::from_key(&kept).or_else(|| {
                log::warn!("unknown work mode {kept:?} in storage; asking again");
                WorkMode::implied_by(platform)
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
            mode: signal(mode),
            sidebar_open: signal(sidebar_open),
            secrets_persistent: secrets.persistent(),
            node: NodeStore::new(&env),
            console: ConsoleStore::new(&env),
            host: HostStore::new(&env),
            versions,
            desktop: DesktopStore::new(platform),
        };
        store.versions.restore(store);
        store.node.restore(store);
        store.console.restore(store);
        store.host.restore(store);
        store.desktop.start(store);
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

    /// The desktop app's own behavior: keeping to the tray, starting quietly, starting at login.
    pub fn desktop(&self) -> DesktopStore {
        self.desktop
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

    /// The page the selected target shows (tracked): the one opened last, else its first under the work mode.
    pub fn page(&self) -> Page {
        let id = self.current.get();
        let kept = self.pages.with(|pages| pages.get(&id).copied());
        kept.unwrap_or_else(|| {
            let roles = self.roles();
            self.targets.with(|targets| {
                let kind = targets
                    .iter()
                    .find(|t| t.id == id)
                    .map_or(TargetKind::ThisDevice, |t| t.kind);
                Page::first_of(kind, roles)
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

    /// The user's work mode (tracked); `None` until the first run's choice.
    pub fn mode(&self) -> Option<WorkMode> {
        self.mode.get()
    }

    /// The roles the shell shows (tracked); the node alone until the first run's choice.
    pub fn roles(&self) -> RoleSet {
        self.mode.get().map_or(RoleSet::NODE, WorkMode::roles)
    }

    /// Switches the work mode and keeps the choice. Nothing of a hidden role stops or is forgotten: its networks,
    /// servers and control plane keep running; only the pages it had open close, and a target the mode no longer
    /// lists hands the selection to the primary one.
    pub fn set_mode(&self, mode: WorkMode) {
        let roles = mode.roles();
        batch(|| {
            self.mode.set(Some(mode));
            self.pages
                .update(|pages| pages.retain(|_, page| page.listed_under(roles)));
            self.stack
                .update(|stack| stack.retain(|page| page.listed_under(roles)));
            let current = self.current.get_untracked();
            let listed = self.targets.with_untracked(|targets| {
                targets
                    .iter()
                    .any(|t| t.id == current && target_listed(t, roles))
            });
            if !listed {
                self.current.set(TargetId::PRIMARY);
                self.stack.set(Vec::new());
            }
        });
        if let Err(full) = Storage::set_item(MODE_KEY, mode.as_key()) {
            log::warn!("work mode not kept: storage full at {} bytes", full.bytes);
        }
    }

    /// The targets the switcher lists under the work mode (tracked).
    pub fn listed_targets(&self) -> Vec<Target> {
        let roles = self.roles();
        self.targets.with(|targets| {
            targets
                .iter()
                .filter(|t| target_listed(t, roles))
                .cloned()
                .collect()
        })
    }

    /// The hidden roles that still run something here (tracked): this device's networks for the node, the hosted
    /// easytier-web for the host. The shell keeps a way to them, so hiding a role never hides a running process.
    pub fn hidden_running(&self) -> Vec<Role> {
        let roles = self.roles();
        self.targets.with(|targets| hidden_running(roles, targets))
    }

    /// The UI's strings in the chosen or the system's language (tracked).
    pub fn strings(&self) -> &'static Strings {
        self.language
            .get()
            .resolve(&crate::i18n::system_language_tag())
            .strings()
    }

    /// A binding of the string `pick` chooses, following the language as it changes.
    pub fn tr(self, pick: Pick) -> impl Fn() -> &'static str + Copy + 'static {
        move || pick(self.strings())
    }
}

/// The roles outside `roles` that run something among `targets`: this device's networks for the node, the hosted
/// easytier-web for the host; a console's signed-in session runs nothing here.
fn hidden_running(roles: RoleSet, targets: &[Target]) -> Vec<Role> {
    Role::ALL
        .into_iter()
        .filter(|role| !roles.contains(*role))
        .filter(|role| {
            let kind = match role {
                Role::Node => TargetKind::ThisDevice,
                Role::Host => TargetKind::Hosted,
                Role::Console => return false,
            };
            targets.iter().any(|t| {
                t.kind == kind
                    && matches!(t.status, TargetStatus::Online | TargetStatus::Connecting)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::target::TargetOrigin;

    fn target(id: TargetId, kind: TargetKind, status: TargetStatus) -> Target {
        let origin = match kind {
            TargetKind::ThisDevice => TargetOrigin::Local,
            TargetKind::Server => TargetOrigin::Url("https://et.example.com".to_owned()),
            TargetKind::Hosted => TargetOrigin::Loopback { port: 11211 },
        };
        Target {
            id,
            kind,
            origin,
            name: None,
            status,
        }
    }

    fn server() -> Target {
        let id = TargetId::next_after([TargetId::PRIMARY], 0).unwrap();
        target(id, TargetKind::Server, TargetStatus::Online)
    }

    #[test]
    fn a_client_lists_only_this_device() {
        let device = target(
            TargetId::PRIMARY,
            TargetKind::ThisDevice,
            TargetStatus::Stopped,
        );
        let hosted = target(TargetId::HOSTED, TargetKind::Hosted, TargetStatus::Online);
        assert!(target_listed(&device, RoleSet::NODE));
        assert!(!target_listed(&server(), RoleSet::NODE));
        assert!(!target_listed(&hosted, RoleSet::NODE));
        // The hosted control plane serves the console as well as the host.
        let console = RoleSet::of([Role::Console]).unwrap();
        assert!(target_listed(&hosted, console));
        assert!(target_listed(&server(), console));
    }

    #[test]
    fn the_web_builds_server_stays_listed_whatever_the_mode() {
        let served = Target {
            origin: TargetOrigin::SameOrigin,
            ..target(
                TargetId::PRIMARY,
                TargetKind::Server,
                TargetStatus::NotConnected,
            )
        };
        assert!(target_listed(&served, RoleSet::NODE));
    }

    #[test]
    fn a_target_opens_on_a_page_its_mode_lists() {
        assert_eq!(
            Page::first_of(TargetKind::ThisDevice, RoleSet::NODE),
            Page::Networks
        );
        assert_eq!(
            Page::first_of(TargetKind::Server, RoleSet::ALL),
            Page::Machines
        );
        // The web build's server under a node-only mode opens on the node, which offers the server's nodes.
        assert_eq!(
            Page::first_of(TargetKind::Server, RoleSet::NODE),
            Page::Networks
        );
        let host = RoleSet::of([Role::Host]).unwrap();
        assert_eq!(
            Page::first_of(TargetKind::ThisDevice, host),
            Page::HostStatus
        );
    }

    #[test]
    fn a_hidden_role_that_runs_something_stays_reachable() {
        let running = [
            target(
                TargetId::PRIMARY,
                TargetKind::ThisDevice,
                TargetStatus::Online,
            ),
            target(
                TargetId::HOSTED,
                TargetKind::Hosted,
                TargetStatus::Connecting,
            ),
            server(),
        ];
        assert_eq!(hidden_running(RoleSet::ALL, &running), vec![]);
        assert_eq!(hidden_running(RoleSet::NODE, &running), vec![Role::Host]);
        let console = RoleSet::of([Role::Console]).unwrap();
        assert_eq!(
            hidden_running(console, &running),
            vec![Role::Node, Role::Host]
        );

        let stopped = [
            target(
                TargetId::PRIMARY,
                TargetKind::ThisDevice,
                TargetStatus::Stopped,
            ),
            target(TargetId::HOSTED, TargetKind::Hosted, TargetStatus::Failed),
        ];
        assert_eq!(hidden_running(console, &stopped), vec![]);
    }
}
