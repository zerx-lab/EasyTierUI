//! `sidecar`: the node role on the desktops, driving the official `easytier-core` / `easytier-cli` release as
//! separate processes (`rule://easytier-core-boundary`).
//!
//! * **`Privilege::User`** (no TUN, and every managed instance): a child of the app, supervised by a thread that
//!   captures its redacted output, restarts it with capped exponential backoff while the user wants it up, and kills
//!   it on stop, shutdown and drop.
//! * **`Privilege::System`** (TUN): an OS service named `easytierui-<key>`, installed with `easytier-cli service …`
//!   through the OS's own administrator prompt (see [`elevate`]). It outlives the app. The service is installed
//!   *without* start-at-boot by default (PRODUCT.md leaves "run the core as a service by default" open, and an
//!   imported or forgotten profile must never bring a VPN up on its own): the app is the switch. Pass
//!   [`SidecarBackend::with_service_autostart`] to change that. What the service runs never lives where the user
//!   can write: the elevated step ([`privileged`]) copies `easytier-core`, `easytier-cli` and the rendered config into
//!   an administrator-only directory, checks the copies against the digests taken just before the prompt, points the
//!   service at them, and every later `service start / stop / uninstall` runs the copied CLI.
//! * **State** is read through `easytier-cli -p 127.0.0.1:<port> -o json node|peer` on the loopback RPC portal; the
//!   adapter is [`parse`]. CLI output is never logged (`node` prints the whole config, secret included).
//!
//! Everything blocks; the store calls it through `core::work::off_thread`.

mod argv;
mod elevate;
mod instance;
mod parse;
pub mod privileged;
mod redact;
mod run;
mod service;
mod supervisor;

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

pub use elevate::{DEFAULT_PROMPT, Elevate, ElevatedOutput, SystemElevator};
use instance::{InstancePaths, Meta, MetaPrivilege, digest_hex, io_error, read_meta, write_meta};
use nsh_core::time::{Instant, now, unix_ms};
use privileged::{Location, Request, ServiceFiles, Source};
use redact::Redactor;
use service::{OnFailure, ServiceState};
use supervisor::{Phase, Spec, Supervisor, Timing, lock};

use super::{
    node::{
        InstanceKey, LaunchSource, NodeBackend, NodeError, NodeLaunch, Privilege, ProgramSource,
        Programs,
    },
    paths,
    secret::Secret,
    state::{FailReason, LogLine, NetworkState, NodeSnapshot},
};

/// A CLI read is abandoned (the process killed) after this long.
const CLI_TIMEOUT: Duration = Duration::from_secs(3);

/// `--check-config` is abandoned after this long.
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);

/// How long a fresh child may take to open its portal before the silence counts as a failure.
const CHILD_START_GRACE: Duration = Duration::from_secs(20);

/// The same for an OS service (it also waits out the service manager).
const SERVICE_START_GRACE: Duration = Duration::from_secs(30);

/// The longest message quoted from a failing `--check-config`.
const MAX_CHECK_MESSAGE_CHARS: usize = 2000;

/// A distinct name for each `--check-config` temp file within this process.
static CHECK_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// What the app knows about an instance's OS service beyond `meta.json`.
#[derive(Debug, Clone, Copy)]
struct ServiceRun {
    /// The user asked for it to be up (cleared by `stop` or a failed start).
    wanted: bool,
    /// When the start was requested or completed.
    since: Instant,
}

#[derive(Default)]
struct State {
    supervisors: HashMap<InstanceKey, Supervisor>,
    /// The `easytier-cli` of the release each instance was started with (a version switch applies to the next start).
    cli: HashMap<InstanceKey, PathBuf>,
    services: HashMap<InstanceKey, ServiceRun>,
    /// Serializes start / stop / remove of one instance (an administrator prompt can take minutes).
    op_locks: HashMap<InstanceKey, Arc<Mutex<()>>>,
}

/// The node backend of the desktops.
pub struct SidecarBackend {
    programs: Arc<dyn ProgramSource>,
    elevator: Arc<dyn Elevate>,
    /// Instances live here instead of `<data dir>/instances` (tests).
    root: Option<PathBuf>,
    /// The administrator-only service directories live here instead of the system's (tests).
    service_location: Option<Location>,
    service_autostart: bool,
    timing: Timing,
    state: Mutex<State>,
}

impl SidecarBackend {
    /// A backend that finds the EasyTier programs through `programs` at each start.
    pub fn new(programs: Arc<dyn ProgramSource>) -> Self {
        Self {
            programs,
            elevator: Arc::new(SystemElevator::default()),
            root: None,
            service_location: None,
            service_autostart: false,
            timing: Timing::DEFAULT,
            state: Mutex::new(State::default()),
        }
    }

    /// Whether a TUN instance's OS service also starts at boot (default: no, see the module docs).
    #[must_use]
    pub fn with_service_autostart(mut self, autostart: bool) -> Self {
        self.service_autostart = autostart;
        self
    }

    /// The text of the macOS administrator prompt, localized by the caller.
    #[must_use]
    pub fn with_elevation_prompt(mut self, prompt: &str) -> Self {
        self.elevator = Arc::new(SystemElevator::new(prompt));
        self
    }

    /// Keeps instances under `root` instead of the data directory.
    #[cfg(test)]
    #[must_use]
    fn with_root(mut self, root: PathBuf) -> Self {
        self.root = Some(root);
        self
    }

    /// Keeps the administrator-only service directories under `location` instead of the system's.
    #[cfg(test)]
    #[must_use]
    fn with_service_location(mut self, location: Location) -> Self {
        self.service_location = Some(location);
        self
    }

    /// Replaces the administrator prompt (tests record instead of elevating).
    #[cfg(test)]
    #[must_use]
    fn with_elevator(mut self, elevator: Arc<dyn Elevate>) -> Self {
        self.elevator = elevator;
        self
    }

    /// Replaces the restart policy (tests shrink the delays).
    #[cfg(test)]
    #[must_use]
    fn with_timing(mut self, timing: Timing) -> Self {
        self.timing = timing;
        self
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        lock(&self.state)
    }

    fn instances_root(&self) -> Result<PathBuf, NodeError> {
        match &self.root {
            Some(root) => {
                paths::ensure_private_dir(root).map_err(io_error("the instances directory"))?;
                Ok(root.clone())
            }
            None => {
                paths::data_subdir("instances").map_err(|error| NodeError::Io(error.to_string()))
            }
        }
    }

    fn paths_of(&self, key: &InstanceKey) -> Result<(PathBuf, InstancePaths), NodeError> {
        let root = self.instances_root()?;
        let paths = InstancePaths::new(&root, key);
        Ok((root, paths))
    }

    /// Where the administrator-only service directories are.
    fn service_location(&self) -> Result<Location, NodeError> {
        match &self.service_location {
            Some(location) => Ok(location.clone()),
            None => privileged::system_location().ok_or(NodeError::Unsupported(
                "an operating-system service is not available here",
            )),
        }
    }

    /// The service's log directory when it runs (it writes in its own directory), else the instance's.
    fn log_dir(&self, key: &InstanceKey, paths: &InstancePaths, service: bool) -> PathBuf {
        if service && let Ok(location) = self.service_location() {
            return ServiceFiles::new(&location.base, key.as_str()).logs;
        }
        paths.logs.clone()
    }

    /// The active release, if it is new enough.
    fn gate(&self) -> Result<Programs, NodeError> {
        let programs = self.programs.active()?;
        if !programs.version.is_supported() {
            return Err(NodeError::CoreUnsupported(programs.version));
        }
        if !programs.core.is_file() {
            return Err(NodeError::CoreMissing);
        }
        Ok(programs)
    }

    fn op_lock(&self, key: &InstanceKey) -> Arc<Mutex<()>> {
        Arc::clone(
            self.state()
                .op_locks
                .entry(key.clone())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    /// Takes the supervisor out of the table; the caller stops it (outside the table's lock).
    fn take_supervisor(&self, key: &InstanceKey) -> Option<Supervisor> {
        self.state().supervisors.remove(key)
    }

    fn set_service_run(&self, key: &InstanceKey, wanted: bool) {
        self.state().services.insert(
            key.clone(),
            ServiceRun {
                wanted,
                since: now(),
            },
        );
    }

    /// The `easytier-cli` to read `key` with: the one it started with, else the active release's.
    fn cli_for(&self, key: &InstanceKey) -> Result<PathBuf, NodeError> {
        if let Some(cli) = self.state().cli.get(key) {
            return Ok(cli.clone());
        }
        let programs = self.programs.active()?;
        self.state().cli.insert(key.clone(), programs.cli.clone());
        Ok(programs.cli)
    }

    // ---- start ---------------------------------------------------------------------------------------------

    fn start_user(
        &self,
        launch: &NodeLaunch,
        programs: &Programs,
        paths: &InstancePaths,
        port: u16,
        mut meta: Meta,
    ) -> Result<(), NodeError> {
        if meta.service_installed {
            return Err(NodeError::Service(
                "this network is installed as a system service; remove that first to run it as the current user"
                    .to_owned(),
            ));
        }
        let (child, redactor, digest) = match &launch.source {
            LaunchSource::Local { config_toml } => {
                let digest = write_config(paths, config_toml)?;
                (
                    argv::local_child(&paths.config, port),
                    Redactor::new().with_config_toml(config_toml.expose()),
                    digest,
                )
            }
            LaunchSource::Managed {
                config_server,
                machine_id,
                hostname,
            } => {
                // A managed instance has no local config; do not leave an old one (with a secret) behind.
                paths::remove_if_exists(&paths.config).map_err(io_error("config.toml"))?;
                (
                    argv::managed_child(config_server, machine_id, hostname.as_deref(), port)?,
                    Redactor::new().with_secret(config_server.expose()),
                    digest_hex(format!("managed:{machine_id}").as_bytes()),
                )
            }
        };
        // A restart: the old child must be gone before the new one binds the same portal.
        if let Some(old) = self.take_supervisor(&launch.key) {
            old.stop();
        }
        let spec = Spec {
            program: programs.core.clone(),
            args: child.args,
            config_server: child.config_server,
            work_dir: paths.dir.clone(),
        };
        let supervisor = Supervisor::start(spec, redactor, self.timing).map_err(spawn_error)?;
        meta.privilege = Some(MetaPrivilege::User);
        meta.config_digest = Some(digest);
        if let Err(error) = write_meta(paths, &meta) {
            supervisor.stop();
            return Err(error);
        }
        let mut state = self.state();
        state.cli.insert(launch.key.clone(), programs.cli.clone());
        state.services.remove(&launch.key);
        state.supervisors.insert(launch.key.clone(), supervisor);
        Ok(())
    }

    /// The three files a service is staged from, with the digests taken now (the elevated step checks the copies
    /// against them), and the digest of the three together.
    fn service_sources(
        programs: &Programs,
        paths: &InstancePaths,
        config_digest: &str,
    ) -> Result<([Source; 3], String), NodeError> {
        let hashed = |path: &Path, what: &'static str| {
            privileged::digest_file(path)
                .map(|sha256| Source {
                    path: path.to_path_buf(),
                    sha256,
                })
                .map_err(io_error(what))
        };
        let core = hashed(&programs.core, "easytier-core")?;
        let cli = hashed(&programs.cli, "easytier-cli")?;
        let config = Source {
            path: paths.config.clone(),
            sha256: config_digest.to_owned(),
        };
        let together =
            digest_hex(format!("{}\0{}\0{}", core.sha256, cli.sha256, config.sha256).as_bytes());
        Ok(([core, cli, config], together))
    }

    fn start_system(
        &self,
        launch: &NodeLaunch,
        programs: &Programs,
        paths: &InstancePaths,
        port: u16,
        mut meta: Meta,
    ) -> Result<(), NodeError> {
        let LaunchSource::Local { config_toml } = &launch.source else {
            return Err(NodeError::Unsupported(
                "a managed network runs as the current user",
            ));
        };
        let location = self.service_location()?;
        let files = ServiceFiles::new(&location.base, launch.key.as_str());
        let service = argv::service_name(launch.key.as_str());
        let display_name = format!("EasyTierUI {}", launch.key);
        // Everything the service runs is the administrator-only copy; none of it is the user's file.
        let install = argv::ServiceInstall {
            service: &service,
            display_name: &display_name,
            core: &files.core,
            work_dir: &files.dir,
            config: &files.config,
            log_dir: &files.logs,
            port,
            autostart: self.service_autostart,
        };
        let fingerprint = digest_hex(
            format!(
                "{}\0{}\0{}",
                files.dir.display(),
                port,
                self.service_autostart
            )
            .as_bytes(),
        );
        argv::check_service_paths(&install, cfg!(target_os = "linux"))?;
        let digest = write_config(paths, config_toml)?;
        let (sources, files_digest) = Self::service_sources(programs, paths, &digest)?;
        // The service and a child would fight over the portal and the interface.
        if let Some(old) = self.take_supervisor(&launch.key) {
            old.stop();
        }
        let hint = service::query_status(&programs.cli, &service);
        let needs_install = hint == Some(ServiceState::NotInstalled)
            || !(meta.service_installed
                && meta.service_fingerprint.as_deref() == Some(&fingerprint));
        let needs_stage =
            needs_install || meta.service_files_digest.as_deref() != Some(&files_digest);

        self.state()
            .cli
            .insert(launch.key.clone(), programs.cli.clone());
        self.set_service_run(&launch.key, true);
        let result = (|| {
            let elevator = self.elevator.as_ref();
            let start = argv::service_verb(&service, argv::ServiceVerb::Start);
            // A running service must stop first for the new files (or the new install) to take effect.
            match hint {
                Some(ServiceState::Running) => service::run_elevated(
                    elevator,
                    &files.cli,
                    &argv::service_verb(&service, argv::ServiceVerb::Stop),
                    OnFailure::Tolerate(service::STOP_TOLERATES),
                )?,
                None if meta.service_installed => service::run_elevated(
                    elevator,
                    &files.cli,
                    &argv::service_verb(&service, argv::ServiceVerb::Stop),
                    OnFailure::Ignore,
                )?,
                _ => {}
            }
            let mut started = false;
            if needs_stage {
                // One prompt: copy and verify, then run the copied CLI (the install, or the start when only the
                // files changed).
                let [core, cli, config] = sources;
                let (then, on_failure) = if needs_install {
                    (argv::service_install(&install), OnFailure::Fail)
                } else {
                    (start.clone(), OnFailure::Tolerate(service::START_TOLERATES))
                };
                let request = Request::Stage {
                    base: location.base.clone(),
                    key: launch.key.as_str().to_owned(),
                    core,
                    cli,
                    config,
                    then,
                };
                let (program, args) = request.invocation()?;
                service::run_elevated(elevator, &program, &args, on_failure)?;
                started = !needs_install;
                meta.service_files_digest = Some(files_digest.clone());
                if needs_install {
                    meta.service_installed = true;
                    meta.service_fingerprint = Some(fingerprint.clone());
                }
                meta.privilege = Some(MetaPrivilege::System);
                meta.config_digest = Some(digest.clone());
                write_meta(paths, &meta)?;
            }
            if !started {
                service::run_elevated(
                    elevator,
                    &files.cli,
                    &start,
                    OnFailure::Tolerate(service::START_TOLERATES),
                )?;
            }
            meta.privilege = Some(MetaPrivilege::System);
            meta.config_digest = Some(digest.clone());
            write_meta(paths, &meta)
        })();
        match result {
            Ok(()) => {
                self.set_service_run(&launch.key, true);
                Ok(())
            }
            Err(error) => {
                self.set_service_run(&launch.key, false);
                Err(error)
            }
        }
    }

    // ---- poll ----------------------------------------------------------------------------------------------

    fn poll_child(&self, key: &InstanceKey, paths: &InstancePaths, phase: Phase) -> NetworkState {
        match phase {
            Phase::Done => NetworkState::Stopped,
            Phase::GaveUp(reason) => NetworkState::Failed { reason },
            Phase::Backoff { code, last_log } => NetworkState::Failed {
                reason: FailReason::Exited { code, last_log },
            },
            Phase::Up { since } => {
                let cli = match self.cli_for(key) {
                    Ok(cli) => cli,
                    Err(error) => {
                        return NetworkState::Failed {
                            reason: error.into_fail_reason(),
                        };
                    }
                };
                let port = read_meta(paths).rpc_port;
                match read_snapshot(&cli, port) {
                    Ok(snapshot) => NetworkState::Running(Box::new(snapshot)),
                    Err(error) if since.elapsed() < CHILD_START_GRACE => {
                        log::debug!("the core does not answer yet: {error}");
                        NetworkState::Starting
                    }
                    Err(error) => NetworkState::Failed {
                        reason: FailReason::Unreachable(error.to_string()),
                    },
                }
            }
        }
    }

    fn poll_service(
        &self,
        key: &InstanceKey,
        paths: &InstancePaths,
        mut meta: Meta,
    ) -> NetworkState {
        let cli = match self.cli_for(key) {
            Ok(cli) => cli,
            Err(error) => {
                return NetworkState::Failed {
                    reason: error.into_fail_reason(),
                };
            }
        };
        let error = match read_snapshot(&cli, meta.rpc_port) {
            Ok(snapshot) => return NetworkState::Running(Box::new(snapshot)),
            Err(error) => error,
        };
        let run = self.state().services.get(key).copied();
        let wanted = run.is_some_and(|run| run.wanted);
        if run.is_some_and(|run| run.wanted && run.since.elapsed() < SERVICE_START_GRACE) {
            return NetworkState::Starting;
        }
        let service = argv::service_name(key.as_str());
        match (service::query_status(&cli, &service), wanted) {
            (Some(ServiceState::NotInstalled), _) => {
                // Removed behind our back: forget it.
                meta.service_installed = false;
                meta.service_fingerprint = None;
                if let Err(write_error) = write_meta(paths, &meta) {
                    log::warn!("could not update the instance record: {write_error}");
                }
                self.state().services.remove(key);
                NetworkState::Stopped
            }
            (Some(ServiceState::Stopped) | None, false) => NetworkState::Stopped,
            (Some(ServiceState::Stopped), true) => NetworkState::Failed {
                reason: FailReason::Exited {
                    code: None,
                    last_log: tail_text(
                        &instance::tail_log_files(
                            &self.log_dir(key, paths, true),
                            &self.redactor_for(paths),
                        ),
                        20,
                    ),
                },
            },
            (Some(ServiceState::Running) | None, _) => NetworkState::Failed {
                reason: FailReason::Unreachable(error.to_string()),
            },
        }
    }

    /// A redactor that knows the secrets in the instance's config file, if there is one.
    fn redactor_for(&self, paths: &InstancePaths) -> Redactor {
        match fs::read_to_string(&paths.config) {
            Ok(config) => Redactor::new().with_config_toml(&config),
            Err(_) => Redactor::new(),
        }
    }
}

impl NodeBackend for SidecarBackend {
    fn check(&self, launch: &NodeLaunch) -> Result<(), NodeError> {
        let programs = self.gate()?;
        // A managed instance's config lives on the server; only the version gate applies.
        let LaunchSource::Local { config_toml } = &launch.source else {
            return Ok(());
        };
        let (_, paths) = self.paths_of(&launch.key)?;
        paths.create()?;
        let sequence = CHECK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let file = paths
            .dir
            .join(format!("check-{}-{sequence}.toml", std::process::id()));
        paths::write_private_file(&file, config_toml.expose().as_bytes())
            .map_err(io_error("the config to check"))?;
        let outcome = run_check(&programs.core, &file, config_toml);
        if let Err(error) = paths::remove_if_exists(&file) {
            log::warn!("could not remove {}: {error}", file.display());
        }
        outcome
    }

    fn start(&self, launch: &NodeLaunch) -> Result<(), NodeError> {
        let programs = self.gate()?;
        let operation = self.op_lock(&launch.key);
        let _serial = lock(&operation);
        let (root, paths) = self.paths_of(&launch.key)?;
        paths.create()?;
        let (port, meta) = instance::ensure_port(&root, &paths, &launch.key)?;
        match launch.privilege {
            Privilege::User => self.start_user(launch, &programs, &paths, port, meta),
            Privilege::System => self.start_system(launch, &programs, &paths, port, meta),
        }
    }

    fn stop(&self, key: &InstanceKey) -> Result<(), NodeError> {
        let operation = self.op_lock(key);
        let _serial = lock(&operation);
        if let Some(supervisor) = self.take_supervisor(key) {
            supervisor.stop();
        }
        let (_, paths) = self.paths_of(key)?;
        let meta = read_meta(&paths);
        if !meta.service_installed {
            return Ok(());
        }
        let programs = self.programs.active()?;
        let location = self.service_location()?;
        let files = ServiceFiles::new(&location.base, key.as_str());
        let service = argv::service_name(key.as_str());
        // Nothing to stop, nothing to ask: skip the prompt when the manager says so.
        let hint = service::query_status(&programs.cli, &service);
        if matches!(
            hint,
            Some(ServiceState::Stopped | ServiceState::NotInstalled)
        ) {
            self.set_service_run(key, false);
            return Ok(());
        }
        // The administrator-only copy of the CLI, never the user's.
        let result = service::run_elevated(
            self.elevator.as_ref(),
            &files.cli,
            &argv::service_verb(&service, argv::ServiceVerb::Stop),
            OnFailure::Tolerate(service::STOP_TOLERATES),
        );
        if result.is_ok() {
            self.set_service_run(key, false);
        }
        result
    }

    fn remove(&self, key: &InstanceKey) -> Result<(), NodeError> {
        let operation = self.op_lock(key);
        let _serial = lock(&operation);
        if let Some(supervisor) = self.take_supervisor(key) {
            supervisor.stop();
        }
        let (_, paths) = self.paths_of(key)?;
        let meta = read_meta(&paths);
        if meta.service_installed || meta.service_files_digest.is_some() {
            // Uninstall through the administrator-only CLI copy, then delete that directory: one prompt. The
            // files are kept if the service cannot be removed (it still points at them).
            let location = self.service_location()?;
            let request = Request::Purge {
                base: location.base,
                key: key.as_str().to_owned(),
                then: argv::service_verb(
                    &argv::service_name(key.as_str()),
                    argv::ServiceVerb::Uninstall,
                ),
            };
            let (program, args) = request.invocation()?;
            service::run_elevated(self.elevator.as_ref(), &program, &args, OnFailure::Fail)?;
        }
        {
            let mut state = self.state();
            state.cli.remove(key);
            state.services.remove(key);
        }
        match fs::remove_dir_all(&paths.dir) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(NodeError::Io(format!("the instance directory: {error}"))),
        }
    }

    fn poll(&self, key: &InstanceKey) -> NetworkState {
        let (_, paths) = match self.paths_of(key) {
            Ok(found) => found,
            Err(error) => {
                return NetworkState::Failed {
                    reason: error.into_fail_reason(),
                };
            }
        };
        let phase = self.state().supervisors.get(key).map(Supervisor::phase);
        if let Some(phase) = phase {
            return self.poll_child(key, &paths, phase);
        }
        let meta = read_meta(&paths);
        if meta.service_installed {
            return self.poll_service(key, &paths, meta);
        }
        NetworkState::Stopped
    }

    fn logs(&self, key: &InstanceKey) -> Vec<LogLine> {
        if let Some(lines) = self.state().supervisors.get(key).map(Supervisor::logs) {
            return lines;
        }
        match self.paths_of(key) {
            Ok((_, paths)) => {
                let installed = read_meta(&paths).service_installed;
                instance::tail_log_files(
                    &self.log_dir(key, &paths, installed),
                    &self.redactor_for(&paths),
                )
            }
            Err(error) => {
                log::debug!("no logs for {key}: {error}");
                Vec::new()
            }
        }
    }

    fn service_installed(&self, key: &InstanceKey) -> bool {
        self.paths_of(key)
            .is_ok_and(|(_, paths)| read_meta(&paths).service_installed)
    }

    fn shutdown(&self) {
        let supervisors: Vec<Supervisor> =
            self.state().supervisors.drain().map(|(_, s)| s).collect();
        // Ask all first so the kills overlap, then wait for each.
        for supervisor in &supervisors {
            supervisor.request_stop();
        }
        drop(supervisors);
    }
}

impl Drop for SidecarBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Writes the rendered config (owner-only, atomic) and returns its digest.
fn write_config(paths: &InstancePaths, config: &Secret) -> Result<String, NodeError> {
    paths::write_private_file(&paths.config, config.expose().as_bytes())
        .map_err(io_error("config.toml"))?;
    Ok(digest_hex(config.expose().as_bytes()))
}

fn spawn_error(error: io::Error) -> NodeError {
    if error.kind() == io::ErrorKind::NotFound {
        NodeError::CoreMissing
    } else {
        NodeError::Spawn(error.to_string())
    }
}

/// `easytier-core --check-config -c <file>`.
fn run_check(core: &Path, file: &Path, config: &Secret) -> Result<(), NodeError> {
    let captured = run::run(run::command(core, argv::check_config(file)), CHECK_TIMEOUT)
        .map_err(|error| NodeError::Spawn(error.to_string()))?;
    if captured.success() {
        return Ok(());
    }
    let redactor = Redactor::new().with_config_toml(config.expose());
    let text = if captured.stderr.trim().is_empty() {
        &captured.stdout
    } else {
        &captured.stderr
    };
    let mut message: String = redactor
        .redact(text.trim())
        .chars()
        .take(MAX_CHECK_MESSAGE_CHARS)
        .collect();
    if message.is_empty() {
        message = silent_rejection(config, captured.status.code(), &redactor);
    }
    Err(NodeError::ConfigRejected(message))
}

/// What to tell when `--check-config` fails without printing anything (2.6.4 never initializes its logger in that
/// mode, so the reason is lost): a TOML syntax error is found by parsing the config here (redacted: the parser
/// quotes the offending line), anything else is the bare exit status.
fn silent_rejection(config: &Secret, code: Option<i32>, redactor: &Redactor) -> String {
    if let Err(error) = config.expose().parse::<toml::Table>() {
        return redactor.redact(error.to_string().trim());
    }
    match code {
        Some(code) => {
            format!("the core rejected the config (exit status {code}) without saying why")
        }
        None => "the core was terminated while checking the config".to_owned(),
    }
}

fn tail_text(lines: &[LogLine], count: usize) -> String {
    let skip = lines.len().saturating_sub(count);
    lines
        .iter()
        .skip(skip)
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Why the portal could not be read (never carries CLI output).
#[derive(Debug, thiserror::Error)]
enum ReadError {
    #[error("the management portal on this machine did not answer")]
    Unreachable,
    #[error("the management portal answered something unexpected")]
    Unexpected(#[from] parse::ParseError),
}

/// One `easytier-cli -p 127.0.0.1:<port> -o json <subcommand>` read; its stdout is returned, never logged.
fn cli_json(cli: &Path, port: u16, subcommand: &str) -> Result<String, ReadError> {
    match run::run(
        run::command(cli, argv::cli_read(port, subcommand)),
        CLI_TIMEOUT,
    ) {
        Ok(captured) if captured.success() => Ok(captured.stdout),
        Ok(captured) => {
            log::debug!(
                "easytier-cli {subcommand} exited with {:?}",
                captured.status.code()
            );
            Err(ReadError::Unreachable)
        }
        Err(error) => {
            log::debug!("easytier-cli {subcommand} did not run: {error}");
            Err(ReadError::Unreachable)
        }
    }
}

/// The node and its peers, read with two CLI processes at once.
fn read_snapshot(cli: &Path, port: u16) -> Result<NodeSnapshot, ReadError> {
    if port == 0 {
        return Err(ReadError::Unreachable);
    }
    let (node, peers) = thread::scope(|scope| {
        let peers = thread::Builder::new()
            .name("etui-cli-peer".to_owned())
            .spawn_scoped(scope, || cli_json(cli, port, "peer"));
        let node = cli_json(cli, port, "node");
        let peers = match peers {
            Ok(handle) => handle.join().unwrap_or(Err(ReadError::Unreachable)),
            Err(_) => cli_json(cli, port, "peer"),
        };
        (node, peers)
    });
    Ok(parse::snapshot(&node?, &peers?, unix_ms())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::version::CoreVersion;

    const NODE_A: &str = include_str!("../../../tests/fixtures/easytier-2.6.4/node-a.toml");
    const SECRET: &str = "example-secret-do-not-use";

    struct Fixed(Result<Programs, NodeError>);

    impl ProgramSource for Fixed {
        fn active(&self) -> Result<Programs, NodeError> {
            self.0.clone()
        }
    }

    fn programs_at(dir: &Path, version: &str) -> Programs {
        Programs {
            version: version.parse::<CoreVersion>().unwrap(),
            dir: dir.to_path_buf(),
            core: dir.join("easytier-core"),
            cli: dir.join("easytier-cli"),
            web: dir.join("easytier-web"),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("etui-sidecar-mod-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn launch(privilege: Privilege) -> NodeLaunch {
        NodeLaunch {
            key: InstanceKey::for_profile("a1").unwrap(),
            privilege,
            source: LaunchSource::Local {
                config_toml: Secret::new(NODE_A),
            },
        }
    }

    #[test]
    fn a_missing_or_old_core_stops_every_entry_point() {
        let root = scratch("gate");
        let missing = SidecarBackend::new(Arc::new(Fixed(Err(NodeError::CoreMissing))))
            .with_root(root.join("i"));
        assert_eq!(
            missing.check(&launch(Privilege::User)),
            Err(NodeError::CoreMissing)
        );
        assert_eq!(
            missing.start(&launch(Privilege::User)),
            Err(NodeError::CoreMissing)
        );

        let old = programs_at(&root, "2.5.9");
        let backend = SidecarBackend::new(Arc::new(Fixed(Ok(old)))).with_root(root.join("i"));
        assert!(matches!(
            backend.start(&launch(Privilege::User)),
            Err(NodeError::CoreUnsupported(version)) if version.minor == 5
        ));

        // Supported version but no binary on disk.
        let current = programs_at(&root, "2.6.4");
        let backend = SidecarBackend::new(Arc::new(Fixed(Ok(current)))).with_root(root.join("i"));
        assert_eq!(
            backend.check(&launch(Privilege::User)),
            Err(NodeError::CoreMissing)
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn nothing_running_polls_as_stopped_with_no_logs() {
        let root = scratch("idle");
        let backend = SidecarBackend::new(Arc::new(Fixed(Err(NodeError::CoreMissing))))
            .with_root(root.join("i"));
        let key = InstanceKey::for_profile("a1").unwrap();
        assert_eq!(backend.poll(&key), NetworkState::Stopped);
        assert!(backend.logs(&key).is_empty());
        assert!(!backend.service_installed(&key));
        backend.shutdown();
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_silent_check_failure_is_explained_from_the_toml_and_redacted() {
        let bad = Secret::new(format!("network_secret = \"{SECRET}\"\nthis is [not toml"));
        let redactor = Redactor::new().with_secret(SECRET);
        let message = silent_rejection(&bad, Some(1), &redactor);
        assert!(
            message.to_lowercase().contains("toml") || message.contains("line"),
            "{message}"
        );
        assert!(!message.contains(SECRET), "{message}");

        let valid = Secret::new(NODE_A);
        assert_eq!(
            silent_rejection(&valid, Some(1), &redactor),
            "the core rejected the config (exit status 1) without saying why"
        );
    }

    #[cfg(unix)]
    mod with_fake_core {
        use std::os::unix::fs::PermissionsExt as _;

        use super::*;

        /// A release whose `easytier-core` is a shell script that prints its arguments and environment shape, then
        /// idles, and whose `easytier-cli` prints the fixtures.
        fn fake_release(dir: &Path) -> Programs {
            let programs = programs_at(dir, "2.6.4");
            let core = format!(
                "#!/bin/sh\nfor a in \"$@\"; do echo \"arg:$a\"; done\necho \"env-config-server:[$ET_CONFIG_SERVER]\"\necho 'network_secret = \"{SECRET}\"'\nif [ \"$1\" = \"--check-config\" ]; then echo 'bad config near network_secret = \"{SECRET}\"' >&2; exit 1; fi\nsleep 60\n"
            );
            let cli = format!(
                "#!/bin/sh\ncase \"$*\" in\n*service*) echo 'Service is stopped'; exit 0;;\n*node) cat <<'EOF'\n{}\nEOF\n;;\n*peer) cat <<'EOF'\n{}\nEOF\n;;\nesac\n",
                include_str!("../../../tests/fixtures/easytier-2.6.4/cli-node.json"),
                include_str!("../../../tests/fixtures/easytier-2.6.4/cli-peer.json"),
            );
            for (path, body) in [(&programs.core, core), (&programs.cli, cli)] {
                fs::write(path, body).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            }
            programs
        }

        fn backend_for(root: &Path) -> SidecarBackend {
            let programs = fake_release(root);
            SidecarBackend::new(Arc::new(Fixed(Ok(programs))))
                .with_root(root.join("instances"))
                .with_timing(Timing {
                    base_delay: Duration::from_millis(20),
                    max_delay: Duration::from_millis(40),
                    stable_after: Duration::from_secs(60),
                    max_rapid_failures: 2,
                })
        }

        fn wait_for_state(
            backend: &SidecarBackend,
            key: &InstanceKey,
            done: impl Fn(&NetworkState) -> bool,
        ) -> NetworkState {
            for _ in 0..200 {
                let state = backend.poll(key);
                if done(&state) {
                    return state;
                }
                thread::sleep(Duration::from_millis(50));
            }
            backend.poll(key)
        }

        #[test]
        fn a_user_instance_runs_is_read_logged_redacted_and_stopped() {
            let root = scratch("user");
            let backend = backend_for(&root);
            let request = launch(Privilege::User);
            backend.start(&request).unwrap();

            let state = wait_for_state(&backend, &request.key, |s| {
                matches!(s, NetworkState::Running(_))
            });
            let NetworkState::Running(snapshot) = state else {
                panic!("expected Running, got {state:?}");
            };
            assert_eq!(snapshot.node.hostname, "example-host-a");
            assert_eq!(snapshot.peers.len(), 1);

            // Files: config 0600, meta with a port, logs dir.
            let dir = root.join("instances").join("p-a1");
            let mode = fs::metadata(dir.join("config.toml"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
            assert!(
                read_meta(&InstancePaths::new(&root.join("instances"), &request.key)).rpc_port != 0
            );
            assert!(dir.join("logs").is_dir());

            // The child saw the loopback portal arguments and no config-server.
            let logs = backend.logs(&request.key);
            let text: Vec<&str> = logs.iter().map(|l| l.text.as_str()).collect();
            assert!(
                text.iter()
                    .any(|l| l.starts_with("arg:--rpc-portal") || l.starts_with("arg:127.0.0.1:")),
                "{text:?}"
            );
            assert!(text.contains(&"arg:--rpc-portal-whitelist"));
            assert!(text.contains(&"arg:127.0.0.1/32,::1/128"));
            assert!(text.contains(&"arg:--disable-env-parsing"));
            assert!(text.contains(&"env-config-server:[]"));
            assert!(logs.iter().all(|l| !l.text.contains(SECRET)), "{text:?}");

            // Starting again restarts (one child, still running afterwards).
            backend.start(&request).unwrap();
            let state = wait_for_state(&backend, &request.key, |s| {
                matches!(s, NetworkState::Running(_))
            });
            assert!(matches!(state, NetworkState::Running(_)));

            backend.stop(&request.key).unwrap();
            assert_eq!(backend.poll(&request.key), NetworkState::Stopped);
            backend.remove(&request.key).unwrap();
            assert!(!dir.exists());
            backend.shutdown();
            fs::remove_dir_all(&root).unwrap();
        }

        #[test]
        fn a_managed_instance_gets_the_config_server_only_in_its_environment() {
            let root = scratch("managed");
            let backend = backend_for(&root);
            let url = "udp://config.example:22020/example-user-do-not-use";
            let request = NodeLaunch {
                key: InstanceKey::for_profile("m1").unwrap(),
                privilege: Privilege::User,
                source: LaunchSource::Managed {
                    config_server: Secret::new(url),
                    machine_id: "00000000-0000-0000-0000-000000000001".to_owned(),
                    hostname: Some("example-host-m".to_owned()),
                },
            };
            backend.start(&request).unwrap();
            wait_for_state(&backend, &request.key, |_| {
                !backend.logs(&request.key).is_empty()
            });
            thread::sleep(Duration::from_millis(300));
            let logs = backend.logs(&request.key);
            let text: Vec<&str> = logs.iter().map(|l| l.text.as_str()).collect();
            // The child received it (its echo is redacted in our ring), and no argument carries it.
            assert!(text.contains(&"env-config-server:[***]"), "{text:?}");
            assert!(
                text.iter()
                    .filter(|l| l.starts_with("arg:"))
                    .all(|l| !l.contains("config.example"))
            );
            assert!(text.contains(&"arg:--machine-id=00000000-0000-0000-0000-000000000001"));
            assert!(
                !root
                    .join("instances")
                    .join("p-m1")
                    .join("config.toml")
                    .exists()
            );
            backend.shutdown();
            fs::remove_dir_all(&root).unwrap();
        }

        #[test]
        fn a_rejected_config_is_reported_redacted() {
            let root = scratch("check");
            let backend = backend_for(&root);
            let error = backend.check(&launch(Privilege::User)).unwrap_err();
            let NodeError::ConfigRejected(message) = error else {
                panic!("expected ConfigRejected, got {error:?}");
            };
            assert!(message.contains("bad config"));
            assert!(!message.contains(SECRET), "{message}");
            // The temp file is gone: only `logs/` is left in the instance directory.
            let names: Vec<String> = fs::read_dir(root.join("instances").join("p-a1"))
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            assert_eq!(names, ["logs"]);
            backend.shutdown();
            fs::remove_dir_all(&root).unwrap();
        }

        fn service_location(root: &Path) -> Location {
            let anchor = root.join("system");
            Location {
                base: anchor.join("easytierui"),
                anchor,
            }
        }

        #[test]
        fn a_system_start_stages_root_owned_copies_installs_then_starts_and_records_the_service() {
            use super::super::service::testing::Recorder;

            let root = scratch("system");
            let location = service_location(&root);
            let files = ServiceFiles::new(&location.base, "p-a1");
            let recorder = Arc::new(Recorder::default());
            let backend = backend_for(&root)
                .with_elevator(recorder.clone())
                .with_service_location(location.clone());
            let request = launch(Privilege::System);
            backend.start(&request).unwrap();
            let calls = recorder.lines();
            assert_eq!(calls.len(), 2, "{calls:?}");
            let recorded = recorder.calls.lock().unwrap().clone();

            // Call 1: the app re-runs itself with the stage request; its CLI arguments follow `--`.
            let (program, args) = &recorded[0];
            assert_eq!(program, &std::env::current_exe().unwrap());
            assert_eq!(args[0], privileged::FLAG);
            assert_eq!(args[1], "stage");
            assert_eq!(args[2], location.base.to_string_lossy());
            assert_eq!(args[3], "p-a1");
            // Sources are the user's files with the digests taken before the prompt.
            let release = root.join("easytier-core");
            assert_eq!(args[4], release.to_string_lossy());
            assert_eq!(args[5], privileged::digest_file(&release).unwrap());
            assert_eq!(
                args[8],
                root.join("instances/p-a1/config.toml").to_string_lossy()
            );
            assert_eq!(args[9], digest_hex(NODE_A.as_bytes()));
            let separator = args.iter().position(|a| a == "--").unwrap();
            let install = args[separator + 1..].join(" ");
            assert!(
                install.starts_with("service --name easytierui-p-a1 install "),
                "{install}"
            );
            assert!(install.contains("--disable-autostart true"));
            // Every path the service is given is the administrator-only copy, none is the user's file.
            let service_args = &args[separator + 1..];
            for (flag, wanted) in [
                ("--core-path", &files.core),
                ("--service-work-dir", &files.dir),
                ("-c", &files.config),
                ("--file-log-dir", &files.logs),
            ] {
                let at = service_args.iter().position(|a| a == flag).unwrap();
                assert_eq!(service_args[at + 1], wanted.to_string_lossy(), "{flag}");
            }
            assert!(!install.contains(&root.join("instances").to_string_lossy().into_owned()));
            assert!(!install.contains(&root.join("easytier-core").to_string_lossy().into_owned()));
            assert!(!calls.iter().any(|c| c.contains(SECRET)));

            // Call 2: the start runs the administrator-only CLI copy.
            assert_eq!(recorded[1].0, files.cli);
            assert_eq!(calls[1], "service --name easytierui-p-a1 start");
            assert!(backend.service_installed(&request.key));

            // Same files, same port, same directory: a restart needs no staging and no install. The fake CLI's
            // status says "stopped", so no stop is needed either.
            backend.start(&request).unwrap();
            assert_eq!(recorder.lines().len(), 3);
            assert_eq!(recorder.lines()[2], "service --name easytierui-p-a1 start");
            assert_eq!(recorder.calls.lock().unwrap()[2].0, files.cli);

            // A changed config is copied again through the same step, which then starts the service (one prompt).
            let edited = NodeLaunch {
                source: LaunchSource::Local {
                    config_toml: Secret::new(format!("{NODE_A}\n# edited\n")),
                },
                ..request.clone()
            };
            backend.start(&edited).unwrap();
            let recorded = recorder.calls.lock().unwrap().clone();
            assert_eq!(recorded.len(), 4);
            let (program, args) = &recorded[3];
            assert_eq!(program, &std::env::current_exe().unwrap());
            assert_eq!(args[1], "stage");
            assert_eq!(
                args[args.len() - 4..].join(" "),
                "service --name easytierui-p-a1 start"
            );

            // A running child is refused once a service exists.
            let as_user = launch(Privilege::User);
            assert!(matches!(
                backend.start(&as_user),
                Err(NodeError::Service(_))
            ));

            // Removing is one step: uninstall through the copied CLI, then delete the administrator-only directory.
            backend.remove(&request.key).unwrap();
            let recorded = recorder.calls.lock().unwrap().clone();
            let (program, args) = recorded.last().unwrap();
            assert_eq!(program, &std::env::current_exe().unwrap());
            assert_eq!(args[1], "purge");
            assert_eq!(args[3], "p-a1");
            assert_eq!(
                args[args.len() - 4..].join(" "),
                "service --name easytierui-p-a1 uninstall"
            );
            assert!(!backend.service_installed(&request.key));
            fs::remove_dir_all(&root).unwrap();
        }

        #[test]
        fn a_declined_prompt_leaves_the_instance_stopped_and_unrecorded() {
            use super::super::service::testing::Recorder;

            let root = scratch("declined");
            let recorder = Arc::new(Recorder::default());
            recorder
                .script
                .lock()
                .unwrap()
                .push(Err(NodeError::ElevationDenied));
            let backend = backend_for(&root)
                .with_elevator(recorder.clone())
                .with_service_location(service_location(&root));
            let request = launch(Privilege::System);
            assert_eq!(backend.start(&request), Err(NodeError::ElevationDenied));
            assert!(!backend.service_installed(&request.key));
            assert_eq!(backend.poll(&request.key), NetworkState::Stopped);
            fs::remove_dir_all(&root).unwrap();
        }
    }

    /// The real core, unprivileged, on loopback with `no_tun`: `cargo test -- --ignored real_core`.
    #[test]
    #[ignore = "runs the real easytier-core from /tmp/easytier-2.6.4 (or $EASYTIER_DIR) on loopback"]
    fn real_core_smoke() {
        let dir = PathBuf::from(
            std::env::var("EASYTIER_DIR")
                .unwrap_or_else(|_| "/tmp/easytier-2.6.4/easytier-macos-aarch64".to_owned()),
        );
        let core = dir.join("easytier-core");
        let version_text =
            run::run(run::command(&core, ["--version"]), Duration::from_secs(10)).unwrap();
        let programs = Programs {
            version: CoreVersion::from_version_output(&version_text.stdout).unwrap(),
            dir: dir.clone(),
            core,
            cli: dir.join("easytier-cli"),
            web: dir.join("easytier-web"),
        };
        let root = scratch("real");
        let backend =
            SidecarBackend::new(Arc::new(Fixed(Ok(programs)))).with_root(root.join("instances"));
        let listener_port = instance::pick_free_port(&[]).unwrap();
        let config = format!(
            "instance_name = \"smoke\"\nhostname = \"example-host-smoke\"\nipv4 = \"10.144.99.1/24\"\nlisteners = [\"tcp://127.0.0.1:{listener_port}\"]\n\n[network_identity]\nnetwork_name = \"etui-smoke\"\nnetwork_secret = \"{SECRET}\"\n\n[flags]\nno_tun = true\n"
        );
        let request = NodeLaunch {
            key: InstanceKey::for_profile("smoke").unwrap(),
            privilege: Privilege::User,
            source: LaunchSource::Local {
                config_toml: Secret::new(config),
            },
        };
        backend.check(&request).unwrap();
        let bad = NodeLaunch {
            source: LaunchSource::Local {
                config_toml: Secret::new("this is [not toml"),
            },
            ..request.clone()
        };
        assert!(matches!(
            backend.check(&bad),
            Err(NodeError::ConfigRejected(_))
        ));

        backend.start(&request).unwrap();
        let mut state = NetworkState::Stopped;
        for _ in 0..120 {
            state = backend.poll(&request.key);
            if matches!(state, NetworkState::Running(_)) {
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }
        let NetworkState::Running(snapshot) = state else {
            panic!(
                "the core never answered: {state:?}; logs: {:?}",
                backend.logs(&request.key)
            );
        };
        assert_eq!(snapshot.node.hostname, "example-host-smoke");
        assert_eq!(
            snapshot.node.virtual_ipv4,
            crate::core::state::parse_ipv4_net("10.144.99.1/24")
        );
        assert!(snapshot.peers.is_empty());
        let logs = backend.logs(&request.key);
        assert!(!logs.is_empty());
        assert!(logs.iter().all(|l| !l.text.contains(SECRET)));

        backend.stop(&request.key).unwrap();
        assert_eq!(backend.poll(&request.key), NetworkState::Stopped);
        backend.remove(&request.key).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }
}
