//! App state: signals behind functions. Pages read through the getters (tracked where they are read) and change
//! state only through the functions here; each target keeps its own page, so switching targets loses nothing.

use std::collections::BTreeMap;

use nsh_core::{prelude::*, storage::Storage};

use crate::{
    core::{
        platform::{Capabilities, Platform},
        role::Role,
        target::{Target, TargetId, TargetKind},
    },
    i18n::{LanguageChoice, Pick, Strings},
};

/// The storage key of the user's language choice.
const LANGUAGE_KEY: &str = "ui.language";
/// The storage key of the sidebar's state, set to [`SIDEBAR_COLLAPSED`] while the user keeps it hidden.
const SIDEBAR_KEY: &str = "ui.sidebar";
const SIDEBAR_COLLAPSED: &str = "collapsed";

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
}

impl Store {
    /// The state of a fresh start on `platform`, with the language choice and sidebar state the user kept.
    pub fn new(platform: Platform) -> Self {
        let language = match Storage::get_item(LANGUAGE_KEY) {
            None => LanguageChoice::default(),
            Some(kept) => LanguageChoice::from_key(&kept).unwrap_or_else(|| {
                log::warn!("unknown language choice {kept:?} in storage; following the system");
                LanguageChoice::default()
            }),
        };
        let sidebar_open = Storage::get_item(SIDEBAR_KEY).as_deref() != Some(SIDEBAR_COLLAPSED);
        Self {
            platform,
            targets: signal(Target::initial(platform)),
            current: signal(TargetId::PRIMARY),
            pages: signal(BTreeMap::new()),
            stack: signal(Vec::new()),
            language: signal(language),
            sidebar_open: signal(sidebar_open),
        }
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
