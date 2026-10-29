//! The privileged half of the OS-service install: the app re-runs its own executable through the OS's administrator
//! prompt (`Request::invocation`) and this module does the work as root / an administrator.
//!
//! Why: an OS service runs as root (LocalSystem), so everything it starts must be out of the user's reach. The
//! core, the CLI and the rendered config the service uses are therefore *copied* into a directory only the
//! administrator controls, and the copies' sha256 are checked against the digests the app computed before it asked
//! (a file swapped in between is refused). Later `service start / stop / uninstall` run the copied CLI.
//!
//! | OS | Directory | Access |
//! |---|---|---|
//! | macOS | `/Library/Application Support/EasyTierUI/services/<key>/` | root-owned, `0755` (config `0600`) |
//! | Linux | `/var/lib/easytierui/<key>/` | root-owned, `0755` (config `0600`) |
//! | Windows | `%ProgramData%\EasyTierUI\<key>\` | owner Administrators, ACL: SYSTEM and Administrators full; Users only list, and read `logs\` |
//!
//! No shell is involved anywhere: the elevated process is this app's executable with an argument vector (the
//! launcher of [`super::elevate`] passes it as data), the copy and the digest check are Rust, the root-owned CLI is
//! spawned with an argument vector, and the Windows ACL is `icacls.exe` with fixed arguments. Every directory on
//! the way is checked (a real directory, owned by root / the administrator, not writable by others, never a
//! symlink or reparse point) before anything is written, and the request is refused unless its base directory is
//! the one fixed for the OS.
//!
//! `main` hands the process arguments to [`dispatch`] first; a normal launch returns `None` there.

use std::{
    ffi::OsString,
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    time::Duration,
};

use sha2::{Digest as _, Sha256};

use super::run;
use crate::core::node::NodeError;

/// The argument that makes the app run a privileged request instead of opening a window.
pub const FLAG: &str = "--etui-privileged";

/// The longest a `easytier-cli service …` step may take once elevated.
const CLI_TIMEOUT: Duration = Duration::from_secs(120);

/// The longest instance key accepted (`p-` and 64 alphanumerics, with headroom).
const MAX_KEY_CHARS: usize = 80;

/// What the elevated CLI printed when a purge may go on (a service that is already gone).
const NOT_INSTALLED: &str = "not installed";

/// Why a privileged request was refused or failed.
#[derive(Debug, thiserror::Error)]
pub enum PrivilegedError {
    /// The request or a path on the way is not acceptable.
    #[error("{0}")]
    Invalid(String),
    /// A file operation failed.
    #[error("{what}: {source}")]
    Io {
        /// What was being done.
        what: String,
        /// The OS error.
        source: io::Error,
    },
    /// A copy does not match the digest the app computed before elevating.
    #[error(
        "the copy of {0} does not match the digest computed before the prompt; nothing was installed"
    )]
    Digest(String),
}

fn io_error(what: impl Into<String>) -> impl FnOnce(io::Error) -> PrivilegedError {
    let what = what.into();
    move |source| PrivilegedError::Io { what, source }
}

fn invalid(message: impl Into<String>) -> PrivilegedError {
    PrivilegedError::Invalid(message.into())
}

/// Where the administrator-only service directories live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// An existing system directory the chain below it is built from (`/var/lib`).
    pub anchor: PathBuf,
    /// The directory holding one subdirectory per instance.
    pub base: PathBuf,
}

/// The fixed location for this OS; `None` where no desktop service exists.
pub fn system_location() -> Option<Location> {
    if cfg!(target_os = "macos") {
        Some(Location {
            anchor: PathBuf::from("/Library/Application Support"),
            base: PathBuf::from("/Library/Application Support/EasyTierUI/services"),
        })
    } else if cfg!(target_os = "linux") {
        Some(Location {
            anchor: PathBuf::from("/var/lib"),
            base: PathBuf::from("/var/lib/easytierui"),
        })
    } else if cfg!(windows) {
        let data = std::env::var_os("ProgramData")
            .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from);
        Some(Location {
            base: data.join("EasyTierUI"),
            anchor: data,
        })
    } else {
        None
    }
}

/// The name of an official program on this OS.
fn program_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// The files of one service's administrator-only directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceFiles {
    /// `<base>/<key>`: the service's working directory.
    pub dir: PathBuf,
    /// The copied `easytier-core`.
    pub core: PathBuf,
    /// The copied `easytier-cli`: what every elevated `service …` step runs.
    pub cli: PathBuf,
    /// The copied config.
    pub config: PathBuf,
    /// The service's log directory (readable by every user).
    pub logs: PathBuf,
}

impl ServiceFiles {
    /// The layout of instance `key` under `base`.
    pub fn new(base: &Path, key: &str) -> Self {
        let dir = base.join(key);
        Self {
            core: dir.join(program_name("easytier-core")),
            cli: dir.join(program_name("easytier-cli")),
            config: dir.join("config.toml"),
            logs: dir.join("logs"),
            dir,
        }
    }
}

/// A file to copy and the sha256 (lower-case hex) the copy must have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The user-side file.
    pub path: PathBuf,
    /// Its digest, computed by the app just before the prompt.
    pub sha256: String,
}

/// What the elevated process is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Copy the three files into the instance's directory, verify them, then run the copied CLI with `then`
    /// (nothing when empty).
    Stage {
        /// The base directory (must be the fixed one).
        base: PathBuf,
        /// The instance key.
        key: String,
        /// `easytier-core`.
        core: Source,
        /// `easytier-cli`.
        cli: Source,
        /// The rendered config.
        config: Source,
        /// Arguments for the copied CLI.
        then: Vec<OsString>,
    },
    /// Run the copied CLI with `then` (an uninstall) and, when that worked or the service was already gone, delete
    /// the instance's directory.
    Purge {
        /// The base directory (must be the fixed one).
        base: PathBuf,
        /// The instance key.
        key: String,
        /// Arguments for the copied CLI.
        then: Vec<OsString>,
    },
}

/// Whether `key` can name a directory: ASCII letters, digits and `-`.
fn valid_key(key: &str) -> bool {
    (1..=MAX_KEY_CHARS).contains(&key.len())
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Whether `digest` is a lower-case hex sha256.
fn valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

impl Request {
    /// The arguments after the program name: [`FLAG`], the mode, the values, `--`, the CLI's arguments.
    pub fn to_args(&self) -> Vec<OsString> {
        let mut args = vec![OsString::from(FLAG)];
        match self {
            Self::Stage {
                base,
                key,
                core,
                cli,
                config,
                then,
            } => {
                args.push(OsString::from("stage"));
                args.push(base.as_os_str().to_owned());
                args.push(OsString::from(key));
                for source in [core, cli, config] {
                    args.push(source.path.as_os_str().to_owned());
                    args.push(OsString::from(&source.sha256));
                }
                args.push(OsString::from("--"));
                args.extend(then.iter().cloned());
            }
            Self::Purge { base, key, then } => {
                args.push(OsString::from("purge"));
                args.push(base.as_os_str().to_owned());
                args.push(OsString::from(key));
                args.push(OsString::from("--"));
                args.extend(then.iter().cloned());
            }
        }
        args
    }

    /// The program and arguments to hand to the elevator: this executable and [`Self::to_args`].
    ///
    /// # Errors
    ///
    /// [`NodeError::Service`] when the app's own path cannot be read.
    pub fn invocation(&self) -> Result<(PathBuf, Vec<OsString>), NodeError> {
        let exe = std::env::current_exe().map_err(|error| {
            NodeError::Service(format!("the app's own path could not be read: {error}"))
        })?;
        Ok((exe, self.to_args()))
    }

    /// Reads the arguments after [`FLAG`] (the mode first).
    ///
    /// # Errors
    ///
    /// [`PrivilegedError::Invalid`] for anything that is not exactly a request this module writes.
    pub fn parse(args: &[OsString]) -> Result<Self, PrivilegedError> {
        let mut iter = args.iter();
        let mode = text(&mut iter, "mode")?;
        let base = PathBuf::from(take(&mut iter, "base directory")?);
        let key = text(&mut iter, "instance key")?;
        if !valid_key(&key) {
            return Err(invalid("the instance key is not valid"));
        }
        let request = match mode.as_str() {
            "stage" => {
                let core = source(&mut iter, "core")?;
                let cli = source(&mut iter, "cli")?;
                let config = source(&mut iter, "config")?;
                let then = rest(&mut iter)?;
                Self::Stage {
                    base,
                    key,
                    core,
                    cli,
                    config,
                    then,
                }
            }
            "purge" => {
                let then = rest(&mut iter)?;
                Self::Purge { base, key, then }
            }
            _ => return Err(invalid("the request is not known")),
        };
        Ok(request)
    }

    fn base(&self) -> &Path {
        match self {
            Self::Stage { base, .. } | Self::Purge { base, .. } => base,
        }
    }
}

fn take<'a>(
    iter: &mut std::slice::Iter<'a, OsString>,
    what: &str,
) -> Result<&'a OsString, PrivilegedError> {
    iter.next()
        .ok_or_else(|| invalid(format!("the request has no {what}")))
}

fn text(iter: &mut std::slice::Iter<'_, OsString>, what: &str) -> Result<String, PrivilegedError> {
    take(iter, what)?
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("the {what} is not text")))
}

fn source(
    iter: &mut std::slice::Iter<'_, OsString>,
    what: &str,
) -> Result<Source, PrivilegedError> {
    let path = PathBuf::from(take(iter, what)?);
    let sha256 = text(iter, what)?;
    if !valid_digest(&sha256) {
        return Err(invalid(format!("the digest of the {what} is not a sha256")));
    }
    Ok(Source { path, sha256 })
}

/// The `--` separator and then the CLI's arguments.
fn rest(iter: &mut std::slice::Iter<'_, OsString>) -> Result<Vec<OsString>, PrivilegedError> {
    if take(iter, "separator")? != "--" {
        return Err(invalid("the request has no separator"));
    }
    Ok(iter.cloned().collect())
}

/// What a finished request reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The process exit code (`0` on success).
    pub code: u8,
    /// What the CLI printed, if it ran.
    pub output: String,
}

impl Outcome {
    fn done() -> Self {
        Self {
            code: 0,
            output: String::new(),
        }
    }
}

/// If `args` (the process arguments, program name first) are a privileged request, runs it and returns the exit
/// code; `None` for a normal launch.
pub fn dispatch(args: impl IntoIterator<Item = OsString>) -> Option<u8> {
    let mut args = args.into_iter();
    args.next()?;
    let flag = args.next()?;
    if flag != FLAG {
        return None;
    }
    let rest: Vec<OsString> = args.collect();
    let result = Request::parse(&rest).and_then(|request| execute(&request));
    Some(match result {
        Ok(outcome) => {
            report(outcome.code == 0, &outcome.output);
            outcome.code
        }
        Err(error) => {
            report(false, &error.to_string());
            1
        }
    })
}

/// Writes a line to the stream the launcher reads (standard error when failing).
fn report(ok: bool, message: &str) {
    if message.is_empty() {
        return;
    }
    let line = format!("{message}\n");
    let written = if ok {
        io::stdout().write_all(line.as_bytes())
    } else {
        io::stderr().write_all(line.as_bytes())
    };
    if let Err(error) = written {
        log::debug!("the result could not be reported: {error}");
    }
}

/// Runs `request` as the administrator: only against the fixed system location, and only as root / an
/// administrator.
///
/// # Errors
///
/// [`PrivilegedError`] when the request is refused or a step fails.
pub fn execute(request: &Request) -> Result<Outcome, PrivilegedError> {
    let location =
        system_location().ok_or_else(|| invalid("no service directory on this system"))?;
    if request.base() != location.base {
        return Err(invalid("the service directory is not the expected one"));
    }
    apply(request, &location, Trust::Root)
}

/// Who owns the directories the request may rely on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trust {
    /// Root / the administrator (the real thing).
    Root,
    /// The current user (tests, which cannot be root).
    #[cfg(test)]
    CurrentUser,
}

/// What a directory is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    /// Everyone may list it (and walk through to `logs`).
    Traverse,
    /// Everyone may read what is in it.
    Read,
}

/// Whether a copied file is run or only read by the service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Program,
    Config,
}

fn apply(request: &Request, location: &Location, trust: Trust) -> Result<Outcome, PrivilegedError> {
    let owner = sys::Owner::new(trust)?;
    match request {
        Request::Stage {
            key,
            core,
            cli,
            config,
            then,
            ..
        } => {
            let files = ServiceFiles::new(&location.base, key);
            stage(
                &owner,
                location,
                &files,
                [
                    (core, Kind::Program),
                    (cli, Kind::Program),
                    (config, Kind::Config),
                ],
            )?;
            if then.is_empty() {
                return Ok(Outcome::done());
            }
            run_cli(&files.cli, then)
        }
        Request::Purge { key, then, .. } => {
            let files = ServiceFiles::new(&location.base, key);
            purge(&owner, location, &files, then)
        }
    }
}

/// Builds the directory chain, copies each source next to its target under a temporary name, checks the copies'
/// digests, and only then renames them into place.
fn stage(
    owner: &sys::Owner,
    location: &Location,
    files: &ServiceFiles,
    sources: [(&Source, Kind); 3],
) -> Result<(), PrivilegedError> {
    ensure_chain(owner, &location.anchor, &files.dir)?;
    sys::ensure_dir(owner, &files.logs, Access::Read)?;
    let targets = [&files.core, &files.cli, &files.config];
    let mut pending: Vec<(PathBuf, &Path)> = Vec::new();
    let mut failure = None;
    for ((source, kind), target) in sources.into_iter().zip(targets) {
        match copy_verified(owner, source, target, kind) {
            Ok(temp) => pending.push((temp, target.as_path())),
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    if let Some(error) = failure {
        for (temp, _) in &pending {
            remove_quietly(temp);
        }
        return Err(error);
    }
    for (temp, target) in &pending {
        fs::rename(temp, target).map_err(io_error(format!("installing {}", target.display())))?;
    }
    Ok(())
}

/// Creates every directory from just below `anchor` down to `leaf`, administrator-only; an existing one must
/// already be.
fn ensure_chain(owner: &sys::Owner, anchor: &Path, leaf: &Path) -> Result<(), PrivilegedError> {
    let relative = leaf
        .strip_prefix(anchor)
        .map_err(|_| invalid("the service directory is outside the system directory"))?;
    let mut path = anchor.to_path_buf();
    for part in relative.components() {
        path.push(part);
        sys::ensure_dir(owner, &path, Access::Traverse)?;
    }
    Ok(())
}

fn temp_name(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
    target.with_file_name(format!(".new-{name}"))
}

fn remove_quietly(path: &Path) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        log::debug!("could not remove {}: {error}", path.display());
    }
}

/// Copies `source` to a temporary file beside `target`, then reads the copy back and compares its sha256.
fn copy_verified(
    owner: &sys::Owner,
    source: &Source,
    target: &Path,
    kind: Kind,
) -> Result<PathBuf, PrivilegedError> {
    let temp = temp_name(target);
    remove_quietly(&temp);
    let mut input = fs::File::open(&source.path)
        .map_err(io_error(format!("opening {}", source.path.display())))?;
    let is_file = input
        .metadata()
        .map_err(io_error(format!("reading {}", source.path.display())))?
        .is_file();
    if !is_file {
        return Err(invalid(format!(
            "{} is not a regular file",
            source.path.display()
        )));
    }
    {
        let mut output = sys::create_file(owner, &temp, kind)?;
        io::copy(&mut input, &mut output)
            .map_err(io_error(format!("copying {}", source.path.display())))?;
        output
            .sync_all()
            .map_err(io_error(format!("writing {}", temp.display())))?;
    }
    let copied =
        digest_file(&temp).map_err(io_error(format!("reading back {}", temp.display())))?;
    if copied != source.sha256 {
        remove_quietly(&temp);
        let name = target
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        return Err(PrivilegedError::Digest(name));
    }
    sys::seal_file(owner, &temp, kind)?;
    Ok(temp)
}

/// Runs the copied CLI; its output (both streams) and exit code are the outcome.
fn run_cli(cli: &Path, args: &[OsString]) -> Result<Outcome, PrivilegedError> {
    let captured = run::run(run::command(cli, args), CLI_TIMEOUT)
        .map_err(|error| invalid(error.to_string()))?;
    let output = format!("{}\n{}", captured.stdout, captured.stderr)
        .trim()
        .to_owned();
    let code = if captured.success() {
        0
    } else {
        captured
            .status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .filter(|code| *code != 0)
            .unwrap_or(1)
    };
    Ok(Outcome { code, output })
}

/// Uninstalls through the copied CLI, then deletes the instance directory (kept when the uninstall really failed:
/// the service still points at it).
fn purge(
    owner: &sys::Owner,
    location: &Location,
    files: &ServiceFiles,
    then: &[OsString],
) -> Result<Outcome, PrivilegedError> {
    match fs::symlink_metadata(&files.dir) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Outcome::done()),
        Err(error) => return Err(io_error(format!("reading {}", files.dir.display()))(error)),
    }
    ensure_chain(owner, &location.anchor, &files.dir)?;
    if !then.is_empty() && files.cli.is_file() {
        let outcome = run_cli(&files.cli, then)?;
        if outcome.code != 0 && !outcome.output.to_lowercase().contains(NOT_INSTALLED) {
            return Ok(outcome);
        }
    }
    fs::remove_dir_all(&files.dir)
        .map_err(io_error(format!("removing {}", files.dir.display())))?;
    Ok(Outcome::done())
}

/// Lower-case hex sha256 of the file at `path`, read in chunks.
///
/// # Errors
///
/// The [`io::Error`] of opening or reading it.
pub fn digest_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(hex(&hasher.finalize()))
}

/// Lower-case hex sha256 of `bytes`.
pub fn digest_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut out, byte| {
            if write!(out, "{byte:02x}").is_err() {
                log::debug!("formatting a digest failed");
            }
            out
        },
    )
}

#[cfg(unix)]
mod sys {
    //! Unix: ownership by uid, modes by `chmod`; no shell, no `unsafe` (the effective uid is read off a file this
    //! process just created).

    use std::{
        fs, io,
        os::unix::fs::{
            DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
        },
        path::Path,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{Access, Kind, PrivilegedError, Trust, invalid, io_error};

    /// A distinct name for each probe file within this process.
    static PROBE_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Who the directories on the way must belong to.
    #[derive(Debug)]
    pub struct Owner {
        uid: u32,
        root: bool,
    }

    /// The uid this process creates files as.
    fn effective_uid() -> Result<u32, PrivilegedError> {
        let sequence = PROBE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let probe =
            std::env::temp_dir().join(format!("etui-uid-probe-{}-{sequence}", std::process::id()));
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&probe)
            .map_err(io_error("reading the effective user"))?;
        let uid = file
            .metadata()
            .map_err(io_error("reading the effective user"))?
            .uid();
        drop(file);
        if let Err(error) = fs::remove_file(&probe) {
            log::debug!("could not remove {}: {error}", probe.display());
        }
        Ok(uid)
    }

    impl Owner {
        pub fn new(trust: Trust) -> Result<Self, PrivilegedError> {
            let uid = effective_uid()?;
            match trust {
                Trust::Root if uid != 0 => {
                    Err(invalid("this step must run with administrator rights"))
                }
                Trust::Root => Ok(Self { uid, root: true }),
                #[cfg(test)]
                Trust::CurrentUser => Ok(Self { uid, root: false }),
            }
        }

        /// Makes `path` (just created, or sealed) belong to root's group as well.
        fn claim(&self, path: &Path) -> Result<(), PrivilegedError> {
            if self.root {
                std::os::unix::fs::chown(path, Some(0), Some(0))
                    .map_err(io_error(format!("taking ownership of {}", path.display())))?;
            }
            Ok(())
        }
    }

    pub fn ensure_dir(owner: &Owner, path: &Path, _access: Access) -> Result<(), PrivilegedError> {
        match fs::symlink_metadata(path) {
            Ok(meta) => {
                let others_can_write = meta.permissions().mode() & 0o022 != 0;
                if meta.file_type().is_dir() && meta.uid() == owner.uid && !others_can_write {
                    Ok(())
                } else {
                    Err(invalid(format!(
                        "{} is not a directory only the administrator controls",
                        path.display()
                    )))
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::DirBuilder::new()
                    .mode(0o755)
                    .create(path)
                    .map_err(io_error(format!("creating {}", path.display())))?;
                fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                    .map_err(io_error(format!("securing {}", path.display())))?;
                owner.claim(path)
            }
            Err(error) => Err(io_error(format!("reading {}", path.display()))(error)),
        }
    }

    pub fn create_file(
        _owner: &Owner,
        path: &Path,
        _kind: Kind,
    ) -> Result<fs::File, PrivilegedError> {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(io_error(format!("creating {}", path.display())))
    }

    pub fn seal_file(owner: &Owner, path: &Path, kind: Kind) -> Result<(), PrivilegedError> {
        let mode = match kind {
            Kind::Program => 0o755,
            Kind::Config => 0o600,
        };
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(io_error(format!("securing {}", path.display())))?;
        owner.claim(path)
    }
}

#[cfg(windows)]
mod sys {
    //! Windows: the directories are owned by Administrators and locked with `icacls.exe` (fixed arguments, well-known
    //! SIDs so the account names' language does not matter); a reparse point is never followed.

    use std::{
        ffi::OsString,
        fs, io,
        os::windows::fs::MetadataExt as _,
        path::{Path, PathBuf},
        time::Duration,
    };

    use super::{Access, Kind, PrivilegedError, Trust, invalid, io_error};
    use crate::core::sidecar::run;

    const REPARSE_POINT: u32 = 0x400;
    const ICACLS_TIMEOUT: Duration = Duration::from_secs(30);
    const SYSTEM: &str = "*S-1-5-18";
    const ADMINISTRATORS: &str = "*S-1-5-32-544";
    const USERS: &str = "*S-1-5-32-545";

    /// Nothing to remember on Windows: ownership is set with the ACL.
    #[derive(Debug)]
    pub struct Owner;

    impl Owner {
        pub fn new(_trust: Trust) -> Result<Self, PrivilegedError> {
            Ok(Self)
        }
    }

    fn icacls_path() -> PathBuf {
        let root = std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        root.join("System32").join("icacls.exe")
    }

    fn icacls(args: Vec<OsString>) -> Result<(), PrivilegedError> {
        let captured = run::run(run::command(&icacls_path(), args), ICACLS_TIMEOUT)
            .map_err(|error| invalid(error.to_string()))?;
        if captured.success() {
            Ok(())
        } else {
            Err(invalid(format!(
                "icacls failed: {}",
                captured.stderr.trim().chars().take(300).collect::<String>()
            )))
        }
    }

    fn os(text: &str) -> OsString {
        OsString::from(text)
    }

    /// Owner Administrators; SYSTEM and Administrators full control; Users may only list (and read `logs`).
    fn lock_down(path: &Path, access: Access) -> Result<(), PrivilegedError> {
        icacls(vec![
            path.as_os_str().to_owned(),
            os("/setowner"),
            os(ADMINISTRATORS),
        ])?;
        icacls(vec![path.as_os_str().to_owned(), os("/reset")])?;
        let users = match access {
            Access::Traverse => format!("{USERS}:(RX)"),
            Access::Read => format!("{USERS}:(OI)(CI)RX"),
        };
        icacls(vec![
            path.as_os_str().to_owned(),
            os("/inheritance:r"),
            os("/grant:r"),
            OsString::from(format!("{SYSTEM}:(OI)(CI)F")),
            os("/grant:r"),
            OsString::from(format!("{ADMINISTRATORS}:(OI)(CI)F")),
            os("/grant:r"),
            OsString::from(users),
        ])
    }

    pub fn ensure_dir(_owner: &Owner, path: &Path, access: Access) -> Result<(), PrivilegedError> {
        match fs::symlink_metadata(path) {
            Ok(meta) => {
                if !meta.is_dir() || meta.file_attributes() & REPARSE_POINT != 0 {
                    return Err(invalid(format!(
                        "{} is not a plain directory",
                        path.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(path).map_err(io_error(format!("creating {}", path.display())))?;
            }
            Err(error) => return Err(io_error(format!("reading {}", path.display()))(error)),
        }
        lock_down(path, access)
    }

    pub fn create_file(
        _owner: &Owner,
        path: &Path,
        _kind: Kind,
    ) -> Result<fs::File, PrivilegedError> {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(io_error(format!("creating {}", path.display())))
    }

    pub fn seal_file(_owner: &Owner, _path: &Path, _kind: Kind) -> Result<(), PrivilegedError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    fn digest(text: &str) -> String {
        digest_bytes(text.as_bytes())
    }

    fn stage_request(base: &Path, key: &str, then: &[&str]) -> Request {
        Request::Stage {
            base: base.to_path_buf(),
            key: key.to_owned(),
            core: Source {
                path: PathBuf::from("/u/cores/2.6.4/easytier-core"),
                sha256: digest("core"),
            },
            cli: Source {
                path: PathBuf::from("/u/cores/2.6.4/easytier-cli"),
                sha256: digest("cli"),
            },
            config: Source {
                path: PathBuf::from("/u/instances/p-a1/config.toml"),
                sha256: digest("config"),
            },
            then: args(then),
        }
    }

    #[test]
    fn a_request_round_trips_through_its_arguments() {
        let request = stage_request(
            Path::new("/Library/Application Support/EasyTierUI/services"),
            "p-a1",
            &[
                "service",
                "--name",
                "easytierui-p-a1",
                "install",
                "--",
                "-c",
                "x y",
            ],
        );
        let all = request.to_args();
        assert_eq!(
            all.first().map(OsString::as_os_str),
            Some(std::ffi::OsStr::new(FLAG))
        );
        assert_eq!(Request::parse(&all[1..]).unwrap(), request);

        let purge = Request::Purge {
            base: PathBuf::from("/var/lib/easytierui"),
            key: "p-a1".to_owned(),
            then: args(&["service", "--name", "easytierui-p-a1", "uninstall"]),
        };
        assert_eq!(Request::parse(&purge.to_args()[1..]).unwrap(), purge);
    }

    #[test]
    fn requests_with_bad_values_are_refused() {
        let good = stage_request(Path::new("/var/lib/easytierui"), "p-a1", &[]).to_args();
        for (index, bad) in [(3, "../../etc"), (3, ""), (3, "p a"), (1, "unstage")] {
            let mut changed = good.clone();
            changed[index] = OsString::from(bad);
            assert!(Request::parse(&changed[1..]).is_err(), "{bad}");
        }
        let mut short_digest = good.clone();
        short_digest[5] = OsString::from("abc");
        assert!(Request::parse(&short_digest[1..]).is_err());
        let mut upper = good.clone();
        upper[5] = OsString::from("A".repeat(64));
        assert!(Request::parse(&upper[1..]).is_err());
        let without_separator: Vec<OsString> = good[1..good.len() - 1].to_vec();
        assert!(Request::parse(&without_separator).is_err());
        assert!(Request::parse(&[]).is_err());
    }

    #[test]
    fn only_the_fixed_directory_is_accepted() {
        let request = stage_request(Path::new("/tmp/attacker"), "p-a1", &[]);
        let error = execute(&request).unwrap_err();
        assert!(matches!(error, PrivilegedError::Invalid(_)), "{error}");
    }

    #[test]
    fn a_normal_launch_is_not_a_request() {
        assert_eq!(dispatch(args(&["easytierui"])), None);
        assert_eq!(dispatch(args(&["easytierui", "--other"])), None);
        assert_eq!(dispatch(Vec::new()), None);
        // A request that cannot be read is answered, never ignored.
        assert_eq!(dispatch(args(&["easytierui", FLAG, "stage"])), Some(1));
    }

    #[test]
    fn the_service_layout_is_under_the_instance_directory() {
        let files = ServiceFiles::new(Path::new("/var/lib/easytierui"), "p-a1");
        assert_eq!(files.dir, Path::new("/var/lib/easytierui/p-a1"));
        assert_eq!(
            files.config,
            Path::new("/var/lib/easytierui/p-a1/config.toml")
        );
        assert_eq!(files.logs, Path::new("/var/lib/easytierui/p-a1/logs"));
        assert!(files.core.starts_with(&files.dir) && files.cli.starts_with(&files.dir));
    }

    #[test]
    fn the_system_locations_are_the_documented_ones() {
        let location = system_location().unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(
                location.base,
                Path::new("/Library/Application Support/EasyTierUI/services")
            );
        } else if cfg!(target_os = "linux") {
            assert_eq!(location.base, Path::new("/var/lib/easytierui"));
        }
        assert!(location.base.starts_with(&location.anchor));
    }

    #[cfg(unix)]
    mod on_disk {
        use std::os::unix::fs::PermissionsExt as _;

        use super::*;

        struct Fixture {
            root: PathBuf,
            location: Location,
            core: Source,
            cli: Source,
            config: Source,
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                if let Err(error) = fs::remove_dir_all(&self.root) {
                    log::debug!("could not remove the fixture: {error}");
                }
            }
        }

        fn write_source(dir: &Path, name: &str, body: &str) -> Source {
            let path = dir.join(name);
            fs::write(&path, body).unwrap();
            Source {
                path,
                sha256: digest(body),
            }
        }

        fn fixture(name: &str, cli_body: &str) -> Fixture {
            let root =
                std::env::temp_dir().join(format!("etui-priv-{name}-{}", std::process::id()));
            if root.exists() {
                fs::remove_dir_all(&root).unwrap();
            }
            let user = root.join("user");
            let anchor = root.join("anchor");
            fs::create_dir_all(&user).unwrap();
            fs::create_dir_all(&anchor).unwrap();
            let location = Location {
                base: anchor.join("easytierui"),
                anchor,
            };
            let core = write_source(&user, "easytier-core", "core body");
            let cli = write_source(&user, "easytier-cli", cli_body);
            let config = write_source(
                &user,
                "config.toml",
                "network_secret = \"example-secret-do-not-use\"\n",
            );
            Fixture {
                root,
                location,
                core,
                cli,
                config,
            }
        }

        fn stage_of(f: &Fixture, then: &[&str]) -> Request {
            Request::Stage {
                base: f.location.base.clone(),
                key: "p-a1".to_owned(),
                core: f.core.clone(),
                cli: f.cli.clone(),
                config: f.config.clone(),
                then: args(then),
            }
        }

        const ECHO_CLI: &str = "#!/bin/sh\necho \"ran: $*\"\n";

        #[test]
        fn staging_copies_verifies_seals_and_runs_the_copied_cli() {
            let f = fixture("stage", ECHO_CLI);
            let outcome = apply(
                &stage_of(&f, &["service", "start"]),
                &f.location,
                Trust::CurrentUser,
            )
            .unwrap();
            assert_eq!(outcome.code, 0, "{}", outcome.output);
            assert_eq!(outcome.output, "ran: service start");

            let files = ServiceFiles::new(&f.location.base, "p-a1");
            assert_eq!(fs::read_to_string(&files.core).unwrap(), "core body");
            assert_eq!(fs::read_to_string(&files.cli).unwrap(), ECHO_CLI);
            assert_eq!(digest_file(&files.config).unwrap(), f.config.sha256);
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&files.core), 0o755);
            assert_eq!(mode(&files.cli), 0o755);
            assert_eq!(mode(&files.config), 0o600);
            assert_eq!(mode(&files.dir), 0o755);
            assert_eq!(mode(&files.logs), 0o755);
            let leftovers: Vec<String> = fs::read_dir(&files.dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with(".new-"))
                .collect();
            assert!(leftovers.is_empty(), "{leftovers:?}");

            // Staging again replaces the files (a config change) and still works.
            fs::write(&f.config.path, "changed").unwrap();
            let mut again = stage_of(&f, &[]);
            if let Request::Stage { config, .. } = &mut again {
                config.sha256 = digest("changed");
            }
            apply(&again, &f.location, Trust::CurrentUser).unwrap();
            assert_eq!(fs::read_to_string(&files.config).unwrap(), "changed");
        }

        #[test]
        fn a_file_swapped_after_the_digest_was_taken_is_refused_and_nothing_is_installed() {
            let f = fixture("swap", ECHO_CLI);
            fs::write(&f.core.path, "malicious core").unwrap();
            let error = apply(
                &stage_of(&f, &["service", "start"]),
                &f.location,
                Trust::CurrentUser,
            )
            .unwrap_err();
            assert!(
                matches!(&error, PrivilegedError::Digest(name) if name == "easytier-core"),
                "{error}"
            );
            let files = ServiceFiles::new(&f.location.base, "p-a1");
            assert!(!files.core.exists() && !files.cli.exists() && !files.config.exists());
            let leftovers = fs::read_dir(&files.dir)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with(".new-"))
                .count();
            assert_eq!(leftovers, 0);
        }

        #[test]
        fn a_later_file_failing_removes_the_earlier_temporary_copies() {
            let f = fixture("late", ECHO_CLI);
            fs::write(&f.config.path, "swapped config").unwrap();
            let error = apply(&stage_of(&f, &[]), &f.location, Trust::CurrentUser).unwrap_err();
            assert!(
                matches!(&error, PrivilegedError::Digest(name) if name == "config.toml"),
                "{error}"
            );
            let files = ServiceFiles::new(&f.location.base, "p-a1");
            let names: Vec<String> = fs::read_dir(&files.dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            assert_eq!(names, ["logs"]);
        }

        #[test]
        fn a_directory_others_can_write_is_refused() {
            let f = fixture("open", ECHO_CLI);
            fs::create_dir_all(&f.location.base).unwrap();
            fs::set_permissions(&f.location.base, fs::Permissions::from_mode(0o777)).unwrap();
            let error = apply(&stage_of(&f, &[]), &f.location, Trust::CurrentUser).unwrap_err();
            assert!(matches!(error, PrivilegedError::Invalid(_)), "{error}");
            assert!(!ServiceFiles::new(&f.location.base, "p-a1").core.exists());
        }

        #[test]
        fn a_symlinked_directory_is_refused() {
            let f = fixture("link", ECHO_CLI);
            let elsewhere = f.root.join("elsewhere");
            fs::create_dir_all(&elsewhere).unwrap();
            std::os::unix::fs::symlink(&elsewhere, &f.location.base).unwrap();
            let error = apply(&stage_of(&f, &[]), &f.location, Trust::CurrentUser).unwrap_err();
            assert!(matches!(error, PrivilegedError::Invalid(_)), "{error}");
            assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
        }

        #[test]
        fn purging_uninstalls_through_the_copy_then_deletes_the_directory() {
            let f = fixture("purge", ECHO_CLI);
            apply(&stage_of(&f, &[]), &f.location, Trust::CurrentUser).unwrap();
            let purge = Request::Purge {
                base: f.location.base.clone(),
                key: "p-a1".to_owned(),
                then: args(&["service", "uninstall"]),
            };
            let outcome = apply(&purge, &f.location, Trust::CurrentUser).unwrap();
            assert_eq!(outcome.code, 0);
            assert!(!ServiceFiles::new(&f.location.base, "p-a1").dir.exists());
            // Nothing there: nothing to do.
            assert_eq!(
                apply(&purge, &f.location, Trust::CurrentUser).unwrap().code,
                0
            );
        }

        #[test]
        fn a_failed_uninstall_keeps_the_files_but_a_missing_service_does_not() {
            let failing = "#!/bin/sh\necho 'access denied' >&2\nexit 3\n";
            let f = fixture("purge-fail", failing);
            apply(&stage_of(&f, &[]), &f.location, Trust::CurrentUser).unwrap();
            let purge = Request::Purge {
                base: f.location.base.clone(),
                key: "p-a1".to_owned(),
                then: args(&["service", "uninstall"]),
            };
            let outcome = apply(&purge, &f.location, Trust::CurrentUser).unwrap();
            assert_eq!(outcome.code, 3);
            assert!(outcome.output.contains("access denied"));
            assert!(ServiceFiles::new(&f.location.base, "p-a1").dir.exists());

            let gone = "#!/bin/sh\necho 'Error: Service is not installed' >&2\nexit 1\n";
            let g = fixture("purge-gone", gone);
            apply(&stage_of(&g, &[]), &g.location, Trust::CurrentUser).unwrap();
            let purge = Request::Purge {
                base: g.location.base.clone(),
                key: "p-a1".to_owned(),
                then: args(&["service", "uninstall"]),
            };
            assert_eq!(
                apply(&purge, &g.location, Trust::CurrentUser).unwrap().code,
                0
            );
            assert!(!ServiceFiles::new(&g.location.base, "p-a1").dir.exists());
        }

        #[test]
        fn the_real_entry_point_needs_administrator_rights() {
            // Tests do not run as root: the real trust level refuses (and never touches the disk).
            if sys::Owner::new(Trust::Root).is_ok() {
                return;
            }
            let f = fixture("root", ECHO_CLI);
            let error = apply(&stage_of(&f, &[]), &f.location, Trust::Root).unwrap_err();
            assert!(matches!(error, PrivilegedError::Invalid(_)), "{error}");
            assert!(!f.location.base.exists());
        }
    }
}
