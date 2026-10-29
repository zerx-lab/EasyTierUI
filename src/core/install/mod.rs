//! EasyTier releases on this desktop: detect, download, verify, install, switch, roll back.
//!
//! Layout under the app's data directory (`cores/`, owner-only):
//!
//! ```text
//! cores/2.6.4/            one release's programs, side by side with others
//! cores/2.6.5.partial/    an unpack in progress; never used, removed on failure
//! cores/active            "2.6.4" or "custom:/abs/dir": what the next start of a node or host runs
//! cores/previous          the same, what `rollback` returns to
//! ```
//!
//! [`Installations`] implements [`ProgramSource`], which `sidecar` and `controlplane` read at every start, so a
//! switch applies to the next start. Methods that touch the disk or run a program block: call them through
//! [`crate::core::work::off_thread`]. [`list_releases`], [`download`] and [`Installations::install`] are async and
//! belong in an NSH task (`spawn`), because they use NSH's `fetch`.
//!
//! Nothing is installed silently and nothing is installed into the `PATH`: the store asks the user before
//! `install`, and a user-chosen directory (`adopt_custom`) needs explicit confirmation after [`detect_dir`] showed
//! its version.
//!
//! [`detect_dir`]: Installations::detect_dir

mod error;
mod programs;
mod release;
mod unpack;

use std::{
    collections::HashMap,
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::SystemTime,
};

pub use error::InstallError;
pub use release::{
    Asset, AssetTarget, Release, download, list_releases, parse_releases, verify_sha256,
};

use super::{
    node::{NodeError, ProgramSource, Programs},
    paths::{data_subdir, ensure_private_dir, remove_if_exists, write_private_file},
    version::CoreVersion,
    work::off_thread,
};

/// The pointer file naming the release that runs.
const ACTIVE: &str = "active";
/// The pointer file naming the release `rollback` returns to.
const PREVIOUS: &str = "previous";
/// A pointer's prefix for a user-chosen directory.
const CUSTOM_PREFIX: &str = "custom:";

/// Where in an install the UI is, for a progress line. `fetch` does not stream, so there are no byte counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPhase {
    /// Fetching the zip from GitHub.
    Downloading,
    /// Checking its sha256 against the release's digest.
    Verifying,
    /// Unpacking, checking the unpacked core's version and moving it into place.
    Unpacking,
    /// Installed (not yet active).
    Done,
}

/// What an `active` or `previous` pointer names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreRef {
    /// A release installed under `cores/<number>/` (the number without commit).
    Release(CoreVersion),
    /// A directory the user chose, outside the app's data directory.
    Custom(PathBuf),
}

impl CoreRef {
    /// The pointer for `version`'s release directory.
    fn release(version: &CoreVersion) -> Self {
        Self::Release(CoreVersion {
            commit: None,
            ..version.clone()
        })
    }

    /// The pointer file's text.
    fn to_text(&self) -> Result<String, InstallError> {
        match self {
            Self::Release(version) => Ok(version.number()),
            Self::Custom(dir) => dir
                .to_str()
                .map(|text| format!("{CUSTOM_PREFIX}{text}"))
                .ok_or_else(|| InstallError::Io("the path is not valid text".to_owned())),
        }
    }

    /// Reads a pointer file's text; anything that is not a bare version number or `custom:<absolute path>` is
    /// refused (a pointer never becomes a path by accident).
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if let Some(dir) = text.strip_prefix(CUSTOM_PREFIX) {
            let dir = PathBuf::from(dir);
            return dir.is_absolute().then_some(Self::Custom(dir));
        }
        let version: CoreVersion = text.parse().ok()?;
        (version.commit.is_none() && version.number() == text).then_some(Self::Release(version))
    }
}

/// One entry of [`Installations::installed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRelease {
    /// Its version (with the commit when the program could be run).
    pub version: CoreVersion,
    /// The directory holding the programs.
    pub dir: PathBuf,
    /// The next start runs this one.
    pub active: bool,
    /// `rollback` returns to this one.
    pub previous: bool,
    /// A user-chosen directory, not one of the app's own installs.
    pub custom: bool,
    /// All three programs exist, the core reports a version and it is supported. A release that is not usable can
    /// still be removed (or reinstalled).
    pub usable: bool,
}

/// What `--version` reported for one core file, valid while the file is unchanged.
#[derive(Debug)]
struct Detected {
    modified: Option<SystemTime>,
    len: u64,
    version: CoreVersion,
}

#[derive(Debug)]
struct Inner {
    root: PathBuf,
    /// Parsed versions per release directory: `--version` runs a process, and the sidecar asks at each start.
    versions: Mutex<HashMap<PathBuf, Detected>>,
    /// Serializes everything that changes the layout or the pointers.
    changes: Mutex<()>,
}

/// The installed EasyTier releases and the pointers over them. Cheap to clone (shared state); `Send + Sync`.
#[derive(Debug, Clone)]
pub struct Installations {
    inner: Arc<Inner>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The guarded data (a cache, a unit) is never left half-updated, so a poisoned lock is still consistent.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Deletes a directory tree, logging instead of failing: this runs while another error is being reported.
fn remove_tree_quietly(dir: &Path) {
    match fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => log::warn!("could not remove {}: {error}", dir.display()),
    }
}

impl Installations {
    /// The installations in the app's data directory (`cores/`).
    ///
    /// # Errors
    ///
    /// [`InstallError::DataDir`] when the data directory is unusable.
    pub fn new() -> Result<Self, InstallError> {
        let root =
            data_subdir("cores").map_err(|error| InstallError::DataDir(error.to_string()))?;
        Ok(Self::with_root(root))
    }

    /// The installations under `root` (created on first write). For tests and portable layouts.
    pub fn with_root(root: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                root,
                versions: Mutex::new(HashMap::new()),
                changes: Mutex::new(()),
            }),
        }
    }

    /// The `cores` directory.
    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    /// Where release `version` is (or would be) installed.
    pub fn release_dir(&self, version: &CoreVersion) -> PathBuf {
        self.inner.root.join(version.number())
    }

    fn dir_of(&self, reference: &CoreRef) -> PathBuf {
        match reference {
            CoreRef::Release(version) => self.release_dir(version),
            CoreRef::Custom(dir) => dir.clone(),
        }
    }

    fn read_pointer(&self, name: &str) -> Option<CoreRef> {
        let path = self.inner.root.join(name);
        match fs::read_to_string(&path) {
            Ok(text) => {
                let parsed = CoreRef::parse(&text);
                if parsed.is_none() {
                    log::warn!("ignoring the unreadable pointer {}", path.display());
                }
                parsed
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                log::warn!("could not read {}: {error}", path.display());
                None
            }
        }
    }

    fn write_pointer(&self, name: &str, reference: Option<&CoreRef>) -> Result<(), InstallError> {
        let path = self.inner.root.join(name);
        match reference {
            Some(reference) => {
                ensure_private_dir(&self.inner.root)
                    .map_err(|error| InstallError::io("creating the cores directory", &error))?;
                write_private_file(&path, reference.to_text()?.as_bytes())
                    .map_err(|error| InstallError::io("writing a pointer", &error))
            }
            None => remove_if_exists(&path)
                .map_err(|error| InstallError::io("removing a pointer", &error)),
        }
    }

    /// What the next start of a node or host runs, if anything is chosen (it may be gone from disk).
    pub fn active_ref(&self) -> Option<CoreRef> {
        self.read_pointer(ACTIVE)
    }

    /// What [`Self::rollback`] returns to.
    pub fn previous_ref(&self) -> Option<CoreRef> {
        self.read_pointer(PREVIOUS)
    }

    /// Makes `new` active and the former active release the previous one.
    fn switch_to(&self, new: &CoreRef, former: Option<CoreRef>) -> Result<(), InstallError> {
        match former {
            Some(former) => self.write_pointer(PREVIOUS, Some(&former))?,
            // Nothing to go back to; a stale previous that equals the new active would be a no-op rollback.
            None if self.previous_ref().as_ref() == Some(new) => {
                self.write_pointer(PREVIOUS, None)?
            }
            None => {}
        }
        self.write_pointer(ACTIVE, Some(new))
    }

    /// The programs in `dir` and their version, from the cache while the core file is unchanged.
    fn programs_in(&self, dir: &Path) -> Result<Programs, InstallError> {
        let [core, cli, web] = programs::locate(dir)?;
        let meta = fs::metadata(&core)
            .map_err(|error| InstallError::io("reading the core file", &error))?;
        let (modified, len) = (meta.modified().ok(), meta.len());
        let cached = lock(&self.inner.versions)
            .get(dir)
            .filter(|seen| seen.modified == modified && seen.len == len)
            .map(|seen| seen.version.clone());
        let version = match cached {
            Some(version) => version,
            None => {
                // Outside the lock: running a program can take seconds.
                let version = programs::read_version(&core)?;
                lock(&self.inner.versions).insert(
                    dir.to_path_buf(),
                    Detected {
                        modified,
                        len,
                        version: version.clone(),
                    },
                );
                version
            }
        };
        Ok(Programs {
            version,
            dir: dir.to_path_buf(),
            core,
            cli,
            web,
        })
    }

    /// [`Self::programs_in`] for a release the app can drive.
    fn usable_programs(&self, dir: &Path) -> Result<Programs, InstallError> {
        let found = self.programs_in(dir)?;
        if found.version.is_supported() {
            Ok(found)
        } else {
            Err(InstallError::Unsupported(found.version))
        }
    }

    fn forget(&self, dir: &Path) {
        lock(&self.inner.versions).remove(dir);
    }

    /// The programs in a user-chosen directory and their version, to show the user before [`Self::adopt_custom`].
    /// A version older than [`crate::core::version::MIN_CORE_VERSION`] is returned as is (so the UI can say
    /// which); `adopt_custom` refuses it.
    ///
    /// # Errors
    ///
    /// [`InstallError::NotAbsolute`] for a relative path, [`InstallError::MissingProgram`] when `easytier-core`,
    /// `easytier-cli` or `easytier-web` is not a file there, [`InstallError::VersionUnreadable`].
    pub fn detect_dir(&self, dir: &Path) -> Result<Programs, InstallError> {
        Self::check_custom_path(dir)?;
        self.programs_in(dir)
    }

    /// Looks for `easytier-core` on the `PATH` (absolute entries only) and detects the directory holding it, to
    /// show the user before [`Self::adopt_custom`].
    ///
    /// # Errors
    ///
    /// [`InstallError::NotOnPath`], else as [`Self::detect_dir`] for the directory found.
    pub fn detect_on_path(&self) -> Result<Programs, InstallError> {
        let path = std::env::var_os("PATH").ok_or(InstallError::NotOnPath)?;
        let core = programs::file_name(programs::CORE);
        let dir = std::env::split_paths(&path)
            .filter(|dir| dir.is_absolute())
            .find(|dir| fs::metadata(dir.join(&core)).is_ok_and(|meta| meta.is_file()))
            .ok_or(InstallError::NotOnPath)?;
        self.detect_dir(&dir)
    }

    fn check_custom_path(dir: &Path) -> Result<(), InstallError> {
        if dir.is_absolute() && !dir.components().any(|part| part == Component::ParentDir) {
            Ok(())
        } else {
            Err(InstallError::NotAbsolute(dir.to_path_buf()))
        }
    }

    /// Makes a user-chosen directory the active release; the former active one becomes the previous one.
    ///
    /// The UI must have shown [`Self::detect_dir`]'s result and got the user's explicit confirmation first.
    /// Running instances must be stopped by the caller before the switch (see [`Self::activate`]).
    ///
    /// # Errors
    ///
    /// As [`Self::detect_dir`], plus [`InstallError::Unsupported`] for a core older than the oldest supported one.
    pub fn adopt_custom(&self, dir: &Path) -> Result<Programs, InstallError> {
        Self::check_custom_path(dir)?;
        let programs = self.usable_programs(dir)?;
        let _changes = lock(&self.inner.changes);
        let reference = CoreRef::Custom(dir.to_path_buf());
        let former = self.active_ref();
        if former.as_ref() != Some(&reference) {
            self.switch_to(&reference, former)?;
        }
        Ok(programs)
    }

    /// Every installed release, newest first, then the user-chosen directories that are active or previous.
    pub fn installed(&self) -> Vec<InstalledRelease> {
        let active = self.active_ref();
        let previous = self.previous_ref();
        let mut found = Vec::new();
        match fs::read_dir(&self.inner.root) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let Ok(name) = entry.file_name().into_string() else {
                        continue;
                    };
                    // Only `<number>` directories: `.partial` leftovers and pointer files are not releases.
                    let Ok(number) = name.parse::<CoreVersion>() else {
                        continue;
                    };
                    if number.commit.is_some() || number.number() != name || !entry.path().is_dir()
                    {
                        continue;
                    }
                    let reference = CoreRef::Release(number.clone());
                    let dir = entry.path();
                    let usable = self.usable_programs(&dir);
                    found.push(InstalledRelease {
                        version: match &usable {
                            Ok(programs) => programs.version.clone(),
                            Err(_) => self.programs_in(&dir).map_or(number, |p| p.version),
                        },
                        dir,
                        active: active.as_ref() == Some(&reference),
                        previous: previous.as_ref() == Some(&reference),
                        custom: false,
                        usable: usable.is_ok(),
                    });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => log::warn!("could not list {}: {error}", self.inner.root.display()),
        }
        found.sort_by_key(|release| {
            std::cmp::Reverse((
                release.version.major,
                release.version.minor,
                release.version.patch,
            ))
        });
        for (reference, is_active) in [(&active, true), (&previous, false)] {
            let Some(CoreRef::Custom(dir)) = reference else {
                continue;
            };
            let Ok(programs) = self.programs_in(dir) else {
                log::warn!("{} is chosen but has no usable programs", dir.display());
                continue;
            };
            found.push(InstalledRelease {
                usable: programs.version.is_supported(),
                version: programs.version,
                dir: dir.clone(),
                active: is_active,
                previous: !is_active,
                custom: true,
            });
        }
        found
    }

    /// Makes the installed release `version` the active one; the former active one becomes the previous one.
    /// Activating the active release changes nothing.
    ///
    /// Stopping running instances before a switch, and starting them afterwards, is the store's job: a core that
    /// keeps running keeps the old files open, and an OS service keeps the path it was installed with.
    ///
    /// # Errors
    ///
    /// [`InstallError::NotInstalled`] when there is no such directory, [`InstallError::MissingProgram`],
    /// [`InstallError::VersionUnreadable`] or [`InstallError::Unsupported`] when it is broken.
    pub fn activate(&self, version: &CoreVersion) -> Result<Programs, InstallError> {
        let reference = CoreRef::release(version);
        let dir = self.dir_of(&reference);
        if !dir.is_dir() {
            return Err(InstallError::NotInstalled(version.number()));
        }
        let programs = self.usable_programs(&dir)?;
        let _changes = lock(&self.inner.changes);
        let former = self.active_ref();
        if former.as_ref() != Some(&reference) {
            self.switch_to(&reference, former)?;
        }
        Ok(programs)
    }

    /// Goes back to the previous release; the active one becomes the previous one (calling it again returns).
    /// Stop running instances first, as for [`Self::activate`].
    ///
    /// # Errors
    ///
    /// [`InstallError::NoPrevious`] when there is none; the previous one's own errors when it is gone or broken
    /// (nothing changes then).
    pub fn rollback(&self) -> Result<Programs, InstallError> {
        let _changes = lock(&self.inner.changes);
        let target = self.previous_ref().ok_or(InstallError::NoPrevious)?;
        let programs = self.usable_programs(&self.dir_of(&target))?;
        let former = self.active_ref();
        self.write_pointer(PREVIOUS, former.as_ref())?;
        self.write_pointer(ACTIVE, Some(&target))?;
        Ok(programs)
    }

    /// Deletes an installed release. The active one cannot be removed (switch first); removing the previous one
    /// clears the rollback pointer.
    ///
    /// # Errors
    ///
    /// [`InstallError::ActiveInUse`], [`InstallError::NotInstalled`], [`InstallError::Io`].
    pub fn remove(&self, version: &CoreVersion) -> Result<(), InstallError> {
        let _changes = lock(&self.inner.changes);
        let reference = CoreRef::release(version);
        if self.active_ref().as_ref() == Some(&reference) {
            return Err(InstallError::ActiveInUse);
        }
        let dir = self.dir_of(&reference);
        if !dir.is_dir() {
            return Err(InstallError::NotInstalled(version.number()));
        }
        if self.previous_ref().as_ref() == Some(&reference) {
            self.write_pointer(PREVIOUS, None)?;
        }
        fs::remove_dir_all(&dir)
            .map_err(|error| InstallError::io("removing the release", &error))?;
        self.forget(&dir);
        Ok(())
    }

    /// Downloads, verifies and installs `release` side by side with the others. It does not become active: the
    /// store asks the user, then calls [`Self::activate`].
    ///
    /// The sha256 is checked before anything is unpacked; a mismatch writes nothing. The unpacked core must report
    /// the release's version. `on_phase` is called on the calling (engine) thread at each step. Await in an NSH
    /// task (`spawn`).
    ///
    /// # Errors
    ///
    /// [`InstallError::Unsupported`] for a release older than the oldest supported one, [`InstallError::NoAsset`],
    /// [`InstallError::MissingDigest`], [`InstallError::AlreadyInstalled`], the download's errors
    /// ([`download`]), [`InstallError::DigestMismatch`], [`InstallError::BadArchive`], [`InstallError::BadEntry`],
    /// [`InstallError::VersionMismatch`], [`InstallError::Io`].
    pub async fn install(
        &self,
        release: &Release,
        mut on_phase: impl FnMut(InstallPhase),
    ) -> Result<Programs, InstallError> {
        if !release.version.is_supported() {
            return Err(InstallError::Unsupported(release.version.clone()));
        }
        let asset = release.asset.as_ref().ok_or(InstallError::NoAsset)?;
        let sha256 = asset.sha256.clone().ok_or(InstallError::MissingDigest)?;

        // Fail before a 30 MB download when it is already there.
        let this = self.clone();
        let version = release.version.clone();
        off_thread(move || this.check_not_installed(&version))
            .await
            .map_err(|error| InstallError::Interrupted(error.to_string()))??;

        on_phase(InstallPhase::Downloading);
        let bytes = download(asset).await?;

        on_phase(InstallPhase::Verifying);
        let bytes = off_thread(move || verify_sha256(&bytes, &sha256).map(|()| bytes))
            .await
            .map_err(|error| InstallError::Interrupted(error.to_string()))??;

        on_phase(InstallPhase::Unpacking);
        let this = self.clone();
        let version = release.version.clone();
        let programs = off_thread(move || this.install_archive(&version, &bytes))
            .await
            .map_err(|error| InstallError::Interrupted(error.to_string()))??;

        on_phase(InstallPhase::Done);
        Ok(programs)
    }

    /// [`InstallError::AlreadyInstalled`] when `version` is installed and works.
    fn check_not_installed(&self, version: &CoreVersion) -> Result<(), InstallError> {
        let dir = self.release_dir(version);
        if dir.is_dir() && self.usable_programs(&dir).is_ok() {
            return Err(InstallError::AlreadyInstalled(version.number()));
        }
        Ok(())
    }

    /// Unpacks the (already digest-verified) zip `bytes` of release `version` into `cores/<number>.partial/`,
    /// checks the unpacked core's version and moves it to `cores/<number>/`. A broken earlier install of the same
    /// version is replaced. On any error nothing is left behind.
    fn install_archive(
        &self,
        version: &CoreVersion,
        bytes: &[u8],
    ) -> Result<Programs, InstallError> {
        let _changes = lock(&self.inner.changes);
        let number = version.number();
        let dir = self.release_dir(version);
        let partial = self.inner.root.join(format!("{number}.partial"));
        self.check_not_installed(version)?;
        ensure_private_dir(&self.inner.root)
            .map_err(|error| InstallError::io("creating the cores directory", &error))?;
        remove_tree_quietly(&partial);

        let unpacked = (|| {
            unpack::unpack_zip(bytes, &partial)?;
            let [core, _, _] = programs::locate(&partial)?;
            let found = programs::read_version(&core)?;
            if found.number() != number {
                return Err(InstallError::VersionMismatch {
                    expected: number.clone(),
                    found: found.number(),
                });
            }
            if !found.is_supported() {
                return Err(InstallError::Unsupported(found));
            }
            Ok(())
        })();
        if let Err(error) = unpacked {
            remove_tree_quietly(&partial);
            return Err(error);
        }

        // A directory here is a broken install (check_not_installed passed): replace it.
        remove_tree_quietly(&dir);
        self.forget(&dir);
        if let Err(error) = fs::rename(&partial, &dir) {
            remove_tree_quietly(&partial);
            return Err(InstallError::io("moving the release into place", &error));
        }
        self.usable_programs(&dir)
    }
}

impl ProgramSource for Installations {
    /// The active release's programs; checks that all three exist and the core's version parses (cached while
    /// the core file is unchanged).
    fn active(&self) -> Result<Programs, NodeError> {
        let Some(reference) = self.active_ref() else {
            return Err(NodeError::CoreMissing);
        };
        match self.usable_programs(&self.dir_of(&reference)) {
            Ok(programs) => Ok(programs),
            Err(InstallError::Unsupported(version)) => Err(NodeError::CoreUnsupported(version)),
            Err(error) => {
                log::warn!("the active EasyTier release is unusable: {error}");
                Err(NodeError::CoreMissing)
            }
        }
    }
}

// Two attributes, not `cfg(all(test, unix))`: clippy's test exemptions look for a plain `cfg(test)`.
#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::io::{Cursor, Write as _};

    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;

    fn version(text: &str) -> CoreVersion {
        text.parse().unwrap()
    }

    /// A fresh root per test.
    fn installations(name: &str) -> Installations {
        let root = std::env::temp_dir().join(format!("etui-install-{name}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        Installations::with_root(root)
    }

    /// A shell script that prints `easytier-core <version>`, standing in for a program.
    fn script(version: &str) -> Vec<u8> {
        format!("#!/bin/sh\necho 'easytier-core {version}'\n").into_bytes()
    }

    /// A release zip with the real layout: one top-level directory holding the programs.
    fn release_zip(reported: &str) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for name in [
            "easytier-core",
            "easytier-cli",
            "easytier-web",
            "easytier-web-embed",
        ] {
            writer
                .start_file(
                    format!("easytier-linux-x86_64/{name}"),
                    SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(&script(reported)).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn install(
        installs: &Installations,
        number: &str,
        reported: &str,
    ) -> Result<Programs, InstallError> {
        installs.install_archive(&version(number), &release_zip(reported))
    }

    /// A directory with the three programs, like a user-chosen one.
    fn custom_dir(name: &str, reported: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        remove_tree_quietly(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in ["easytier-core", "easytier-cli", "easytier-web"] {
            let file = dir.join(name);
            fs::write(&file, script(reported)).unwrap();
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir
    }

    #[test]
    fn installs_side_by_side_and_reads_the_version() {
        let installs = installations("install");
        let programs = install(&installs, "2.6.4", "2.6.4-8428a89d").unwrap();
        assert_eq!(programs.version.to_string(), "2.6.4-8428a89d");
        assert_eq!(programs.dir, installs.root().join("2.6.4"));
        assert!(programs.core.is_file() && programs.cli.is_file() && programs.web.is_file());
        assert!(!installs.root().join("2.6.4.partial").exists());

        assert_eq!(
            install(&installs, "2.6.4", "2.6.4-8428a89d"),
            Err(InstallError::AlreadyInstalled("2.6.4".to_owned()))
        );
        install(&installs, "2.6.5", "2.6.5-aaaaaaaa").unwrap();
        let listed = installs.installed();
        let numbers: Vec<_> = listed.iter().map(|r| r.version.number()).collect();
        assert_eq!(numbers, ["2.6.5", "2.6.4"]);
        assert!(
            listed
                .iter()
                .all(|r| r.usable && !r.active && !r.previous && !r.custom)
        );
        fs::remove_dir_all(installs.root()).unwrap();
    }

    #[test]
    fn a_wrong_or_unsupported_core_leaves_nothing_behind() {
        let installs = installations("mismatch");
        assert_eq!(
            install(&installs, "2.6.4", "2.6.9-8428a89d"),
            Err(InstallError::VersionMismatch {
                expected: "2.6.4".to_owned(),
                found: "2.6.9".to_owned()
            })
        );
        assert!(matches!(
            install(&installs, "2.5.0", "2.5.0-abcdef01"),
            Err(InstallError::Unsupported(_))
        ));
        let leftovers: Vec<_> = fs::read_dir(installs.root()).unwrap().flatten().collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");

        // A zip missing a program is refused, too.
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("top/easytier-core", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&script("2.6.4-8428a89d")).unwrap();
        let zip = writer.finish().unwrap().into_inner();
        assert_eq!(
            installs.install_archive(&version("2.6.4"), &zip),
            Err(InstallError::MissingProgram("easytier-cli".to_owned()))
        );
        assert!(!installs.root().join("2.6.4").exists());
        assert!(!installs.root().join("2.6.4.partial").exists());
        fs::remove_dir_all(installs.root()).unwrap();
    }

    #[test]
    fn a_traversing_zip_installs_nothing() {
        let installs = installations("traversal");
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("top/../../evil", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"x").unwrap();
        let zip = writer.finish().unwrap().into_inner();
        assert!(matches!(
            installs.install_archive(&version("2.6.4"), &zip),
            Err(InstallError::BadEntry(_))
        ));
        assert!(!installs.root().join("2.6.4.partial").exists());
        assert!(!installs.root().join("..").join("evil").exists());
        fs::remove_dir_all(installs.root()).unwrap();
    }

    #[test]
    fn a_broken_install_is_replaced_by_a_new_one() {
        let installs = installations("repair");
        install(&installs, "2.6.4", "2.6.4-8428a89d").unwrap();
        fs::remove_file(installs.root().join("2.6.4").join("easytier-web")).unwrap();
        assert!(!installs.installed()[0].usable);
        install(&installs, "2.6.4", "2.6.4-8428a89d").unwrap();
        assert!(installs.installed()[0].usable);
        fs::remove_dir_all(installs.root()).unwrap();
    }

    #[test]
    fn active_and_previous_move_together_and_roll_back() {
        let installs = installations("pointers");
        let (a, b) = (version("2.6.4"), version("2.6.5"));
        install(&installs, "2.6.4", "2.6.4-8428a89d").unwrap();
        install(&installs, "2.6.5", "2.6.5-aaaaaaaa").unwrap();
        assert_eq!(installs.active(), Err(NodeError::CoreMissing));
        assert_eq!(installs.rollback(), Err(InstallError::NoPrevious));

        installs.activate(&a).unwrap();
        assert_eq!(installs.active_ref(), Some(CoreRef::Release(a.clone())));
        assert_eq!(installs.previous_ref(), None);
        assert_eq!(installs.active().unwrap().version.number(), "2.6.4");

        // Activating the active one is a no-op.
        installs.activate(&a).unwrap();
        assert_eq!(installs.previous_ref(), None);

        installs.activate(&b).unwrap();
        assert_eq!(installs.active_ref(), Some(CoreRef::Release(b.clone())));
        assert_eq!(installs.previous_ref(), Some(CoreRef::Release(a.clone())));
        assert_eq!(installs.active().unwrap().version.number(), "2.6.5");
        let listed = installs.installed();
        assert!(listed[0].active && !listed[0].previous);
        assert!(listed[1].previous && !listed[1].active);

        installs.rollback().unwrap();
        assert_eq!(installs.active_ref(), Some(CoreRef::Release(a.clone())));
        assert_eq!(installs.previous_ref(), Some(CoreRef::Release(b.clone())));
        assert_eq!(installs.active().unwrap().version.number(), "2.6.4");
        installs.rollback().unwrap();
        assert_eq!(installs.active_ref(), Some(CoreRef::Release(b.clone())));

        // The active release stays; the previous one can go and takes the pointer with it.
        assert_eq!(installs.remove(&b), Err(InstallError::ActiveInUse));
        installs.remove(&a).unwrap();
        assert_eq!(installs.previous_ref(), None);
        assert!(!installs.release_dir(&a).exists());
        assert_eq!(
            installs.remove(&a),
            Err(InstallError::NotInstalled("2.6.4".to_owned()))
        );
        assert_eq!(
            installs.activate(&a),
            Err(InstallError::NotInstalled("2.6.4".to_owned()))
        );
        assert_eq!(installs.rollback(), Err(InstallError::NoPrevious));
        fs::remove_dir_all(installs.root()).unwrap();
    }

    #[test]
    fn active_reports_missing_and_unsupported() {
        let installs = installations("errors");
        install(&installs, "2.6.4", "2.6.4-8428a89d").unwrap();
        installs.activate(&version("2.6.4")).unwrap();
        assert!(installs.active().is_ok());

        // A program vanishes: missing, not a crash.
        fs::remove_file(installs.root().join("2.6.4").join("easytier-cli")).unwrap();
        assert_eq!(installs.active(), Err(NodeError::CoreMissing));

        // A pointer to a release that is not there.
        installs
            .write_pointer(ACTIVE, Some(&CoreRef::Release(version("9.9.9"))))
            .unwrap();
        assert_eq!(installs.active(), Err(NodeError::CoreMissing));

        // An old core in a user-chosen directory is reported as unsupported, never silently run.
        let old = custom_dir("etui-old-core", "2.5.0-abcdef01");
        installs
            .write_pointer(ACTIVE, Some(&CoreRef::Custom(old.clone())))
            .unwrap();
        match installs.active() {
            Err(NodeError::CoreUnsupported(found)) => assert_eq!(found.number(), "2.5.0"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            installs.adopt_custom(&old),
            Err(InstallError::Unsupported(_))
        ));
        fs::remove_dir_all(&old).unwrap();
        fs::remove_dir_all(installs.root()).unwrap();
    }

    #[test]
    fn a_chosen_directory_is_detected_then_adopted_with_rollback() {
        let installs = installations("custom");
        install(&installs, "2.6.4", "2.6.4-8428a89d").unwrap();
        installs.activate(&version("2.6.4")).unwrap();

        let dir = custom_dir("etui-custom-core", "2.7.1-feedbeef");
        let detected = installs.detect_dir(&dir).unwrap();
        assert_eq!(detected.version.number(), "2.7.1");
        // Detecting changes nothing.
        assert_eq!(
            installs.active_ref(),
            Some(CoreRef::Release(version("2.6.4")))
        );

        installs.adopt_custom(&dir).unwrap();
        assert_eq!(installs.active_ref(), Some(CoreRef::Custom(dir.clone())));
        assert_eq!(
            installs.previous_ref(),
            Some(CoreRef::Release(version("2.6.4")))
        );
        assert_eq!(installs.active().unwrap().dir, dir);
        let listed = installs.installed();
        assert!(listed.iter().any(|r| r.custom && r.active && r.usable));

        installs.rollback().unwrap();
        assert_eq!(installs.active().unwrap().version.number(), "2.6.4");

        // Relative and `..` paths are refused; so is a directory without all three programs.
        assert!(matches!(
            installs.detect_dir(Path::new("relative/dir")),
            Err(InstallError::NotAbsolute(_))
        ));
        assert!(matches!(
            installs.detect_dir(&dir.join("..").join("x")),
            Err(InstallError::NotAbsolute(_))
        ));
        fs::remove_file(dir.join("easytier-web")).unwrap();
        assert_eq!(
            installs.detect_dir(&dir),
            Err(InstallError::MissingProgram("easytier-web".to_owned()))
        );
        fs::remove_dir_all(&dir).unwrap();
        fs::remove_dir_all(installs.root()).unwrap();
    }

    #[test]
    fn the_version_is_read_again_when_the_core_file_changes() {
        let installs = installations("cache");
        let programs = install(&installs, "2.6.4", "2.6.4-8428a89d").unwrap();
        assert_eq!(installs.active_for_test(&programs.dir), "2.6.4");
        // Same path, different file: the cache must not serve the old answer.
        fs::write(&programs.core, script("2.6.4-0123456789abcdef")).unwrap();
        assert_eq!(installs.active_for_test(&programs.dir), "2.6.4");
        let seen = lock(&installs.inner.versions)
            .get(&programs.dir)
            .map(|d| d.version.to_string());
        assert_eq!(seen.as_deref(), Some("2.6.4-0123456789abcdef"));
        fs::remove_dir_all(installs.root()).unwrap();
    }

    #[test]
    fn pointers_never_become_paths_by_accident() {
        assert_eq!(
            CoreRef::parse("2.6.4\n"),
            Some(CoreRef::Release(version("2.6.4")))
        );
        assert_eq!(CoreRef::parse("../../etc"), None);
        assert_eq!(CoreRef::parse("v2.6.4"), None);
        assert_eq!(CoreRef::parse("2.6.4-8428a89d"), None);
        assert_eq!(CoreRef::parse("custom:relative"), None);
        assert_eq!(
            CoreRef::parse("custom:/opt/easytier"),
            Some(CoreRef::Custom(PathBuf::from("/opt/easytier")))
        );
        assert_eq!(CoreRef::parse(""), None);
    }

    impl Installations {
        /// The detected number of the programs in `dir`.
        fn active_for_test(&self, dir: &Path) -> String {
            self.programs_in(dir).unwrap().version.number()
        }
    }
}
