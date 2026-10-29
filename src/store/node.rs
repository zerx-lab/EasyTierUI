//! The node role's state: this device's network profiles, each instance's state as its backend reports it, peers and
//! logs, and the drafts of the dialogs that create, join, import, edit and enroll networks.
//!
//! Backends: `sidecar` on the desktops, `embedded` on iOS / Android, none in the web build (its Node pages show a
//! Capability Notice). Every call into a backend runs off the engine thread (`core::work::off_thread`) inside an NSH
//! task; its answer carries the revision it was asked at and is dropped when the network moved on since.
//!
//! The split: this file holds the state, what pages read, and the polling that keeps it true; `lifecycle` starts,
//! stops and deletes; `forms` runs the dialogs; `flow`, `editor` and `import` are the drafts (pure, tested);
//! `problem` turns validation errors into data the pages word in the user's language.

mod browse;
mod charts;
mod editor;
mod flow;
mod forms;
mod history;
mod import;
mod lifecycle;
mod problem;
#[cfg(debug_assertions)]
pub mod testing;
mod views;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use nsh_core::prelude::*;

use self::views::Runtime;
pub use self::{
    browse::{NetworkFilter, PeerMix, PeerRef, PeerRow, PeerStats, RouteFilter},
    charts::{
        Aligned, LATENCY_LINES, LatencyLine, LatencyLines, ThroughputSeries, TimedSamples, align,
    },
    editor::{DetailTab, EditorDraft, IpChoice},
    flow::{FlowDraft, FlowKind, FlowStep, parse_peer, parse_peer_at},
    history::{
        HISTORY_CAPACITY, HISTORY_SPAN_MS, MAX_GAP_MS, PeerCountsPoint, PeerRates, PeerSeries,
        RatePoint, SLOT_MS, Totals,
    },
    import::{ImportDraft, ImportStage, ManagedDraft, parse_import},
    problem::{Field, FieldProblem, ImportFailure, ImportReason, ProblemKind, SaveFailure},
    views::{CoreStatus, ExportFailure, ExportView, Failure, LogView, Notice, Op, Status},
};
use super::{Store, StoreEnv};
use crate::{
    core::{
        keychain::SecretStore,
        node::{NodeBackend, NodeError, ProgramSource},
        state::{FailReason, NetworkState, NodeSnapshot},
        target::{TargetId, TargetStatus},
    },
    profile::{NetworkProfile, NshStorage, ProfileId, RepoError, repo},
};

/// How often the instances that are up (or failed) are read: every second, so rates and peers follow the traffic
/// (a read is two `easytier-cli` runs of about 25 ms on the desktops, an in-process call on mobile).
const POLL_EVERY: Duration = Duration::from_secs(1);
/// The core is looked for again every this many polls while it is ready (a release may be switched in Settings);
/// while it is missing or too old it is looked for at every poll.
const CORE_CHECK_EVERY: u32 = 10;

/// See the module docs. A `Copy` handle of signals, like [`Store`].
#[derive(Clone, Copy)]
pub struct NodeStore {
    platform: crate::core::platform::Platform,
    /// This platform runs the node role itself (not the web build).
    local: bool,
    /// The owner tasks run under: the app's, so a button re-rendered by the change it started, or a closed dialog,
    /// never cancels the work (a cancelled stop left its network "stopping" forever).
    owner: Owner,
    /// Goes false when the owner is disposed; tasks that outlive it check it before touching a signal.
    alive: Signal<bool>,
    backend: Stored<Option<Arc<dyn NodeBackend>>>,
    programs: Stored<Option<Arc<dyn ProgramSource>>>,
    secrets: Stored<Arc<dyn SecretStore>>,
    root: Stored<Option<Store>>,
    profiles: Signal<Vec<NetworkProfile>>,
    runtime: Signal<BTreeMap<ProfileId, Runtime>>,
    notice: Signal<Option<Notice>>,
    core: Signal<CoreStatus>,
    /// The network whose dialog is open.
    detail: Signal<Option<ProfileId>>,
    tab: Signal<DetailTab>,
    /// The network a first start is waiting on the person's agreement for (administrator prompt, service).
    tun_confirm: Signal<Option<ProfileId>>,
    flow: Signal<Option<FlowDraft>>,
    editor: Signal<Option<EditorDraft>>,
    import: Signal<Option<ImportDraft>>,
    managed: Signal<Option<ManagedDraft>>,
    logs: Signal<LogView>,
    export: Signal<ExportView>,
    /// Networks with a poll in flight.
    polling: Stored<BTreeSet<ProfileId>>,
    checking_core: Stored<bool>,
    ticks: Stored<u32>,
    /// The last few minutes of each running network's polls, for the charts (see `history`).
    history: Stored<BTreeMap<ProfileId, history::History>>,
    /// Bumped when `history` changed; readers of the history track it.
    history_version: Signal<u64>,
    /// The Networks toolbar's search text and filter.
    networks_query: Signal<String>,
    networks_filter: Signal<NetworkFilter>,
    /// The Peers toolbar: the network it is narrowed to, the search text and the route filter.
    peers_network: Signal<Option<ProfileId>>,
    peers_query: Signal<String>,
    peers_route: Signal<RouteFilter>,
    /// The peer whose drawer is open.
    peer_open: Signal<Option<PeerRef>>,
    /// Whether the log tab re-reads the output at every poll.
    logs_follow: Signal<bool>,
}

impl NodeStore {
    /// Its state at start.
    pub(super) fn new(env: &StoreEnv) -> Self {
        let local = env.platform.capabilities().node.is_local();
        let backend = if local { make_backend(env) } else { None };
        Self {
            platform: env.platform,
            local,
            owner: Owner::current(),
            alive: signal(true),
            backend: stored(backend),
            programs: stored(env.programs.clone()),
            secrets: stored(env.secrets.clone()),
            root: stored(None),
            profiles: signal(Vec::new()),
            runtime: signal(BTreeMap::new()),
            notice: signal(None),
            core: signal(CoreStatus::Checking),
            detail: signal(None),
            tab: signal(DetailTab::Overview),
            tun_confirm: signal(None),
            flow: signal(None),
            editor: signal(None),
            import: signal(None),
            managed: signal(None),
            logs: signal(LogView::default()),
            export: signal(ExportView::default()),
            polling: stored(BTreeSet::new()),
            checking_core: stored(false),
            ticks: stored(0),
            history: stored(BTreeMap::new()),
            history_version: signal(0),
            networks_query: signal(String::new()),
            networks_filter: signal(NetworkFilter::All),
            peers_network: signal(None),
            peers_query: signal(String::new()),
            peers_route: signal(RouteFilter::All),
            peer_open: signal(None),
            logs_follow: signal(false),
        }
    }

    /// Restores what the role kept across runs, once the root store exists: the profiles, one read of each instance
    /// (an installed OS service may already run), the polling, and the stop of this app's child processes at quit.
    pub(super) fn restore(&self, store: Store) {
        if !self.local {
            return;
        }
        let _previous = self.root.replace(Some(store));
        match repo::load_profiles(&NshStorage) {
            Ok(loaded) => {
                if loaded.skipped > 0 {
                    self.notice.set(Some(Notice::Skipped(loaded.skipped)));
                }
                self.runtime.update(|map| {
                    for profile in &loaded.profiles {
                        map.entry(profile.id.clone()).or_default();
                    }
                });
                self.profiles.set(loaded.profiles);
            }
            Err(RepoError::NewerSchema { found }) => {
                log::warn!("the stored profiles use schema {found}, newer than this app's");
                self.notice.set(Some(Notice::NewerSchema));
            }
            Err(error) => {
                log::error!("the stored profiles could not be read: {error}");
                self.notice.set(Some(Notice::Damaged));
            }
        }
        self.refresh_core();
        for id in self.ids_untracked() {
            self.poll_one(id);
        }
        let this = *self;
        let _poller = set_interval(POLL_EVERY, move || this.tick());
        if let Some(backend) = self.backend.get() {
            // Child processes never outlive the app; OS services (TUN networks) are the user's and keep running.
            cleanup(move || backend.shutdown());
        }
        self.sync_target();
    }

    // ----- what pages read -----

    /// Whether this device runs the node role (false in the web build).
    pub fn is_local(&self) -> bool {
        self.local
    }

    /// Whether a TUN network interface can be created here (desktops; the mobile builds run without one).
    pub fn tun_supported(&self) -> bool {
        self.platform.is_desktop()
    }

    /// The ids of the networks, in the order they were made (tracked).
    pub fn ids(&self) -> Vec<ProfileId> {
        self.profiles
            .with(|profiles| profiles.iter().map(|p| p.id.clone()).collect())
    }

    fn ids_untracked(&self) -> Vec<ProfileId> {
        self.profiles
            .with_untracked(|profiles| profiles.iter().map(|p| p.id.clone()).collect())
    }

    /// The profile `id` (tracked).
    pub fn profile(&self, id: &ProfileId) -> Option<NetworkProfile> {
        self.profiles
            .with(|profiles| profiles.iter().find(|p| &p.id == id).cloned())
    }

    fn profile_untracked(&self, id: &ProfileId) -> Option<NetworkProfile> {
        self.profiles
            .with_untracked(|profiles| profiles.iter().find(|p| &p.id == id).cloned())
    }

    /// How network `id` stands (tracked).
    pub fn status(&self, id: &ProfileId) -> Status {
        self.runtime
            .with(|map| map.get(id).map_or(Status::Stopped, Runtime::status))
    }

    fn status_untracked(&self, id: &ProfileId) -> Status {
        self.runtime
            .with_untracked(|map| map.get(id).map_or(Status::Stopped, Runtime::status))
    }

    /// The core's last reading of network `id` while it runs (tracked).
    pub fn snapshot(&self, id: &ProfileId) -> Option<NodeSnapshot> {
        self.runtime.with(|map| {
            map.get(id)
                .and_then(|runtime| runtime.state.snapshot().cloned())
        })
    }

    /// Why the last start of `id` failed, also while an older instance still runs (tracked).
    pub fn start_failure(&self, id: &ProfileId) -> Option<Failure> {
        self.runtime
            .with(|map| map.get(id).and_then(|runtime| runtime.failure.clone()))
    }

    /// Why the last stop of `id` failed (tracked).
    pub fn stop_error(&self, id: &ProfileId) -> Option<FailReason> {
        self.runtime
            .with(|map| map.get(id).and_then(|runtime| runtime.stop_error.clone()))
    }

    /// Whether an action on `id` is in flight (tracked).
    pub fn is_busy(&self, id: &ProfileId) -> bool {
        self.runtime
            .with(|map| map.get(id).is_some_and(|runtime| runtime.op.is_some()))
    }

    /// The networks that are not stopped (running, starting, stopping, failed), in list order (tracked).
    pub fn active_ids(&self) -> Vec<ProfileId> {
        let ids = self.ids();
        self.runtime.with(|map| {
            ids.into_iter()
                .filter(|id| {
                    map.get(id)
                        .is_some_and(|runtime| runtime.status() != Status::Stopped)
                })
                .collect()
        })
    }

    /// Whether any network runs (tracked).
    pub fn any_running(&self) -> bool {
        self.runtime.with(|map| {
            map.values()
                .any(|runtime| matches!(runtime.state, NetworkState::Running(_)))
        })
    }

    /// Whether the EasyTier core can run networks here (tracked).
    pub fn core_status(&self) -> CoreStatus {
        self.core.get()
    }

    /// The message above the list, if any (tracked).
    pub fn notice(&self) -> Option<Notice> {
        self.notice.get()
    }

    /// Hides the message above the list.
    pub fn dismiss_notice(&self) {
        if self.alive() {
            self.notice.set(None);
        }
    }

    /// The network whose dialog is open (tracked).
    pub fn detail(&self) -> Option<ProfileId> {
        self.detail.get()
    }

    /// The tab of the open network dialog (tracked).
    pub fn tab(&self) -> DetailTab {
        self.tab.get()
    }

    /// The network waiting for the person to agree to the administrator prompt (tracked).
    pub fn tun_confirm(&self) -> Option<ProfileId> {
        self.tun_confirm.get()
    }

    /// The create / join flow's draft (tracked).
    pub fn flow(&self) -> Signal<Option<FlowDraft>> {
        self.flow
    }

    /// The network editor's draft (tracked).
    pub fn editor(&self) -> Signal<Option<EditorDraft>> {
        self.editor
    }

    /// The import dialog's draft (tracked).
    pub fn import(&self) -> Signal<Option<ImportDraft>> {
        self.import
    }

    /// The managed-enrollment dialog's draft (tracked).
    pub fn managed(&self) -> Signal<Option<ManagedDraft>> {
        self.managed
    }

    /// The log tab's lines (tracked).
    pub fn logs(&self) -> Signal<LogView> {
        self.logs
    }

    /// The export tab's content (tracked).
    pub fn export(&self) -> Signal<ExportView> {
        self.export
    }

    // ----- keeping the state true -----

    /// Whether the app's owner is still there (tasks check it before writing signals).
    fn alive(&self) -> bool {
        self.alive.try_get().is_some()
    }

    /// Runs `work` as a task under the store's owner.
    fn go(&self, work: impl Future<Output = ()> + 'static) {
        self.owner.run(|| {
            let _task = spawn(work);
        });
    }

    /// Changes network `id`'s runtime record by `f`; writes (and refreshes the target's lamp) only when it changed.
    fn change(&self, id: &ProfileId, f: impl FnOnce(&mut Runtime)) {
        if !self.alive() {
            return;
        }
        let Some(old) = self.runtime.with_untracked(|map| map.get(id).cloned()) else {
            return;
        };
        let mut new = old.clone();
        f(&mut new);
        if new == old {
            return;
        }
        self.track_history(id, &old, &new);
        self.runtime.update(|map| {
            map.insert(id.clone(), new);
        });
        self.sync_target();
    }

    /// The revision of `id` now (0 for an unknown network).
    fn revision_of(&self, id: &ProfileId) -> u64 {
        self.runtime
            .with_untracked(|map| map.get(id).map_or(0, |runtime| runtime.revision))
    }

    /// Keeps the target switcher's lamp for this device true: online while any network runs.
    fn sync_target(&self) {
        if !self.local || !self.alive() {
            return;
        }
        let Some(store) = self.root.get() else {
            return;
        };
        let status = if self.runtime.with_untracked(|map| {
            map.values()
                .any(|runtime| matches!(runtime.state, NetworkState::Running(_)))
        }) {
            TargetStatus::Online
        } else {
            TargetStatus::Stopped
        };
        store.set_target_status(TargetId::PRIMARY, status);
    }

    /// One tick of the poller: read what is up or failed, and look for the core again when it is not ready.
    fn tick(&self) {
        if !self.alive() || !self.local {
            return;
        }
        let ticks = self.ticks.with(|ticks| {
            *ticks = ticks.wrapping_add(1);
            *ticks
        });
        let core_ready = matches!(self.core.get_untracked(), CoreStatus::Ready(_));
        if !core_ready || ticks.checked_rem(CORE_CHECK_EVERY) == Some(0) {
            self.refresh_core();
        }
        let due: Vec<ProfileId> = self.runtime.with_untracked(|map| {
            map.iter()
                .filter(|(_, runtime)| {
                    runtime.op.is_none()
                        && (runtime.state.is_active()
                            || matches!(runtime.state, NetworkState::Failed { .. }))
                })
                .map(|(id, _)| id.clone())
                .collect()
        });
        for id in due {
            self.poll_one(id);
        }
        if self.logs_follow.get_untracked()
            && self.tab.get_untracked() == DetailTab::Logs
            && let Some(id) = self.detail.get_untracked()
        {
            self.load_logs(id);
        }
    }

    /// Reads instance `id` off the engine thread and applies the answer unless the network moved on meanwhile.
    fn poll_one(&self, id: ProfileId) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        let Ok(key) = id.instance_key() else {
            log::warn!("profile {id} has no instance key");
            return;
        };
        if !self.polling.with(|set| set.insert(id.clone())) {
            return;
        }
        let revision = self.revision_of(&id);
        let this = *self;
        self.go(async move {
            let answer = crate::core::work::off_thread(move || backend.poll(&key)).await;
            if !this.alive() {
                return;
            }
            this.polling.with(|set| {
                set.remove(&id);
            });
            match answer {
                Ok(state) => this.change(&id, |runtime| {
                    if runtime.revision != revision || runtime.op.is_some() {
                        return;
                    }
                    if matches!(state, NetworkState::Running(_) | NetworkState::Starting) {
                        runtime.failure = None;
                    }
                    runtime.state = state;
                }),
                Err(error) => log::warn!("reading {id} failed: {error}"),
            }
        });
    }

    /// Asks which release is active and whether the platform needs one, and publishes the answer.
    fn refresh_core(&self) {
        if !self.local || !self.alive() {
            return;
        }
        let Some(programs) = self.programs.get() else {
            let status = if self.platform.is_desktop() {
                CoreStatus::Missing
            } else {
                CoreStatus::NotNeeded
            };
            self.core.set(status);
            return;
        };
        if self
            .checking_core
            .with(|busy| std::mem::replace(busy, true))
        {
            return;
        }
        let this = *self;
        self.go(async move {
            let answer = crate::core::work::off_thread(move || programs.active()).await;
            if !this.alive() {
                return;
            }
            this.checking_core.with(|busy| *busy = false);
            let status = match answer {
                Ok(Ok(found)) => CoreStatus::Ready(found.version),
                Ok(Err(NodeError::CoreUnsupported(version))) => CoreStatus::Unsupported(version),
                Ok(Err(NodeError::CoreMissing)) => CoreStatus::Missing,
                Ok(Err(error)) => {
                    log::warn!("the active EasyTier release could not be used: {error}");
                    CoreStatus::Missing
                }
                Err(error) => {
                    log::warn!("looking for the EasyTier core failed: {error}");
                    return;
                }
            };
            this.core.set(status);
        });
    }
}

/// The backend this build uses on this platform; `None` where the node role has none (the web build, or a desktop
/// whose programs are not wired up).
fn make_backend(env: &StoreEnv) -> Option<Arc<dyn NodeBackend>> {
    #[cfg(debug_assertions)]
    if let Some(backend) = testing::from_storage() {
        return Some(backend);
    }
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        let programs = env.programs.clone()?;
        let backend = crate::core::sidecar::SidecarBackend::new(programs)
            .with_elevation_prompt(startup_strings().node.elevation_prompt);
        Some(Arc::new(backend))
    }
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        let _ = env;
        Some(Arc::new(crate::core::embedded::EmbeddedBackend::new()))
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux",
        target_os = "ios",
        target_os = "android"
    )))]
    {
        let _ = env;
        None
    }
}

/// The strings in the language the app starts in (the OS prompt is worded once, at start).
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn startup_strings() -> &'static crate::i18n::Strings {
    use nsh_core::storage::Storage;

    use crate::i18n::LanguageChoice;

    let choice = Storage::get_item(super::LANGUAGE_KEY)
        .and_then(|kept| LanguageChoice::from_key(&kept))
        .unwrap_or_default();
    choice
        .resolve(&crate::i18n::system_language_tag())
        .strings()
}
