//! The EasyTier programs on this desktop: the detected and installed releases, the active one, downloads in
//! progress. Elsewhere (mobile, web) there is nothing to manage and [`VersionStore::programs`] is `None`.
//!
//! The pages see plain domain types ([`InstalledRelease`], [`AvailableRelease`], [`VersionProblem`], ...) that exist
//! on every platform; the desktops' installer (`core::install`) is only named inside [`backend`].
//!
//! Switching the active release is a sequence the store owns: stop the running networks, switch, restart the hosted
//! control plane when it ran, start the networks again. A release is installed only after the user confirmed it, and
//! a release found on disk is adopted only after a confirmation that shows its version and path.

use std::{future::Future, sync::Arc};

use nsh_core::prelude::*;

use self::backend::Backend;
use super::Store;
use crate::{
    components::confirm,
    core::{node::ProgramSource, platform::Platform, version::MIN_CORE_VERSION, work::off_thread},
    i18n::{Pick, fill},
};

/// Why managing releases failed, in cases the page words (and an optional machine detail shown beside the words).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionProblem {
    /// GitHub could not be reached.
    Offline,
    /// GitHub's anonymous rate limit was reached.
    RateLimited,
    /// GitHub answered with an unexpected status.
    Http(u16),
    /// GitHub's answer could not be read.
    BadResponse,
    /// The release has no download for this device.
    NoDownload,
    /// The release publishes no sha256 digest.
    MissingDigest,
    /// The download is larger than the app accepts.
    TooLarge,
    /// The download's size differs from the listing.
    SizeMismatch,
    /// The download's sha256 differs from the release's digest.
    DigestMismatch,
    /// The release (version in the text) is older than the app supports.
    Unsupported(String),
    /// The archive is damaged.
    BadArchive,
    /// The archive holds an entry that is not allowed.
    UnsafeArchive,
    /// A program (its name in the text) is missing.
    MissingProgram(String),
    /// A program (its name in the text) reported no version.
    VersionUnreadable(String),
    /// The downloaded core is not the requested release.
    VersionMismatch,
    /// The release (version in the text) is already installed.
    AlreadyInstalled(String),
    /// The release (version in the text) is not installed.
    NotInstalled(String),
    /// The active release cannot be removed.
    ActiveInUse,
    /// There is no previous release.
    NoPrevious,
    /// No core on the PATH.
    NotOnPath,
    /// The chosen directory is not an absolute path.
    NotAbsolute,
    /// A file operation failed (the OS message in the text).
    Disk(String),
    /// The data directory is unusable (the OS message in the text).
    DataDir(String),
    /// The background task ended without a result.
    Interrupted,
    /// This platform has no programs to manage.
    Unavailable,
}

impl VersionProblem {
    /// The words for the problem.
    pub fn message(&self) -> Pick {
        match self {
            Self::Offline => |s| s.versions.problem_offline,
            Self::RateLimited => |s| s.versions.problem_rate_limited,
            Self::Http(_) => |s| s.versions.problem_http,
            Self::BadResponse => |s| s.versions.problem_bad_response,
            Self::NoDownload => |s| s.versions.problem_no_download,
            Self::MissingDigest => |s| s.versions.problem_missing_digest,
            Self::TooLarge => |s| s.versions.problem_too_large,
            Self::SizeMismatch => |s| s.versions.problem_size_mismatch,
            Self::DigestMismatch => |s| s.versions.problem_digest_mismatch,
            Self::Unsupported(_) => |s| s.versions.problem_unsupported,
            Self::BadArchive => |s| s.versions.problem_bad_archive,
            Self::UnsafeArchive => |s| s.versions.problem_unsafe_archive,
            Self::MissingProgram(_) => |s| s.versions.problem_missing_program,
            Self::VersionUnreadable(_) => |s| s.versions.problem_version_unreadable,
            Self::VersionMismatch => |s| s.versions.problem_version_mismatch,
            Self::AlreadyInstalled(_) => |s| s.versions.problem_already_installed,
            Self::NotInstalled(_) => |s| s.versions.problem_not_installed,
            Self::ActiveInUse => |s| s.versions.problem_active_in_use,
            Self::NoPrevious => |s| s.versions.problem_no_previous,
            Self::NotOnPath => |s| s.versions.problem_not_on_path,
            Self::NotAbsolute => |s| s.versions.problem_not_absolute,
            Self::Disk(_) => |s| s.versions.problem_disk,
            Self::DataDir(_) => |s| s.versions.problem_data_dir,
            Self::Interrupted => |s| s.versions.problem_interrupted,
            Self::Unavailable => |s| s.versions.problem_unavailable,
        }
    }

    /// The machine data that goes with the words (a status, a version, a program, an OS message), shown apart
    /// from them in the monospace.
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Http(status) => Some(format!("HTTP {status}")),
            Self::Unsupported(text)
            | Self::MissingProgram(text)
            | Self::VersionUnreadable(text)
            | Self::AlreadyInstalled(text)
            | Self::NotInstalled(text)
            | Self::Disk(text)
            | Self::DataDir(text) => Some(text.clone()),
            _ => None,
        }
    }
}

/// The release the next start of a node or the control plane runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveRelease {
    /// Being read.
    Checking,
    /// A usable release (its version, with the commit when the program reports one).
    Ready {
        /// Version text.
        version: String,
    },
    /// Nothing is installed.
    Missing,
    /// The release is older than the app supports.
    Unsupported {
        /// Version text.
        version: String,
    },
    /// It could not be read.
    Failed(VersionProblem),
    /// This platform runs no programs of its own (mobile: the library is in the app; web: the server's).
    Unmanaged,
}

impl ActiveRelease {
    /// Whether programs can be started from it.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }
}

/// A release installed on this device, or a directory the user chose that is in use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRelease {
    /// Its version text (with the commit when the program reports one).
    pub version: String,
    /// The version number without the commit: the key `activate` and `remove` take.
    pub number: String,
    /// Where its programs are.
    pub dir: String,
    /// The next start runs this one.
    pub active: bool,
    /// Rolling back returns to this one.
    pub previous: bool,
    /// A directory the user chose, not one the app installed.
    pub custom: bool,
    /// Its programs all exist, run and are supported.
    pub usable: bool,
}

/// Why a listed release cannot be installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseBlock {
    /// Older than the app supports.
    TooOld,
    /// No download for this device.
    NoDownload,
    /// No sha256 digest to verify it with.
    NoDigest,
}

/// A release GitHub lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableRelease {
    /// The git tag (`v2.6.4`): the key `install` takes.
    pub tag: String,
    /// The version text.
    pub version: String,
    /// The version number without the commit, to match [`InstalledRelease::number`].
    pub number: String,
    /// A pre-release.
    pub prerelease: bool,
    /// The publication date (`2026-05-12`), empty when unknown.
    pub published: String,
    /// The download's size in bytes, if the release has one for this device.
    pub size: Option<u64>,
    /// The download lists a sha256 digest the app checks.
    pub verified: bool,
    /// Why it cannot be installed, if it cannot.
    pub block: Option<ReleaseBlock>,
}

/// The release list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseList {
    /// Not asked for yet.
    NotLoaded,
    /// On its way.
    Loading,
    /// The listing, newest first.
    Ready(Vec<AvailableRelease>),
    /// It could not be fetched.
    Failed(VersionProblem),
}

/// A release found on disk, shown before the user decides to use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Its version text.
    pub version: String,
    /// The directory holding its programs.
    pub dir: String,
    /// New enough for the app.
    pub supported: bool,
}

/// The search for a release on the PATH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    /// Not looked yet.
    Unchecked,
    /// Looking.
    Searching,
    /// Nothing on the PATH.
    NotFound,
    /// A release is there.
    Found(Candidate),
    /// What is there could not be read.
    Failed(VersionProblem),
}

/// The check of a directory the user typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirCheck {
    /// Nothing checked.
    Idle,
    /// Checking.
    Checking,
    /// A release is there.
    Found(Candidate),
    /// The directory is not usable.
    Failed(VersionProblem),
}

/// Where an install is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    /// Starting.
    Starting,
    /// Fetching the zip.
    Downloading,
    /// Checking the sha256.
    Verifying,
    /// Unpacking.
    Unpacking,
}

/// The install in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallProgress {
    /// The release's tag.
    pub tag: String,
    /// Where it is.
    pub step: InstallStep,
}

/// A change of the active release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Change {
    /// Run the installed release with this version number.
    Activate(String),
    /// Return to the previous release.
    Rollback,
    /// Run the programs in this directory.
    Adopt(String),
}

/// `bytes` in MiB with one decimal (`28.8 MiB`).
pub fn format_size(bytes: u64) -> String {
    // One tenth of a MiB is 104 857.6 bytes.
    let tenths = bytes / 104_858;
    format!("{}.{} MiB", tenths / 10, tenths % 10)
}

/// How the active release stands to the newest stable release GitHub lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The newest release is newer than the active one and can be installed.
    Update,
    /// The active release is the newest, or newer.
    UpToDate,
    /// Nothing to compare (no active release), or the newer release cannot be installed on this device.
    Unrelated,
}

/// The newest stable release GitHub lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Latest {
    /// The listing's entry.
    pub release: AvailableRelease,
    /// How the active release stands to it.
    pub standing: Standing,
}

/// The numeric parts of a version number (`2.6.4` → `[2, 6, 4]`, a suffix such as `-rc1` ignored), without
/// trailing zeros, so `2.6` and `2.6.0` compare equal.
fn number_key(number: &str) -> Vec<u64> {
    let mut parts: Vec<u64> = number
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect();
    while parts.last() == Some(&0) {
        parts.pop();
    }
    parts
}

/// Whether version number `candidate` is newer than `base`.
fn is_newer(candidate: &str, base: &str) -> bool {
    number_key(candidate) > number_key(base)
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn version_numbers_compare_by_their_numeric_parts() {
        assert!(is_newer("2.6.4", "2.6.3"));
        assert!(is_newer("2.10.0", "2.9.9"));
        assert!(is_newer("3.0.0", "2.99.99"));
        assert!(!is_newer("2.6.4", "2.6.4"));
        assert!(!is_newer("2.6", "2.6.0"));
        assert!(!is_newer("2.5.9", "2.6.0"));
        assert!(is_newer("2.6.1-rc1", "2.6.0"));
    }
}

/// The installer on the desktops; an empty type elsewhere, so a build without one has no `Some` to find.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod backend {
    use std::{
        path::Path,
        sync::{Arc, Mutex, MutexGuard, PoisonError},
    };

    use super::{
        ActiveRelease, AvailableRelease, Candidate, Detection, DirCheck, InstallStep,
        InstalledRelease, ReleaseBlock, VersionProblem,
    };
    use crate::core::{
        install::{self, InstallError, InstallPhase, Installations, Release},
        node::{NodeError, ProgramSource},
        version::CoreVersion,
    };

    /// The releases of this desktop and the last GitHub listing.
    pub(super) struct Backend {
        installations: Installations,
        listed: Mutex<Vec<Release>>,
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        // The listing is replaced whole, never half-updated, so a poisoned lock is still consistent.
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn problem(error: InstallError) -> VersionProblem {
        match error {
            InstallError::DataDir(message) => VersionProblem::DataDir(message),
            InstallError::Io(message) => VersionProblem::Disk(message),
            InstallError::Interrupted(message) => {
                log::warn!("a background task of the installer ended: {message}");
                VersionProblem::Interrupted
            }
            InstallError::Network(message) => {
                log::warn!("GitHub could not be reached: {message}");
                VersionProblem::Offline
            }
            InstallError::RateLimited => VersionProblem::RateLimited,
            InstallError::Http(status) => VersionProblem::Http(status),
            InstallError::BadResponse(_) => VersionProblem::BadResponse,
            InstallError::NoAsset => VersionProblem::NoDownload,
            InstallError::MissingDigest => VersionProblem::MissingDigest,
            InstallError::TooLarge(_) => VersionProblem::TooLarge,
            InstallError::SizeMismatch { .. } => VersionProblem::SizeMismatch,
            InstallError::DigestMismatch => VersionProblem::DigestMismatch,
            InstallError::Unsupported(version) => VersionProblem::Unsupported(version.to_string()),
            InstallError::BadArchive(_) => VersionProblem::BadArchive,
            InstallError::BadEntry(_) => VersionProblem::UnsafeArchive,
            InstallError::MissingProgram(name) => VersionProblem::MissingProgram(name),
            InstallError::VersionUnreadable { program, .. } => {
                VersionProblem::VersionUnreadable(program)
            }
            InstallError::VersionMismatch { .. } => VersionProblem::VersionMismatch,
            InstallError::AlreadyInstalled(version) => VersionProblem::AlreadyInstalled(version),
            InstallError::NotInstalled(version) => VersionProblem::NotInstalled(version),
            InstallError::ActiveInUse => VersionProblem::ActiveInUse,
            InstallError::NoPrevious => VersionProblem::NoPrevious,
            InstallError::NotAbsolute(_) => VersionProblem::NotAbsolute,
            InstallError::NotOnPath => VersionProblem::NotOnPath,
        }
    }

    fn candidate(programs: &crate::core::node::Programs) -> Candidate {
        Candidate {
            version: programs.version.to_string(),
            dir: programs.dir.display().to_string(),
            supported: programs.version.is_supported(),
        }
    }

    fn listed(release: &Release) -> AvailableRelease {
        let block = if !release.version.is_supported() {
            Some(ReleaseBlock::TooOld)
        } else {
            match &release.asset {
                None => Some(ReleaseBlock::NoDownload),
                Some(asset) if asset.sha256.is_none() => Some(ReleaseBlock::NoDigest),
                Some(_) => None,
            }
        };
        AvailableRelease {
            tag: release.tag.clone(),
            version: release.version.to_string(),
            number: release.version.number(),
            prerelease: release.prerelease,
            published: release
                .published_at
                .get(..10)
                .unwrap_or(&release.published_at)
                .to_owned(),
            size: release.asset.as_ref().map(|asset| asset.size),
            verified: release
                .asset
                .as_ref()
                .is_some_and(|asset| asset.sha256.is_some()),
            block,
        }
    }

    fn version_of(number: &str) -> Result<CoreVersion, VersionProblem> {
        number
            .parse()
            .map_err(|_| VersionProblem::NotInstalled(number.to_owned()))
    }

    impl Backend {
        /// The installer over the app's data directory.
        pub(super) fn open() -> Result<Self, VersionProblem> {
            Installations::new()
                .map(|installations| Self {
                    installations,
                    listed: Mutex::new(Vec::new()),
                })
                .map_err(problem)
        }

        /// Where the sidecar and the control plane find the active release.
        pub(super) fn programs(&self) -> Arc<dyn ProgramSource> {
            Arc::new(self.installations.clone())
        }

        /// The active release (runs `--version` once per file change): blocking.
        pub(super) fn active(&self) -> ActiveRelease {
            match self.installations.active() {
                Ok(programs) => ActiveRelease::Ready {
                    version: programs.version.to_string(),
                },
                Err(NodeError::CoreMissing) => ActiveRelease::Missing,
                Err(NodeError::CoreUnsupported(version)) => ActiveRelease::Unsupported {
                    version: version.to_string(),
                },
                Err(other) => ActiveRelease::Failed(VersionProblem::Disk(other.to_string())),
            }
        }

        /// The installed releases: blocking.
        pub(super) fn installed(&self) -> Vec<InstalledRelease> {
            self.installations
                .installed()
                .into_iter()
                .map(|release| InstalledRelease {
                    version: release.version.to_string(),
                    number: release.version.number(),
                    dir: release.dir.display().to_string(),
                    active: release.active,
                    previous: release.previous,
                    custom: release.custom,
                    usable: release.usable,
                })
                .collect()
        }

        /// Looks for a core on the PATH: blocking.
        pub(super) fn detect_path(&self) -> Detection {
            match self.installations.detect_on_path() {
                Ok(programs) => Detection::Found(candidate(&programs)),
                Err(InstallError::NotOnPath) => Detection::NotFound,
                Err(error) => Detection::Failed(problem(error)),
            }
        }

        /// Reads the programs in `dir`: blocking.
        pub(super) fn detect_dir(&self, dir: &str) -> DirCheck {
            match self.installations.detect_dir(Path::new(dir)) {
                Ok(programs) => DirCheck::Found(candidate(&programs)),
                Err(error) => DirCheck::Failed(problem(error)),
            }
        }

        /// Makes the installed release `number` active: blocking.
        pub(super) fn activate(&self, number: &str) -> Result<(), VersionProblem> {
            let version = version_of(number)?;
            self.installations
                .activate(&version)
                .map(|_| ())
                .map_err(problem)
        }

        /// Returns to the previous release: blocking.
        pub(super) fn rollback(&self) -> Result<(), VersionProblem> {
            self.installations.rollback().map(|_| ()).map_err(problem)
        }

        /// Runs the programs in `dir`, which the user confirmed: blocking.
        pub(super) fn adopt(&self, dir: &str) -> Result<(), VersionProblem> {
            self.installations
                .adopt_custom(Path::new(dir))
                .map(|_| ())
                .map_err(problem)
        }

        /// Deletes the installed release `number`: blocking.
        pub(super) fn remove(&self, number: &str) -> Result<(), VersionProblem> {
            let version = version_of(number)?;
            self.installations.remove(&version).map_err(problem)
        }

        /// GitHub's release list, remembered for [`Self::install`].
        pub(super) async fn list(&self) -> Result<Vec<AvailableRelease>, VersionProblem> {
            let releases = install::list_releases().await.map_err(problem)?;
            let items = releases.iter().map(listed).collect();
            *lock(&self.listed) = releases;
            Ok(items)
        }

        /// Installs the listed release `tag` (not activated) and returns its version number.
        pub(super) async fn install(
            &self,
            tag: &str,
            mut on_step: impl FnMut(InstallStep),
        ) -> Result<String, VersionProblem> {
            let release = lock(&self.listed)
                .iter()
                .find(|release| release.tag == tag)
                .cloned()
                .ok_or(VersionProblem::NoDownload)?;
            let programs = self
                .installations
                .install(&release, |phase| match phase {
                    InstallPhase::Downloading => on_step(InstallStep::Downloading),
                    InstallPhase::Verifying => on_step(InstallStep::Verifying),
                    InstallPhase::Unpacking => on_step(InstallStep::Unpacking),
                    InstallPhase::Done => {}
                })
                .await
                .map_err(problem)?;
            Ok(programs.version.number())
        }
    }
}

/// No installer where the app cannot run EasyTier's programs: the type has no values, so none of its methods runs.
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod backend {
    use std::sync::Arc;

    use super::{
        ActiveRelease, AvailableRelease, Detection, DirCheck, InstallStep, InstalledRelease,
        VersionProblem,
    };
    use crate::core::node::ProgramSource;

    /// Nothing to manage on this platform.
    pub(super) enum Backend {}

    impl Backend {
        pub(super) fn open() -> Result<Self, VersionProblem> {
            Err(VersionProblem::Unavailable)
        }

        pub(super) fn programs(&self) -> Arc<dyn ProgramSource> {
            match *self {}
        }

        pub(super) fn active(&self) -> ActiveRelease {
            match *self {}
        }

        pub(super) fn installed(&self) -> Vec<InstalledRelease> {
            match *self {}
        }

        pub(super) fn detect_path(&self) -> Detection {
            match *self {}
        }

        pub(super) fn detect_dir(&self, _dir: &str) -> DirCheck {
            match *self {}
        }

        pub(super) fn activate(&self, _number: &str) -> Result<(), VersionProblem> {
            match *self {}
        }

        pub(super) fn rollback(&self) -> Result<(), VersionProblem> {
            match *self {}
        }

        pub(super) fn adopt(&self, _dir: &str) -> Result<(), VersionProblem> {
            match *self {}
        }

        pub(super) fn remove(&self, _number: &str) -> Result<(), VersionProblem> {
            match *self {}
        }

        pub(super) async fn list(&self) -> Result<Vec<AvailableRelease>, VersionProblem> {
            match *self {}
        }

        pub(super) async fn install(
            &self,
            _tag: &str,
            _on_step: impl FnMut(InstallStep),
        ) -> Result<String, VersionProblem> {
            match *self {}
        }
    }
}

/// Carries out `change` (blocking).
fn apply(backend: &Backend, change: &Change) -> Result<(), VersionProblem> {
    match change {
        Change::Activate(number) => backend.activate(number),
        Change::Rollback => backend.rollback(),
        Change::Adopt(dir) => backend.adopt(dir),
    }
}

/// See the module docs. A `Copy` handle of signals, like [`Store`].
#[derive(Clone, Copy)]
pub struct VersionStore {
    /// What the store's tasks run under: the app, not the page that started them, so leaving a page never cancels an
    /// install or a switch.
    owner: Owner,
    /// The desktops' installer.
    backend: Stored<Option<Arc<Backend>>>,
    /// The root store, once it exists.
    root: Stored<Option<Store>>,
    /// Bumped by every read of the installed state, so an older answer is dropped.
    revision: Signal<u64>,
    active: Signal<ActiveRelease>,
    installed: Signal<Vec<InstalledRelease>>,
    detection: Signal<Detection>,
    custom_dir: Signal<String>,
    custom: Signal<DirCheck>,
    releases: Signal<ReleaseList>,
    progress: Signal<Option<InstallProgress>>,
    /// The version number of a release just installed, offered for activation.
    fresh: Signal<Option<String>>,
    switching: Signal<bool>,
    problem: Signal<Option<VersionProblem>>,
}

impl VersionStore {
    /// Its state at start on `platform`.
    pub(super) fn new(platform: Platform) -> Self {
        let mut problem = None;
        let backend = if platform.is_desktop() {
            match Backend::open() {
                Ok(backend) => Some(Arc::new(backend)),
                Err(error) => {
                    log::warn!("the EasyTier installer is unavailable: {error:?}");
                    problem = Some(error);
                    None
                }
            }
        } else {
            None
        };
        let active = if backend.is_some() {
            ActiveRelease::Checking
        } else {
            ActiveRelease::Unmanaged
        };
        Self {
            owner: Owner::current(),
            backend: stored(backend),
            root: stored(None),
            revision: signal(0),
            active: signal(active),
            installed: signal(Vec::new()),
            detection: signal(Detection::Unchecked),
            custom_dir: signal(String::new()),
            custom: signal(DirCheck::Idle),
            releases: signal(ReleaseList::NotLoaded),
            progress: signal(None),
            fresh: signal(None),
            switching: signal(false),
            problem: signal(problem),
        }
    }

    /// Where the sidecar and the control plane find the active release; `None` where EasyTier's programs are not
    /// run as processes.
    pub(super) fn programs(&self) -> Option<Arc<dyn ProgramSource>> {
        self.backend.get().map(|backend| backend.programs())
    }

    /// Restores what was kept across runs (detects the active release), once the root store exists.
    pub(super) fn restore(&self, store: Store) {
        self.root.with(|root| *root = Some(store));
        self.refresh();
    }

    /// Whether this device runs EasyTier's programs as processes it installs and switches.
    pub fn manages_programs(&self) -> bool {
        self.backend.get().is_some()
    }

    /// The release the next start runs (tracked).
    pub fn active(&self) -> ActiveRelease {
        self.active.get()
    }

    /// The installed releases, newest first (tracked).
    pub fn installed(&self) -> Vec<InstalledRelease> {
        self.installed.get()
    }

    /// The search for a release on the PATH (tracked).
    pub fn detection(&self) -> Detection {
        self.detection.get()
    }

    /// The directory the user is typing (tracked).
    pub fn custom_dir(&self) -> String {
        self.custom_dir.get()
    }

    /// Sets the directory text; a changed text forgets the earlier check.
    pub fn set_custom_dir(&self, text: &str) {
        batch(|| {
            self.custom_dir.set(text.to_owned());
            self.custom.set(DirCheck::Idle);
        });
    }

    /// The check of the typed directory (tracked).
    pub fn custom_check(&self) -> DirCheck {
        self.custom.get()
    }

    /// GitHub's release list (tracked).
    pub fn releases(&self) -> ReleaseList {
        self.releases.get()
    }

    /// The install in progress (tracked).
    pub fn progress(&self) -> Option<InstallProgress> {
        self.progress.get()
    }

    /// The version number of a release that was just installed and not yet activated (tracked).
    pub fn fresh(&self) -> Option<String> {
        self.fresh.get()
    }

    /// Whether a switch of the active release is under way (tracked).
    pub fn switching(&self) -> bool {
        self.switching.get()
    }

    /// The last failure of an action (tracked).
    pub fn problem(&self) -> Option<VersionProblem> {
        self.problem.get()
    }

    /// Forgets the last failure.
    pub fn dismiss_problem(&self) {
        self.problem.set(None);
    }

    /// The oldest release the app supports.
    pub fn oldest_supported(&self) -> String {
        MIN_CORE_VERSION.to_string()
    }

    /// The newest stable release GitHub lists, and how the active release stands to it (tracked). `None` until the
    /// list is loaded or when it holds no stable release.
    pub fn latest(&self) -> Option<Latest> {
        let ReleaseList::Ready(list) = self.releases.get() else {
            return None;
        };
        let release = list
            .into_iter()
            .filter(|release| !release.prerelease)
            .max_by(|a, b| number_key(&a.number).cmp(&number_key(&b.number)))?;
        let active = self
            .installed
            .get()
            .into_iter()
            .find(|installed| installed.active)
            .map(|installed| installed.number);
        let standing = match active {
            None => Standing::Unrelated,
            Some(number) if is_newer(&release.number, &number) => {
                if release.block.is_none() {
                    Standing::Update
                } else {
                    Standing::Unrelated
                }
            }
            Some(_) => Standing::UpToDate,
        };
        Some(Latest { release, standing })
    }

    /// The EasyTier library version built into the app on iOS and Android; `None` elsewhere.
    pub fn embedded_version(&self) -> Option<String> {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            Some(crate::core::embedded::library_version())
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            None
        }
    }

    /// The Settings page opened: reads the releases on this device, looks on the PATH and asks GitHub once.
    pub fn opened(&self) {
        let this = *self;
        // After the page is built: a build must not write the state it reads.
        this.launch(async move {
            this.refresh();
            this.detect_path();
            this.ensure_releases();
        });
    }

    /// The owner of the signals is gone (the app is quitting): a late answer has nowhere to go.
    fn alive(&self) -> bool {
        self.owner.is_live()
    }

    /// Runs `future` on the engine thread under the store's owner.
    fn launch(&self, future: impl Future<Output = ()> + 'static) {
        self.owner.run(|| {
            spawn(future);
        });
    }

    fn parts(&self) -> Option<(Store, Arc<Backend>)> {
        Some((self.root.get()?, self.backend.get()?))
    }

    /// Reads the active and the installed releases off-thread.
    pub fn refresh(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        let this = *self;
        let revision = self.revision.get_untracked().saturating_add(1);
        self.revision.set(revision);
        self.launch(async move {
            let reading = off_thread(move || (backend.active(), backend.installed())).await;
            if this.revision.try_get() != Some(revision) {
                return;
            }
            match reading {
                Ok((active, installed)) => {
                    this.active.try_set(active);
                    this.installed.try_set(installed);
                }
                Err(error) => {
                    log::warn!("reading the installed releases failed: {error}");
                    this.active
                        .try_set(ActiveRelease::Failed(VersionProblem::Interrupted));
                }
            }
        });
    }

    /// Looks for a release on the PATH (runs its `--version` off-thread).
    pub fn detect_path(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        if matches!(self.detection.get_untracked(), Detection::Searching) {
            return;
        }
        self.detection.set(Detection::Searching);
        let this = *self;
        self.launch(async move {
            let found = off_thread(move || backend.detect_path())
                .await
                .unwrap_or_else(|error| {
                    log::warn!("looking on the PATH failed: {error}");
                    Detection::Failed(VersionProblem::Interrupted)
                });
            this.detection.try_set(found);
        });
    }

    /// Checks the directory the user typed.
    pub fn check_custom_dir(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        let dir = self.custom_dir.get_untracked().trim().to_owned();
        if dir.is_empty() || matches!(self.custom.get_untracked(), DirCheck::Checking) {
            return;
        }
        self.custom.set(DirCheck::Checking);
        let this = *self;
        self.launch(async move {
            let checked = off_thread(move || backend.detect_dir(&dir))
                .await
                .unwrap_or_else(|error| {
                    log::warn!("checking the directory failed: {error}");
                    DirCheck::Failed(VersionProblem::Interrupted)
                });
            this.custom.try_set(checked);
        });
    }

    /// Asks GitHub for the release list unless it was asked already.
    pub fn ensure_releases(&self) {
        if matches!(self.releases.get_untracked(), ReleaseList::NotLoaded) {
            self.load_releases();
        }
    }

    /// Asks GitHub for the release list again.
    pub fn load_releases(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        if matches!(self.releases.get_untracked(), ReleaseList::Loading) {
            return;
        }
        self.releases.set(ReleaseList::Loading);
        let this = *self;
        self.launch(async move {
            let listed = match backend.list().await {
                Ok(items) => ReleaseList::Ready(items),
                Err(problem) => ReleaseList::Failed(problem),
            };
            this.releases.try_set(listed);
        });
    }

    /// Installs the listed release `tag` after the user confirmed it; the release is not activated.
    pub fn install(&self, tag: &str) {
        let Some((store, backend)) = self.parts() else {
            return;
        };
        if self.progress.get_untracked().is_some() || self.switching.get_untracked() {
            return;
        }
        let release = match self.releases.get_untracked() {
            ReleaseList::Ready(list) => list.into_iter().find(|release| release.tag == tag),
            _ => None,
        };
        let Some(release) = release else {
            log::warn!("release {tag} is not in the list; nothing to install");
            return;
        };
        if release.block.is_some() {
            return;
        }
        let strings = store.strings();
        let size = release.size.map_or_else(String::new, format_size);
        let asked = confirm(
            fill(
                strings.versions.confirm_install_title,
                &[("version", &release.version)],
            ),
            fill(strings.versions.confirm_install_body, &[("size", &size)]),
            strings.versions.install,
            strings.common.cancel,
            false,
        );
        let this = *self;
        let tag = release.tag;
        self.launch(async move {
            if asked.await {
                this.run_install(backend, tag).await;
            }
        });
    }

    async fn run_install(self, backend: Arc<Backend>, tag: String) {
        if self.progress.get_untracked().is_some() {
            return;
        }
        self.progress.set(Some(InstallProgress {
            tag: tag.clone(),
            step: InstallStep::Starting,
        }));
        batch(|| {
            self.problem.set(None);
            self.fresh.set(None);
        });
        let reporter = self;
        let step_tag = tag.clone();
        let result = backend
            .install(&tag, move |step| {
                reporter.progress.try_set(Some(InstallProgress {
                    tag: step_tag.clone(),
                    step,
                }));
            })
            .await;
        if !self.alive() {
            return;
        }
        self.progress.set(None);
        match result {
            Ok(number) => {
                self.fresh.set(Some(number));
                self.refresh();
            }
            Err(problem) => self.problem.set(Some(problem)),
        }
    }

    /// Makes the installed release `number` the active one.
    pub fn activate(&self, number: &str) {
        self.switch(Change::Activate(number.to_owned()));
    }

    /// Returns to the previous release.
    pub fn rollback(&self) {
        self.switch(Change::Rollback);
    }

    /// Uses the release in `candidate.dir` after the user confirmed its version and path.
    pub fn adopt(&self, candidate: &Candidate) {
        let Some((store, _)) = self.parts() else {
            return;
        };
        if !candidate.supported || self.switching.get_untracked() {
            return;
        }
        let strings = store.strings();
        let asked = confirm(
            fill(
                strings.versions.confirm_adopt_title,
                &[("version", &candidate.version)],
            ),
            fill(
                strings.versions.confirm_adopt_body,
                &[("path", &candidate.dir)],
            ),
            strings.versions.confirm_adopt_action,
            strings.common.cancel,
            false,
        );
        let this = *self;
        let dir = candidate.dir.clone();
        self.launch(async move {
            if asked.await {
                this.switch(Change::Adopt(dir));
            }
        });
    }

    /// Stops what runs, switches, restarts the hosted control plane if it ran, then starts the networks again.
    fn switch(&self, change: Change) {
        let Some((store, backend)) = self.parts() else {
            return;
        };
        if self.switching.get_untracked() || self.progress.get_untracked().is_some() {
            return;
        }
        let this = *self;
        batch(|| {
            self.switching.set(true);
            self.problem.set(None);
        });
        self.launch(async move {
            let stopped = store.node().stop_all_running().await;
            let applied = off_thread(move || apply(&backend, &change)).await;
            let outcome = applied.unwrap_or_else(|error| {
                log::warn!("switching the release failed: {error}");
                Err(VersionProblem::Interrupted)
            });
            if outcome.is_ok() {
                store.host().restart_if_running().await;
            }
            store.node().start_profiles(stopped);
            if !this.alive() {
                return;
            }
            batch(|| {
                this.switching.set(false);
                match outcome {
                    Ok(()) => {
                        this.fresh.set(None);
                        this.custom.set(DirCheck::Idle);
                    }
                    Err(problem) => this.problem.set(Some(problem)),
                }
            });
            this.refresh();
            this.detect_path();
        });
    }

    /// Deletes the installed release `number` after the user confirmed it. The active release cannot be removed.
    pub fn remove(&self, number: &str) {
        let Some((store, backend)) = self.parts() else {
            return;
        };
        let version = self
            .installed
            .get_untracked()
            .into_iter()
            .find(|release| release.number == number)
            .map_or_else(|| number.to_owned(), |release| release.version);
        let strings = store.strings();
        let asked = confirm(
            fill(
                strings.versions.confirm_remove_title,
                &[("version", &version)],
            ),
            strings.versions.confirm_remove_body,
            strings.versions.remove,
            strings.common.cancel,
            true,
        );
        let this = *self;
        let number = number.to_owned();
        self.launch(async move {
            if !asked.await {
                return;
            }
            let removed = off_thread(move || backend.remove(&number))
                .await
                .unwrap_or_else(|error| {
                    log::warn!("removing the release failed: {error}");
                    Err(VersionProblem::Interrupted)
                });
            if !this.alive() {
                return;
            }
            match removed {
                Ok(()) => {
                    this.problem.set(None);
                    this.refresh();
                }
                Err(problem) => this.problem.set(Some(problem)),
            }
        });
    }
}
