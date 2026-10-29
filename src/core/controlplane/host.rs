//! [`HostedWeb`]: the official `easytier-web` of the active release, run as the app's child process.

use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, UdpSocket},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::{
    error::{HostError, HostPort},
    harden::{
        ADMIN_PASSWORD_ACCOUNT, AdminApi as _, HardenRequest, HardeningReport, LoopbackAdmin,
        harden_accounts,
    },
    http::Loopback,
    logs::LogRing,
    owner::{self, LoopbackOwner, OwnerError, Pending},
    settings::{HostProtocol, HostSettings, WebLayout},
};
use crate::core::{
    keychain::SecretStore,
    node::ProgramSource,
    secret::Secret,
    state::{LogLine, LogStream},
    version::CoreVersion,
};

/// The address the REST API binds: loopback, always.
const API_HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
/// How long the process may take to answer on its API before it is considered failed.
const START_TIMEOUT: Duration = Duration::from_secs(30);
/// How long [`HostedWeb::wait_running`] callers inside this module wait for a start.
const READY_WAIT: Duration = Duration::from_secs(40);
/// The monitor's poll while the API has not answered yet.
const POLL_STARTING: Duration = Duration::from_millis(100);
/// The monitor's poll once it runs: only exit matters.
const POLL_RUNNING: Duration = Duration::from_millis(250);
/// Log lines quoted in an "exited" failure.
const EXIT_LOG_LINES: usize = 8;
/// The account the console signs in with on the hosted server.
const CONSOLE_USER: &str = "admin";
/// Windows: do not open a console window for the child.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Where the hosted control plane is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HostState {
    /// Not running.
    #[default]
    Stopped,
    /// Spawned; the API has not answered yet. Never shown as running.
    Starting,
    /// The API answered.
    Running {
        /// The REST API's loopback address.
        api: SocketAddr,
        /// The config-server port (every interface).
        config_port: u16,
        /// The config-server protocol.
        protocol: HostProtocol,
        /// The release it runs.
        version: CoreVersion,
    },
    /// It could not start or ended on its own.
    Failed {
        /// Why.
        reason: HostError,
    },
}

impl HostState {
    /// Whether a start would duplicate it.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Starting | Self::Running { .. })
    }
}

/// How the console reaches the hosted server: over loopback, as `admin`.
#[derive(Debug, Clone)]
pub struct ApiAccess {
    /// `http://127.0.0.1:<port>`: plain HTTP is acceptable on loopback only.
    pub base_url: String,
    /// The account name (`admin`).
    pub username: String,
    /// Its password, from the keychain.
    pub password: Secret,
}

/// What a running (or starting) process was launched with.
struct Process {
    child: Arc<Mutex<Child>>,
    launched: HostSettings,
}

struct Inner {
    state: HostState,
    /// The settings the next start uses.
    settings: HostSettings,
    /// The saved settings could not be read; defaults are in use until the user saves some.
    settings_error: Option<HostError>,
    process: Option<Process>,
    /// Bumped by every start and stop, so a monitor of an older process knows to stand down.
    generation: u64,
}

struct Shared {
    inner: Mutex<Inner>,
    logs: LogRing,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What a monitor needs to announce "running".
struct ReadyInfo {
    api: SocketAddr,
    config_port: u16,
    protocol: HostProtocol,
    version: CoreVersion,
}

/// The official `easytier-web` run on this desktop, with first-run hardening.
///
/// Every method blocks (process spawn, loopback requests, keychain): call them through
/// [`work::off_thread`](crate::core::work::off_thread), except [`state`](Self::state), [`settings`](Self::settings)
/// and [`logs`](Self::logs), which only read memory. The process is the app's child: [`stop`](Self::stop),
/// [`shutdown`](Self::shutdown) and `Drop` kill it and wait for it, so none is left behind when the app quits
/// normally.
pub struct HostedWeb {
    programs: Arc<dyn ProgramSource>,
    layout: Result<WebLayout, HostError>,
    shared: Arc<Shared>,
    /// A registration that has its captcha and waits for the typed answer.
    pending_owner: Mutex<Option<Pending>>,
}

impl HostedWeb {
    /// A host that finds `easytier-web` through `programs` at each start and keeps its files in the app's data
    /// directory (`web/`).
    pub fn new(programs: Arc<dyn ProgramSource>) -> Self {
        Self::with_layout(programs, WebLayout::in_data_dir())
    }

    /// As [`new`](Self::new) with an explicit file layout (tests, a portable install).
    pub fn with_layout(
        programs: Arc<dyn ProgramSource>,
        layout: Result<WebLayout, HostError>,
    ) -> Self {
        let loaded = match &layout {
            Ok(layout) => HostSettings::load(layout),
            Err(error) => Err(error.clone()),
        };
        let (settings, settings_error) = match loaded {
            Ok(settings) => (settings, None),
            Err(error) => {
                log::warn!("the control-plane settings are unavailable: {error}");
                (HostSettings::default(), Some(error))
            }
        };
        Self {
            programs,
            layout,
            shared: Arc::new(Shared {
                inner: Mutex::new(Inner {
                    state: HostState::Stopped,
                    settings,
                    settings_error,
                    process: None,
                    generation: 0,
                }),
                logs: LogRing::default(),
            }),
            pending_owner: Mutex::new(None),
        }
    }

    fn layout(&self) -> Result<&WebLayout, HostError> {
        self.layout.as_ref().map_err(Clone::clone)
    }

    /// Where the process is now. Cheap; never blocks on the process.
    pub fn state(&self) -> HostState {
        self.shared.lock().state.clone()
    }

    /// The settings the next start uses.
    pub fn settings(&self) -> HostSettings {
        self.shared.lock().settings.clone()
    }

    /// Why the saved settings could not be read at startup, if they could not (defaults are in use).
    pub fn settings_error(&self) -> Option<HostError> {
        self.shared.lock().settings_error.clone()
    }

    /// The process's captured output, redacted, oldest first.
    pub fn logs(&self) -> Vec<LogLine> {
        self.shared.logs.snapshot()
    }

    /// Validates and saves `settings` for the next start; a running process keeps its own until
    /// [`restart`](Self::restart).
    ///
    /// # Errors
    ///
    /// [`HostError::InvalidSettings`] or [`HostError::Io`].
    pub fn set_settings(&self, settings: &HostSettings) -> Result<(), HostError> {
        settings.validate()?;
        settings.save(self.layout()?)?;
        let mut inner = self.shared.lock();
        inner.settings = settings.clone();
        inner.settings_error = None;
        Ok(())
    }

    /// Starts easytier-web with `settings` (saved first): the REST API on `127.0.0.1` only, the database and logs
    /// in the app's data directory. Returns once the process is spawned; [`state`](Self::state) is
    /// [`HostState::Starting`] until its API answers `GET /api/v1/auth/check_login_status` (401 or 200), then
    /// [`HostState::Running`] — see [`wait_running`](Self::wait_running).
    ///
    /// # Errors
    ///
    /// [`HostError::AlreadyRunning`] if it is starting or running; [`HostError::CoreMissing`],
    /// [`HostError::CoreUnsupported`] or [`HostError::ProgramMissing`] without a usable release;
    /// [`HostError::InvalidSettings`]; [`HostError::PortInUse`]; [`HostError::Io`]; [`HostError::Spawn`].
    pub fn start(&self, settings: &HostSettings) -> Result<(), HostError> {
        self.start_with(settings, false)
    }

    /// [`start`](Self::start), optionally with registration open for the owner-registration window. The saved and
    /// remembered settings always say registration is disabled; only the launched process differs.
    fn start_with(
        &self,
        settings: &HostSettings,
        registration_open: bool,
    ) -> Result<(), HostError> {
        let mut normalized = settings.clone();
        normalized.registration_disabled = true;
        let settings = &normalized;
        settings.validate()?;
        let layout = self.layout()?;
        let programs = self.programs.active()?;
        if !programs.web.is_file() {
            return Err(HostError::ProgramMissing);
        }

        let mut inner = self.shared.lock();
        if inner.state.is_active() {
            return Err(HostError::AlreadyRunning);
        }
        let attempt = (|| {
            layout.prepare()?;
            check_ports(settings)?;
            settings.save(layout)?;
            inner.settings = settings.clone();
            inner.settings_error = None;

            let mut command = Command::new(&programs.web);
            command
                .args(settings.launch_args(layout, registration_open))
                .current_dir(&layout.dir)
                // The only secret easytier-web 2.6.4 reads from the environment; it is not ours to pass on.
                .env_remove("OIDC_CLIENT_SECRET")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt as _;
                command.creation_flags(CREATE_NO_WINDOW);
            }
            command
                .spawn()
                .map_err(|error| HostError::Spawn(error.to_string()))
        })();
        let mut child = match attempt {
            Ok(child) => child,
            Err(error) => {
                inner.state = HostState::Failed {
                    reason: error.clone(),
                };
                return Err(error);
            }
        };

        let shared = Arc::clone(&self.shared);
        let mut readers = Vec::with_capacity(2);
        let mut start_readers = || -> Result<(), HostError> {
            if let Some(stdout) = child.stdout.take() {
                readers.push(spawn_reader(&shared, LogStream::Stdout, stdout)?);
            }
            if let Some(stderr) = child.stderr.take() {
                readers.push(spawn_reader(&shared, LogStream::Stderr, stderr)?);
            }
            Ok(())
        };
        if let Err(error) = start_readers() {
            if let Err(kill) = terminate(&mut child) {
                log::warn!("{kill}");
            }
            inner.state = HostState::Failed {
                reason: error.clone(),
            };
            return Err(error);
        }

        inner.generation = inner.generation.saturating_add(1);
        let generation = inner.generation;
        let child = Arc::new(Mutex::new(child));
        let ready = ReadyInfo {
            api: SocketAddr::V4(SocketAddrV4::new(API_HOST, settings.api_port)),
            config_port: settings.config_port,
            protocol: settings.protocol,
            version: programs.version.clone(),
        };
        let monitor_child = Arc::clone(&child);
        let monitor_shared = Arc::clone(&self.shared);
        let spawned = thread::Builder::new()
            .name("easytier-web-monitor".to_owned())
            .spawn(move || monitor(&monitor_shared, &monitor_child, generation, readers, &ready));
        if let Err(error) = spawned {
            let reason = HostError::Spawn(error.to_string());
            if let Err(kill) = terminate_shared(&child) {
                log::warn!("{kill}");
            }
            inner.state = HostState::Failed {
                reason: reason.clone(),
            };
            return Err(reason);
        }
        self.shared.logs.push(
            LogStream::App,
            &format!("easytier-web {} started", programs.version.number()),
        );
        inner.process = Some(Process {
            child,
            launched: HostSettings {
                registration_disabled: !registration_open,
                ..settings.clone()
            },
        });
        inner.state = HostState::Starting;
        Ok(())
    }

    /// Kills the process and waits for it. Stopping a stopped (or failed) host succeeds and clears the failure.
    ///
    /// easytier-web is killed, not asked to leave: its SQLite database is crash-safe and the process holds nothing
    /// else.
    ///
    /// # Errors
    ///
    /// [`HostError::Stop`] when the OS refused to kill or reap it; the state is then `Failed`.
    pub fn stop(&self) -> Result<(), HostError> {
        let mut inner = self.shared.lock();
        self.clear_pending_owner();
        inner.generation = inner.generation.saturating_add(1);
        let Some(process) = inner.process.take() else {
            inner.state = HostState::Stopped;
            return Ok(());
        };
        match terminate_shared(&process.child) {
            Ok(()) => {
                inner.state = HostState::Stopped;
                self.shared
                    .logs
                    .push(LogStream::App, "easytier-web stopped");
                Ok(())
            }
            Err(error) => {
                inner.state = HostState::Failed {
                    reason: error.clone(),
                };
                Err(error)
            }
        }
    }

    /// Stops, then starts again with the saved settings, finding `easytier-web` through the program source anew.
    ///
    /// This is how a version switch applies: after the Versions slice makes another release active, the store calls
    /// `restart()` (or `stop()` before the switch and `start` after), because a running process keeps the binary it
    /// was started from. Returns once the new process is spawned; use [`wait_running`](Self::wait_running).
    ///
    /// # Errors
    ///
    /// As [`stop`](Self::stop) and [`start`](Self::start).
    pub fn restart(&self) -> Result<(), HostError> {
        self.stop()?;
        let settings = self.settings();
        self.start(&settings)
    }

    /// Stops for good at app exit; the same as [`stop`](Self::stop), named for the call site.
    ///
    /// # Errors
    ///
    /// As [`stop`](Self::stop).
    pub fn shutdown(&self) -> Result<(), HostError> {
        self.stop()
    }

    /// Waits until the API answers.
    ///
    /// # Errors
    ///
    /// The failure the process ended with, [`HostError::NotRunning`] if it is stopped, [`HostError::StartTimeout`]
    /// when `timeout` passes first.
    pub fn wait_running(&self, timeout: Duration) -> Result<HostState, HostError> {
        let deadline = Instant::now().checked_add(timeout);
        loop {
            match self.state() {
                running @ HostState::Running { .. } => return Ok(running),
                HostState::Failed { reason } => return Err(reason),
                HostState::Stopped => return Err(HostError::NotRunning),
                HostState::Starting => {}
            }
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return Err(HostError::StartTimeout);
            }
            thread::sleep(POLL_STARTING);
        }
    }

    /// How the console signs in to this server over loopback: `None` until the `admin` password is in `secrets`
    /// (hardening has not run, or the keychain entry is gone).
    pub fn api_access(&self, secrets: &dyn SecretStore) -> Option<ApiAccess> {
        let password = match secrets.get(ADMIN_PASSWORD_ACCOUNT) {
            Ok(password) => password?,
            Err(error) => {
                log::warn!("the keychain could not be read for the hosted admin password: {error}");
                return None;
            }
        };
        let port = self.shared.lock().settings.api_port;
        Some(ApiAccess {
            base_url: format!("http://{API_HOST}:{port}"),
            username: CONSOLE_USER.to_owned(),
            password,
        })
    }

    /// First-run hardening. Starts the server if it is stopped, signs in to `admin` and `user` on loopback with
    /// their seeded passwords, replaces each with the one in `request` or a generated one (kept in `secrets` under
    /// `host/admin-password` and `host/user-password`), confirms the seeded passwords no longer sign in, then
    /// restarts with registration disabled and confirms it is. A database whose seeded passwords already fail is
    /// reported as hardened and left alone.
    ///
    /// Never returns an error: a failure ends the run and is in [`HardeningReport::error`], with the fields saying
    /// how far it got. The API listens on loopback throughout.
    pub fn harden(&self, request: &HardenRequest, secrets: &dyn SecretStore) -> HardeningReport {
        let settings = self.settings();
        let mut report = HardeningReport::new(settings.config_port, settings.protocol);
        if let Err(error) = self.harden_steps(request, secrets, &mut report) {
            log::warn!("hardening stopped: {error}");
            report.error = Some(error);
        }
        report
    }

    fn harden_steps(
        &self,
        request: &HardenRequest,
        secrets: &dyn SecretStore,
        report: &mut HardeningReport,
    ) -> Result<(), HostError> {
        request.validate()?;
        match self.state() {
            HostState::Stopped | HostState::Failed { .. } => {
                let settings = self.settings();
                self.start(&settings)?;
            }
            HostState::Starting | HostState::Running { .. } => {}
        }
        self.wait_running(READY_WAIT)?;
        let api = LoopbackAdmin(self.loopback()?);

        harden_accounts(&api, secrets, request, report)?;

        // The server starts with registration disabled; only an owner-registration window left open (a process
        // launched with it) needs closing here.
        self.close_registration()?;
        if !api.registration_closed()? {
            return Err(HostError::RegistrationStillOpen);
        }
        report.registration_disabled = true;
        Ok(())
    }

    // --- The owner account -------------------------------------------------------------------------------------

    /// Whether the keychain holds an owner account (its name and password).
    pub fn owner_registered(&self, secrets: &dyn SecretStore) -> bool {
        match owner::stored(secrets) {
            Ok(found) => found.is_some(),
            Err(error) => {
                log::warn!("the keychain could not be read for the owner account: {error}");
                false
            }
        }
    }

    /// Starts registering the owner account: makes sure the server runs, then restarts it with registration open
    /// **on loopback only** for this one step, asks for a captcha and returns the PNG for the person to read. The
    /// generated name and password are kept only once [`complete_owner_registration`](Self::complete_owner_registration)
    /// succeeds. The window ends there, on [`cancel_owner_registration`](Self::cancel_owner_registration), on
    /// [`stop`](Self::stop) and when the app quits.
    ///
    /// # Errors
    ///
    /// [`OwnerError::AlreadyRegistered`] when the keychain's owner account signs in; [`OwnerError::Host`] when the
    /// server cannot start or answer, or the keychain fails. On an error the window is closed again.
    pub fn begin_owner_registration(
        &self,
        secrets: &dyn SecretStore,
    ) -> Result<Vec<u8>, OwnerError> {
        self.clear_pending_owner();
        match self.state() {
            HostState::Stopped | HostState::Failed { .. } => {
                let settings = self.settings();
                self.start(&settings)?;
            }
            HostState::Starting | HostState::Running { .. } => {}
        }
        self.wait_running(READY_WAIT)?;
        let api = LoopbackOwner(self.loopback()?);
        if owner::already_registered(&api, secrets)? {
            return Err(OwnerError::AlreadyRegistered);
        }
        let opened = self.reopen(true);
        let begun = opened.and_then(|()| owner::begin(&api));
        match begun {
            Ok((pending, png)) => {
                *self
                    .pending_owner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(pending);
                Ok(png)
            }
            Err(error) => {
                if let Err(close) = self.close_registration() {
                    log::warn!("registration could not be closed again: {close}");
                }
                Err(error.into())
            }
        }
    }

    /// Registers the owner account with the captcha text the person read from the PNG, keeps its name and password
    /// in the keychain (`host/owner-name`, `host/owner-password`) and restarts the server with registration
    /// disabled again.
    ///
    /// # Errors
    ///
    /// [`OwnerError::NotStarted`] without a registration begun; [`OwnerError::CaptchaRefused`] for wrong text (the
    /// window stays open and the same captcha can be answered again, or the registration cancelled);
    /// [`OwnerError::Host`] otherwise (the window is closed).
    pub fn complete_owner_registration(
        &self,
        captcha: &str,
        secrets: &dyn SecretStore,
    ) -> Result<(), OwnerError> {
        let pending = self
            .pending_owner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or(OwnerError::NotStarted)?;
        let api = LoopbackOwner(self.loopback()?);
        match owner::complete(&api, secrets, &pending, captcha) {
            Err(OwnerError::CaptchaRefused) => {
                *self
                    .pending_owner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(pending);
                Err(OwnerError::CaptchaRefused)
            }
            outcome => {
                // The window ends whatever happened.
                let closed = self.close_registration();
                outcome?;
                closed?;
                if !LoopbackAdmin(self.loopback()?).registration_closed()? {
                    return Err(HostError::RegistrationStillOpen.into());
                }
                Ok(())
            }
        }
    }

    /// Gives up a registration begun and closes the window.
    ///
    /// # Errors
    ///
    /// As [`stop`](Self::stop) and [`start`](Self::start) (the server restarts closed).
    pub fn cancel_owner_registration(&self) -> Result<(), HostError> {
        self.close_registration()
    }

    /// What a machine is given to enroll under the owner account: `<protocol>://<this computer's LAN address, or
    /// [`ENROLLMENT_HOST_PLACEHOLDER`]>:<config port>/<owner name>`. A secret: the path is the account to enroll
    /// into. `None` without an owner account in the keychain.
    pub fn owner_enrollment_url(&self, secrets: &dyn SecretStore) -> Option<Secret> {
        let (name, _) = match owner::stored(secrets) {
            Ok(found) => found?,
            Err(error) => {
                log::warn!("the keychain could not be read for the owner account: {error}");
                return None;
            }
        };
        let settings = self.settings();
        Some(owner::enrollment_url(
            &settings,
            &name,
            owner::lan_address(),
        ))
    }

    fn clear_pending_owner(&self) {
        self.pending_owner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }

    /// Restarts the server, registration open (`open`) or closed, and waits for its API.
    fn reopen(&self, open: bool) -> Result<(), HostError> {
        let settings = self.settings();
        self.stop()?;
        self.start_with(&settings, open)?;
        self.wait_running(READY_WAIT)?;
        Ok(())
    }

    /// Ends an owner-registration window: nothing pending, and a process launched with registration open restarts
    /// with it disabled. A stopped server or one that never opened it is left as it is.
    fn close_registration(&self) -> Result<(), HostError> {
        self.clear_pending_owner();
        let open = self
            .shared
            .lock()
            .process
            .as_ref()
            .is_some_and(|process| !process.launched.registration_disabled);
        if open {
            self.reopen(false)?;
        }
        Ok(())
    }

    fn loopback(&self) -> Result<Loopback, HostError> {
        let port = self.shared.lock().settings.api_port;
        Loopback::new(SocketAddr::V4(SocketAddrV4::new(API_HOST, port)))
    }
}

impl Drop for HostedWeb {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            log::warn!("{error}");
        }
    }
}

/// Fails with the port that is taken. The config server binds every interface; this checks loopback so the app
/// itself never opens a listener the OS firewall would ask about.
fn check_ports(settings: &HostSettings) -> Result<(), HostError> {
    let taken = |which, port| HostError::PortInUse { which, port };
    TcpListener::bind((API_HOST, settings.api_port))
        .map(drop)
        .map_err(|_| taken(HostPort::Api, settings.api_port))?;
    let config_free = if settings.protocol.is_stream() {
        TcpListener::bind((API_HOST, settings.config_port)).map(drop)
    } else {
        UdpSocket::bind((API_HOST, settings.config_port)).map(drop)
    };
    config_free.map_err(|_| taken(HostPort::Config, settings.config_port))
}

fn spawn_reader(
    shared: &Arc<Shared>,
    stream: LogStream,
    source: impl std::io::Read + Send + 'static,
) -> Result<JoinHandle<()>, HostError> {
    let shared = Arc::clone(shared);
    thread::Builder::new()
        .name("easytier-web-output".to_owned())
        .spawn(move || shared.logs.pump(stream, source))
        .map_err(|error| HostError::Spawn(error.to_string()))
}

fn terminate_shared(child: &Mutex<Child>) -> Result<(), HostError> {
    let mut child = child.lock().unwrap_or_else(PoisonError::into_inner);
    terminate(&mut child)
}

/// Kills the child unless it already ended, then reaps it.
fn terminate(child: &mut Child) -> Result<(), HostError> {
    match child.try_wait() {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => {}
        Err(error) => log::warn!("could not poll easytier-web before killing it: {error}"),
    }
    child
        .kill()
        .map_err(|error| HostError::Stop(error.to_string()))?;
    child
        .wait()
        .map(drop)
        .map_err(|error| HostError::Stop(error.to_string()))
}

/// Whether the API answers (401 when signed out, 200 when a session is sent).
fn api_answers(api: SocketAddr) -> bool {
    Loopback::new(api)
        .and_then(|client| client.send("GET", "/api/v1/auth/check_login_status", None, None))
        .is_ok_and(|response| matches!(response.status, 200 | 401))
}

/// Watches one process: announces `Running` when the API answers, `Failed` when it dies or never answers. Ends when
/// a stop or restart makes its generation stale.
fn monitor(
    shared: &Shared,
    child: &Mutex<Child>,
    generation: u64,
    readers: Vec<JoinHandle<()>>,
    ready: &ReadyInfo,
) {
    let started = Instant::now();
    let mut running = false;
    loop {
        let polled = child
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .try_wait();
        match polled {
            Ok(Some(status)) => {
                // The pipes close with the process: let the readers deliver the last lines first.
                for reader in readers {
                    if reader.join().is_err() {
                        log::warn!("an easytier-web output reader panicked");
                    }
                }
                let mut inner = shared.lock();
                if inner.generation == generation {
                    let last_log = shared.logs.tail_text(EXIT_LOG_LINES);
                    shared.logs.push(LogStream::App, "easytier-web exited");
                    inner.process = None;
                    inner.state = HostState::Failed {
                        reason: HostError::Exited {
                            code: status.code(),
                            last_log,
                        },
                    };
                }
                return;
            }
            Ok(None) => {}
            Err(error) => {
                let mut inner = shared.lock();
                if inner.generation == generation {
                    inner.process = None;
                    inner.state = HostState::Failed {
                        reason: HostError::Stop(error.to_string()),
                    };
                }
                return;
            }
        }

        if shared.lock().generation != generation {
            return;
        }
        if !running {
            if api_answers(ready.api) {
                let mut inner = shared.lock();
                if inner.generation != generation {
                    return;
                }
                inner.state = HostState::Running {
                    api: ready.api,
                    config_port: ready.config_port,
                    protocol: ready.protocol,
                    version: ready.version.clone(),
                };
                running = true;
            } else if started.elapsed() > START_TIMEOUT {
                let mut inner = shared.lock();
                if inner.generation != generation {
                    return;
                }
                if let Err(error) = terminate_shared(child) {
                    log::warn!("{error}");
                }
                inner.process = None;
                inner.state = HostState::Failed {
                    reason: HostError::StartTimeout,
                };
                return;
            }
        }
        thread::sleep(if running { POLL_RUNNING } else { POLL_STARTING });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::{NodeError, Programs};

    struct NoPrograms;

    impl ProgramSource for NoPrograms {
        fn active(&self) -> Result<Programs, NodeError> {
            Err(NodeError::CoreMissing)
        }
    }

    fn temp_layout(name: &str) -> WebLayout {
        WebLayout::under(
            std::env::temp_dir().join(format!("etui-host-{name}-{}", std::process::id())),
        )
    }

    #[test]
    fn a_host_without_a_release_stays_stopped_and_says_why() {
        let host = HostedWeb::with_layout(Arc::new(NoPrograms), Ok(temp_layout("none")));
        assert_eq!(host.state(), HostState::Stopped);
        assert_eq!(
            host.start(&HostSettings::default()),
            Err(HostError::CoreMissing)
        );
        assert_eq!(host.state(), HostState::Stopped);
        host.stop().unwrap();
        host.shutdown().unwrap();
    }

    #[test]
    fn hardening_a_host_without_a_release_reports_the_failure_and_the_exposure() {
        let host = HostedWeb::with_layout(Arc::new(NoPrograms), Ok(temp_layout("harden")));
        let secrets = crate::core::keychain::MemoryStore::default();
        let report = host.harden(&HardenRequest::default(), &secrets);
        assert_eq!(report.error, Some(HostError::CoreMissing));
        assert!(!report.admin_default_replaced && !report.registration_disabled);
        assert!(report.api_loopback_only && report.exposed_on_all_interfaces);
        assert_eq!(report.config_server_port, 22020);
        assert!(!report.is_complete());
    }

    #[test]
    fn api_access_needs_the_admin_password_and_points_at_loopback() {
        let host = HostedWeb::with_layout(Arc::new(NoPrograms), Ok(temp_layout("access")));
        let secrets = crate::core::keychain::MemoryStore::default();
        assert!(host.api_access(&secrets).is_none());
        secrets
            .set(
                ADMIN_PASSWORD_ACCOUNT,
                &Secret::new("example-secret-do-not-use"),
            )
            .unwrap();
        let access = host.api_access(&secrets).unwrap();
        assert_eq!(access.base_url, "http://127.0.0.1:11211");
        assert_eq!(access.username, "admin");
        assert_eq!(access.password.expose(), "example-secret-do-not-use");
        assert!(!format!("{access:?}").contains("example-secret"));
    }

    #[test]
    fn a_missing_layout_fails_every_start() {
        let host = HostedWeb::with_layout(
            Arc::new(NoPrograms),
            Err(HostError::Io("no data directory".to_owned())),
        );
        assert!(host.settings_error().is_some());
        assert!(matches!(
            host.start(&HostSettings::default()),
            Err(HostError::Io(_))
        ));
    }

    #[test]
    fn invalid_settings_never_reach_the_program_source() {
        let host = HostedWeb::with_layout(Arc::new(NoPrograms), Ok(temp_layout("invalid")));
        let settings = HostSettings {
            api_port: 22,
            ..HostSettings::default()
        };
        assert!(matches!(
            host.start(&settings),
            Err(HostError::InvalidSettings(_))
        ));
    }

    #[test]
    fn the_states_know_whether_a_start_would_duplicate_them() {
        assert!(!HostState::Stopped.is_active());
        assert!(HostState::Starting.is_active());
        assert!(
            !HostState::Failed {
                reason: HostError::StartTimeout
            }
            .is_active()
        );
    }

    #[test]
    fn a_taken_api_port_is_named() {
        let held = TcpListener::bind((API_HOST, 0)).unwrap();
        let port = held.local_addr().unwrap().port();
        let settings = HostSettings {
            api_port: port,
            config_port: if port == 40000 { 40001 } else { 40000 },
            ..HostSettings::default()
        };
        assert_eq!(
            check_ports(&settings),
            Err(HostError::PortInUse {
                which: HostPort::Api,
                port
            })
        );
    }
}
