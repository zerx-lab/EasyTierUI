//! The control-plane host role's state: the easytier-web this desktop hosts (process, first-run hardening) and the
//! deployment files being drafted.
//!
//! The hosted control plane exists on the desktops only (`HostedWeb`, behind [`backend`]); the pages see plain domain
//! types that exist on every platform. While it starts or runs, this store keeps the [`TargetId::HOSTED`] target in
//! the switcher with a truthful lamp, and removes it when the process is stopped. Deployment drafting is pure and
//! works everywhere: a form over [`DeploymentPlan`] and the rendered artifacts, none of which is applied anywhere.

use std::{future::Future, sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use nsh_core::{prelude::*, time::unix_ms};

use self::backend::Backend;
use super::{Page, Store, StoreEnv};
use crate::{
    core::{
        keychain::SecretStore,
        secret::Secret,
        target::{Target, TargetId, TargetKind, TargetOrigin, TargetStatus},
        work::off_thread,
    },
    deploy::{
        Artifact, ConfigProtocol, DEFAULT_API_PORT, DEFAULT_CONFIG_PORT, DeployError,
        DeploymentPlan, Email, Field, Hardening, Hostname, LinuxArch, NodeService, OwnerName,
        ProxyKind, Reason, render,
    },
    i18n::Pick,
};

/// How often a running or starting control plane is read.
const POLL: Duration = Duration::from_secs(1);

/// Why hosting failed, in cases the page words (with machine data shown apart from the words).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostProblem {
    /// No EasyTier release is installed.
    CoreMissing,
    /// The active release (its version in the text) is older than the app supports.
    CoreUnsupported(String),
    /// The active release has no easytier-web.
    ProgramMissing,
    /// The ports cannot be used.
    Ports,
    /// The saved settings could not be read.
    SettingsCorrupt,
    /// Another program uses a port.
    PortInUse {
        /// The config server's port (otherwise the REST API's).
        config: bool,
        /// The port.
        port: u16,
    },
    /// Already running.
    AlreadyRunning,
    /// Not running.
    NotRunning,
    /// The process could not start (the reason in the text).
    Spawn(String),
    /// The process could not be stopped (the reason in the text).
    Stop(String),
    /// The process exited on its own (its last output in the text).
    Exited(String),
    /// The API never answered.
    StartTimeout,
    /// A file failed (the OS message in the text).
    Files(String),
    /// The keychain failed (the message in the text).
    Keychain(String),
    /// No secure random source.
    Random,
    /// A password is empty (the account in the text).
    PasswordEmpty(String),
    /// A password is too short (the account in the text).
    PasswordShort(String),
    /// The API failed (the message in the text).
    Api(String),
    /// An account was changed elsewhere (the account in the text).
    PasswordUnknown(String),
    /// A password change was not confirmed (the account in the text).
    Unconfirmed(String),
    /// Registration is still open after the restart.
    RegistrationOpen,
    /// The background task ended without a result.
    Interrupted,
    /// This platform cannot host.
    Unavailable,
}

impl HostProblem {
    /// The words for the problem.
    pub fn message(&self) -> Pick {
        match self {
            Self::CoreMissing => |s| s.host.problem_core_missing,
            Self::CoreUnsupported(_) => |s| s.host.problem_core_unsupported,
            Self::ProgramMissing => |s| s.host.problem_program_missing,
            Self::Ports => |s| s.host.problem_ports,
            Self::SettingsCorrupt => |s| s.host.problem_settings_corrupt,
            Self::PortInUse { config: false, .. } => |s| s.host.problem_port_api,
            Self::PortInUse { config: true, .. } => |s| s.host.problem_port_config,
            Self::AlreadyRunning => |s| s.host.problem_already_running,
            Self::NotRunning => |s| s.host.problem_not_running,
            Self::Spawn(_) => |s| s.host.problem_spawn,
            Self::Stop(_) => |s| s.host.problem_stop,
            Self::Exited(_) => |s| s.host.problem_exited,
            Self::StartTimeout => |s| s.host.problem_timeout,
            Self::Files(_) => |s| s.host.problem_files,
            Self::Keychain(_) => |s| s.host.problem_keychain,
            Self::Random => |s| s.host.problem_random,
            Self::PasswordEmpty(_) => |s| s.host.problem_password_empty,
            Self::PasswordShort(_) => |s| s.host.problem_password_short,
            Self::Api(_) => |s| s.host.problem_api,
            Self::PasswordUnknown(_) => |s| s.host.problem_password_unknown,
            Self::Unconfirmed(_) => |s| s.host.problem_unconfirmed,
            Self::RegistrationOpen => |s| s.host.problem_registration_open,
            Self::Interrupted => |s| s.host.problem_interrupted,
            Self::Unavailable => |s| s.host.problem_unavailable,
        }
    }

    /// The machine data that goes with the words, shown apart from them in the monospace.
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::PortInUse { port, .. } => Some(port.to_string()),
            Self::CoreUnsupported(text)
            | Self::Spawn(text)
            | Self::Stop(text)
            | Self::Exited(text)
            | Self::Files(text)
            | Self::Keychain(text)
            | Self::PasswordEmpty(text)
            | Self::PasswordShort(text)
            | Self::Api(text)
            | Self::PasswordUnknown(text)
            | Self::Unconfirmed(text) => Some(text.clone()).filter(|text| !text.is_empty()),
            _ => None,
        }
    }

    /// Whether the way out is Settings → EasyTier (no usable release).
    pub fn needs_release(&self) -> bool {
        matches!(
            self,
            Self::CoreMissing | Self::CoreUnsupported(_) | Self::ProgramMissing
        )
    }
}

/// A running control plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningHost {
    /// The REST API's port on loopback.
    pub api_port: u16,
    /// The config server's port (every interface).
    pub config_port: u16,
    /// The config server's protocol.
    pub protocol: ConfigProtocol,
    /// The release it runs.
    pub version: String,
}

/// Where the hosted control plane is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRun {
    /// Not running.
    Stopped,
    /// Spawned; its API has not answered yet.
    Starting,
    /// The API answered.
    Running(RunningHost),
    /// It could not start or ended on its own.
    Failed(HostProblem),
}

/// How the control plane is configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConfig {
    /// The REST API's port (always on loopback).
    pub api_port: u16,
    /// The config server's port (every interface).
    pub config_port: u16,
    /// The config server's protocol.
    pub protocol: ConfigProtocol,
    /// Registration is switched off (the control plane starts with it off once hardened).
    pub registration_disabled: bool,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            api_port: DEFAULT_API_PORT,
            config_port: DEFAULT_CONFIG_PORT,
            protocol: ConfigProtocol::Udp,
            registration_disabled: false,
        }
    }
}

/// What the store is doing to the control plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    /// Nothing.
    Idle,
    /// Starting it.
    Starting,
    /// Stopping it.
    Stopping,
    /// Restarting it.
    Restarting,
    /// Saving settings.
    Saving,
    /// Hardening.
    Hardening,
    /// Registering the owner account.
    Owner,
}

/// What is wrong with the settings form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormError {
    /// The REST API port is not a number.
    ApiNotNumber,
    /// The config server port is not a number.
    ConfigNotNumber,
    /// The ports are out of range or equal.
    Invalid,
}

/// The passwords the user chose; `None` means "generate one".
pub struct PasswordChoice {
    /// The new `admin` password.
    pub admin: Option<Secret>,
    /// The new `user` password.
    pub user: Option<Secret>,
}

/// Which typed passwords are too short.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputErrors {
    /// The `admin` password.
    pub admin_short: bool,
    /// The `user` password.
    pub user_short: bool,
}

/// What hardening did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardeningOutcome {
    /// `admin` no longer signs in with its seeded password.
    pub admin_replaced: bool,
    /// `user` no longer signs in with its seeded password.
    pub user_replaced: bool,
    /// Registration is closed, confirmed against the running server.
    pub registration_disabled: bool,
    /// Both accounts were already hardened.
    pub already_hardened: bool,
    /// The keychain holds the `admin` password.
    pub admin_stored: bool,
    /// The keychain holds the `user` password.
    pub user_stored: bool,
    /// The config server's port.
    pub config_port: u16,
    /// The config server's protocol.
    pub protocol: ConfigProtocol,
    /// Why hardening stopped early.
    pub error: Option<HostProblem>,
    /// Every step finished.
    pub complete: bool,
}

/// Where hardening is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardeningState {
    /// Not run in this session.
    Idle,
    /// Running.
    Running,
    /// Finished or stopped, with what it did.
    Done(HardeningOutcome),
}

/// One line of the checklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// Passed.
    Pass,
    /// Not done.
    Open,
    /// Not known in this session.
    Unknown,
}

/// The checklist's lines that depend on what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HardeningChecks {
    /// The default `admin` password is replaced.
    pub admin: Check,
    /// The default `user` password is replaced.
    pub user: Check,
    /// Registration is off.
    pub registration: Check,
    /// The owner account machines enroll under is registered.
    pub owner: Check,
}

impl HardeningChecks {
    /// Every line passed.
    pub fn all_pass(&self) -> bool {
        [self.admin, self.user, self.registration, self.owner]
            .iter()
            .all(|check| *check == Check::Pass)
    }

    /// How many lines passed, and how many there are.
    pub fn passed(&self) -> (usize, usize) {
        let lines = [self.admin, self.user, self.registration, self.owner];
        let done = lines.iter().filter(|check| **check == Check::Pass).count();
        (done, lines.len())
    }
}

/// Which passwords the keychain holds (never their values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StoredPasswords {
    /// `admin`'s.
    pub admin: bool,
    /// `user`'s.
    pub user: bool,
}

/// The passwords the keychain holds, read on request.
#[derive(Clone, PartialEq)]
pub struct RevealedPasswords {
    /// `admin`'s.
    pub admin: Option<Secret>,
    /// `user`'s.
    pub user: Option<Secret>,
}

/// Why the owner account could not be registered, in cases the page words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerProblem {
    /// The typed captcha text was not accepted; the same image can be answered again.
    CaptchaRefused,
    /// An owner account already signs in.
    AlreadyRegistered,
    /// No registration is waiting for a captcha (it ended).
    NotStarted,
    /// The control plane, its API or the keychain failed.
    Host(HostProblem),
}

impl OwnerProblem {
    /// The words for the problem.
    pub fn message(&self) -> Pick {
        match self {
            Self::CaptchaRefused => |s| s.host.problem_owner_captcha,
            Self::AlreadyRegistered => |s| s.host.problem_owner_already,
            Self::NotStarted => |s| s.host.problem_owner_not_started,
            Self::Host(problem) => problem.message(),
        }
    }

    /// The machine data that goes with the words.
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Host(problem) => problem.detail(),
            Self::CaptchaRefused | Self::AlreadyRegistered | Self::NotStarted => None,
        }
    }
}

/// Where the owner-account step is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerStep {
    /// Not begun (or ended).
    Idle,
    /// Opening the registration window and asking for the captcha.
    Preparing,
    /// The captcha is shown and waits for the typed text.
    Waiting {
        /// The captcha image as a `data:` URI.
        captcha: String,
    },
    /// The typed text is being submitted.
    Submitting {
        /// The captcha image as a `data:` URI.
        captcha: String,
    },
}

/// What a machine is given to enroll under the owner account: a secret, because its path is the account name.
#[derive(Clone, PartialEq)]
pub struct Enrollment {
    /// The enrollment URL.
    pub url: Secret,
    /// The URL holds a placeholder for this computer's address, which the person replaces.
    pub needs_address: bool,
}

/// Everything read from the control plane in one go.
struct Snapshot {
    run: HostRun,
    config: HostConfig,
    config_problem: Option<HostProblem>,
    logs: Vec<String>,
}

/// How the server runs EasyTier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    /// Docker Compose with the official image.
    Compose,
    /// Release programs under systemd.
    Systemd,
}

/// The deployment form: text as typed, parsed only when the plan is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployForm {
    /// The public domain.
    pub domain: String,
    /// The ACME contact email (may be empty).
    pub email: String,
    /// The reverse proxy.
    pub proxy: ProxyKind,
    /// The runtime.
    pub runtime: RuntimeKind,
    /// The server's CPU (systemd only).
    pub arch: LinuxArch,
    /// The REST API's port.
    pub api_port: String,
    /// The config server's port.
    pub config_port: String,
    /// The config server's protocol.
    pub protocol: ConfigProtocol,
    /// The server also runs a node.
    pub node: bool,
    /// The node's owner account.
    pub owner: String,
    /// How far hardening is.
    pub hardening: Hardening,
}

impl Default for DeployForm {
    fn default() -> Self {
        Self {
            domain: String::new(),
            email: String::new(),
            proxy: ProxyKind::Caddy,
            runtime: RuntimeKind::Compose,
            arch: LinuxArch::X86_64,
            api_port: DEFAULT_API_PORT.to_string(),
            config_port: DEFAULT_CONFIG_PORT.to_string(),
            protocol: ConfigProtocol::Udp,
            node: false,
            owner: String::new(),
            hardening: Hardening::Pending,
        }
    }
}

/// What the deployment form renders to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeployOutcome {
    /// No domain yet: nothing to render.
    Empty,
    /// A field was refused.
    Invalid(DeployError),
    /// The files, README first.
    Ready(Vec<Artifact>),
}

fn port_of(text: &str, field: Field) -> Result<u16, DeployError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(DeployError::new(field, Reason::Empty));
    }
    text.parse()
        .map_err(|_| DeployError::new(field, Reason::BadFormat))
}

/// The plan `form` describes.
fn plan_of(form: &DeployForm) -> Result<DeploymentPlan, DeployError> {
    let domain = Hostname::parse(&form.domain)?;
    let mut plan = match form.runtime {
        RuntimeKind::Compose => DeploymentPlan::compose(domain, form.proxy)?,
        RuntimeKind::Systemd => DeploymentPlan::systemd(domain, form.proxy, form.arch)?,
    };
    plan.acme_email = if form.email.trim().is_empty() {
        None
    } else {
        Some(Email::parse(&form.email)?)
    };
    plan.api_port = port_of(&form.api_port, Field::ApiPort)?;
    plan.config_port = port_of(&form.config_port, Field::ConfigPort)?;
    plan.config_protocol = form.protocol;
    plan.node = if form.node {
        Some(NodeService {
            owner: OwnerName::parse(&form.owner)?,
            machine_id: None,
        })
    } else {
        None
    };
    plan.hardening = form.hardening;
    Ok(plan)
}

/// Renders `form`: the README first, then the files in the order the README applies them.
fn outcome_of(form: &DeployForm) -> DeployOutcome {
    if form.domain.trim().is_empty() {
        return DeployOutcome::Empty;
    }
    match plan_of(form).and_then(|plan| render(&plan)) {
        Ok(mut artifacts) => {
            artifacts.sort_by_key(|artifact| artifact.file_name != "README.md");
            DeployOutcome::Ready(artifacts)
        }
        Err(error) => DeployOutcome::Invalid(error),
    }
}

/// The desktops' hosted easytier-web; an empty type elsewhere, so a build without one has no `Some` to find.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod backend {
    use std::sync::Arc;

    use super::{
        ConfigProtocol, Enrollment, HardeningOutcome, HostConfig, HostProblem, HostRun,
        InputErrors, OwnerProblem, PasswordChoice, RunningHost, Snapshot,
    };
    use crate::core::{
        controlplane::{
            ADMIN_PASSWORD_ACCOUNT, ENROLLMENT_HOST_PLACEHOLDER, HardenRequest, HardeningReport,
            HostError, HostPort, HostProtocol, HostSettings, HostState, HostedWeb,
            MIN_PASSWORD_CHARS, OwnerError, PasswordProblem, USER_PASSWORD_ACCOUNT,
        },
        keychain::SecretStore,
        node::ProgramSource,
        secret::Secret,
    };

    /// The most output lines kept for the status page.
    const LOG_LINES: usize = 200;

    /// The hosted web of this desktop.
    pub(super) struct Backend {
        web: HostedWeb,
    }

    fn protocol(value: HostProtocol) -> ConfigProtocol {
        match value {
            HostProtocol::Udp => ConfigProtocol::Udp,
            HostProtocol::Tcp => ConfigProtocol::Tcp,
            HostProtocol::Ws => ConfigProtocol::Ws,
        }
    }

    fn host_protocol(value: ConfigProtocol) -> HostProtocol {
        match value {
            ConfigProtocol::Udp => HostProtocol::Udp,
            ConfigProtocol::Tcp => HostProtocol::Tcp,
            ConfigProtocol::Ws => HostProtocol::Ws,
        }
    }

    fn config_of(settings: &HostSettings) -> HostConfig {
        HostConfig {
            api_port: settings.api_port,
            config_port: settings.config_port,
            protocol: protocol(settings.protocol),
            registration_disabled: settings.registration_disabled,
        }
    }

    fn problem(error: HostError) -> HostProblem {
        match error {
            HostError::CoreMissing => HostProblem::CoreMissing,
            HostError::CoreUnsupported(version) => {
                HostProblem::CoreUnsupported(version.to_string())
            }
            HostError::ProgramMissing => HostProblem::ProgramMissing,
            HostError::InvalidSettings(_) => HostProblem::Ports,
            HostError::SettingsCorrupt(_) => HostProblem::SettingsCorrupt,
            HostError::PortInUse { which, port } => HostProblem::PortInUse {
                config: which == HostPort::Config,
                port,
            },
            HostError::AlreadyRunning => HostProblem::AlreadyRunning,
            HostError::NotRunning => HostProblem::NotRunning,
            HostError::Spawn(message) => HostProblem::Spawn(message),
            HostError::Stop(message) => HostProblem::Stop(message),
            HostError::Exited { code, last_log } => {
                let code = code.map_or_else(String::new, |code| format!("exit code {code}"));
                let detail = if last_log.is_empty() { code } else { last_log };
                HostProblem::Exited(detail)
            }
            HostError::StartTimeout => HostProblem::StartTimeout,
            HostError::Io(message) => HostProblem::Files(message),
            HostError::Keychain(message) => HostProblem::Keychain(message),
            HostError::Random(_) => HostProblem::Random,
            HostError::Password { account, problem } => match problem {
                PasswordProblem::Empty => HostProblem::PasswordEmpty(account.to_owned()),
                PasswordProblem::TooShort => HostProblem::PasswordShort(account.to_owned()),
            },
            HostError::Api(message) => HostProblem::Api(message),
            HostError::PasswordUnknown { account } => {
                HostProblem::PasswordUnknown(account.to_owned())
            }
            HostError::ChangeUnconfirmed { account } => {
                HostProblem::Unconfirmed(account.to_owned())
            }
            HostError::RegistrationStillOpen => HostProblem::RegistrationOpen,
        }
    }

    fn outcome(report: &HardeningReport) -> HardeningOutcome {
        HardeningOutcome {
            admin_replaced: report.admin_default_replaced,
            user_replaced: report.user_default_replaced,
            registration_disabled: report.registration_disabled,
            already_hardened: report.already_hardened,
            admin_stored: report.admin_password_stored,
            user_stored: report.user_password_stored,
            config_port: report.config_server_port,
            protocol: protocol(report.protocol),
            error: report.error.clone().map(problem),
            complete: report.is_complete(),
        }
    }

    fn owner_problem(error: OwnerError) -> OwnerProblem {
        match error {
            OwnerError::Host(error) => OwnerProblem::Host(problem(error)),
            OwnerError::CaptchaRefused => OwnerProblem::CaptchaRefused,
            OwnerError::NotStarted => OwnerProblem::NotStarted,
            OwnerError::AlreadyRegistered => OwnerProblem::AlreadyRegistered,
        }
    }

    impl Backend {
        /// The hosted web finding easytier-web through `programs` at each start.
        pub(super) fn open(programs: Arc<dyn ProgramSource>) -> Option<Self> {
            Some(Self {
                web: HostedWeb::new(programs),
            })
        }

        /// Checks `config` against the control plane's own rules (pure).
        pub(super) fn validate(config: &HostConfig) -> Result<(), HostProblem> {
            let settings = HostSettings {
                api_port: config.api_port,
                config_port: config.config_port,
                protocol: host_protocol(config.protocol),
                ..HostSettings::default()
            };
            settings.validate().map_err(problem)
        }

        /// Which of the typed passwords are shorter than the control plane accepts (pure).
        pub(super) fn short_passwords(choice: &PasswordChoice) -> InputErrors {
            let short = |secret: &Option<Secret>| {
                secret
                    .as_ref()
                    .is_some_and(|secret| secret.expose().chars().count() < MIN_PASSWORD_CHARS)
            };
            InputErrors {
                admin_short: short(&choice.admin),
                user_short: short(&choice.user),
            }
        }

        /// Everything the status pages show, from memory (never blocks on the process).
        pub(super) fn snapshot(&self) -> Snapshot {
            let run = match self.web.state() {
                HostState::Stopped => HostRun::Stopped,
                HostState::Starting => HostRun::Starting,
                HostState::Running {
                    api,
                    config_port,
                    protocol: running_protocol,
                    version,
                } => HostRun::Running(RunningHost {
                    api_port: api.port(),
                    config_port,
                    protocol: protocol(running_protocol),
                    version: version.to_string(),
                }),
                HostState::Failed { reason } => HostRun::Failed(problem(reason)),
            };
            let logs = self.web.logs();
            let skip = logs.len().saturating_sub(LOG_LINES);
            Snapshot {
                run,
                config: config_of(&self.web.settings()),
                config_problem: self.web.settings_error().map(problem),
                logs: logs.into_iter().skip(skip).map(|line| line.text).collect(),
            }
        }

        fn settings_with(&self, config: &HostConfig) -> HostSettings {
            HostSettings {
                api_port: config.api_port,
                config_port: config.config_port,
                protocol: host_protocol(config.protocol),
                ..self.web.settings()
            }
        }

        /// Starts easytier-web with `config` (saved first): blocking.
        pub(super) fn start(&self, config: &HostConfig) -> Result<(), HostProblem> {
            self.web.start(&self.settings_with(config)).map_err(problem)
        }

        /// Saves `config` for the next start: blocking.
        pub(super) fn save(&self, config: &HostConfig) -> Result<(), HostProblem> {
            self.web
                .set_settings(&self.settings_with(config))
                .map_err(problem)
        }

        /// Stops easytier-web: blocking.
        pub(super) fn stop(&self) -> Result<(), HostProblem> {
            self.web.stop().map_err(problem)
        }

        /// Saves `config`, then stops and starts: blocking.
        pub(super) fn restart_with(&self, config: &HostConfig) -> Result<(), HostProblem> {
            self.save(config)?;
            self.web.restart().map_err(problem)
        }

        /// Stops and starts with the saved settings (a new release applies): blocking.
        pub(super) fn restart_saved(&self) -> Result<(), HostProblem> {
            self.web.restart().map_err(problem)
        }

        /// First-run hardening: blocking, up to a minute.
        pub(super) fn harden(
            &self,
            choice: PasswordChoice,
            secrets: &dyn SecretStore,
        ) -> HardeningOutcome {
            let request = HardenRequest {
                admin_password: choice.admin,
                user_password: choice.user,
            };
            outcome(&self.web.harden(&request, secrets))
        }

        /// The passwords in the keychain: blocking.
        pub(super) fn passwords(
            &self,
            secrets: &dyn SecretStore,
        ) -> (Option<Secret>, Option<Secret>) {
            let read = |account: &str| match secrets.get(account) {
                Ok(secret) => secret,
                Err(error) => {
                    log::warn!("the keychain could not be read for {account}: {error}");
                    None
                }
            };
            (read(ADMIN_PASSWORD_ACCOUNT), read(USER_PASSWORD_ACCOUNT))
        }

        /// How the console signs in on loopback, once hardening stored the `admin` password: blocking.
        pub(super) fn sign_in(&self, secrets: &dyn SecretStore) -> Option<(String, Secret)> {
            self.web
                .api_access(secrets)
                .map(|access| (access.username, access.password))
        }

        /// Whether the keychain holds an owner account: blocking.
        pub(super) fn owner_registered(&self, secrets: &dyn SecretStore) -> bool {
            self.web.owner_registered(secrets)
        }

        /// What a machine enrolls with under the owner account, once it is registered: blocking.
        pub(super) fn enrollment(&self, secrets: &dyn SecretStore) -> Option<Enrollment> {
            self.web.owner_enrollment_url(secrets).map(|url| {
                let needs_address = url.expose().contains(ENROLLMENT_HOST_PLACEHOLDER);
                Enrollment { url, needs_address }
            })
        }

        /// Opens the registration window and returns the captcha PNG: blocking, up to a few seconds.
        pub(super) fn begin_owner(
            &self,
            secrets: &dyn SecretStore,
        ) -> Result<Vec<u8>, OwnerProblem> {
            self.web
                .begin_owner_registration(secrets)
                .map_err(owner_problem)
        }

        /// Submits the typed captcha text and closes the window: blocking.
        pub(super) fn complete_owner(
            &self,
            captcha: &str,
            secrets: &dyn SecretStore,
        ) -> Result<(), OwnerProblem> {
            self.web
                .complete_owner_registration(captcha, secrets)
                .map_err(owner_problem)
        }

        /// Closes the registration window: blocking.
        pub(super) fn cancel_owner(&self) -> Result<(), HostProblem> {
            self.web.cancel_owner_registration().map_err(problem)
        }

        /// Stops the process for good (the app quits): blocking.
        pub(super) fn shutdown(&self) {
            if let Err(error) = self.web.shutdown() {
                log::warn!("easytier-web could not be stopped at exit: {error}");
            }
        }
    }
}

/// No hosted web where the app cannot run EasyTier's programs: the type has no values, so none of its methods runs.
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod backend {
    use std::sync::Arc;

    use super::{
        Enrollment, HardeningOutcome, HostConfig, HostProblem, InputErrors, OwnerProblem,
        PasswordChoice, Snapshot,
    };
    use crate::core::{keychain::SecretStore, node::ProgramSource, secret::Secret};

    /// Nothing to host on this platform.
    pub(super) enum Backend {}

    impl Backend {
        pub(super) fn open(_programs: Arc<dyn ProgramSource>) -> Option<Self> {
            None
        }

        pub(super) fn validate(_config: &HostConfig) -> Result<(), HostProblem> {
            Err(HostProblem::Unavailable)
        }

        pub(super) fn short_passwords(_choice: &PasswordChoice) -> InputErrors {
            InputErrors::default()
        }

        pub(super) fn snapshot(&self) -> Snapshot {
            match *self {}
        }

        pub(super) fn start(&self, _config: &HostConfig) -> Result<(), HostProblem> {
            match *self {}
        }

        pub(super) fn save(&self, _config: &HostConfig) -> Result<(), HostProblem> {
            match *self {}
        }

        pub(super) fn stop(&self) -> Result<(), HostProblem> {
            match *self {}
        }

        pub(super) fn restart_with(&self, _config: &HostConfig) -> Result<(), HostProblem> {
            match *self {}
        }

        pub(super) fn restart_saved(&self) -> Result<(), HostProblem> {
            match *self {}
        }

        pub(super) fn harden(
            &self,
            _choice: PasswordChoice,
            _secrets: &dyn SecretStore,
        ) -> HardeningOutcome {
            match *self {}
        }

        pub(super) fn passwords(
            &self,
            _secrets: &dyn SecretStore,
        ) -> (Option<Secret>, Option<Secret>) {
            match *self {}
        }

        pub(super) fn owner_registered(&self, _secrets: &dyn SecretStore) -> bool {
            match *self {}
        }

        pub(super) fn enrollment(&self, _secrets: &dyn SecretStore) -> Option<Enrollment> {
            match *self {}
        }

        pub(super) fn begin_owner(
            &self,
            _secrets: &dyn SecretStore,
        ) -> Result<Vec<u8>, OwnerProblem> {
            match *self {}
        }

        pub(super) fn complete_owner(
            &self,
            _captcha: &str,
            _secrets: &dyn SecretStore,
        ) -> Result<(), OwnerProblem> {
            match *self {}
        }

        pub(super) fn cancel_owner(&self) -> Result<(), HostProblem> {
            match *self {}
        }

        pub(super) fn sign_in(&self, _secrets: &dyn SecretStore) -> Option<(String, Secret)> {
            match *self {}
        }

        pub(super) fn shutdown(&self) {
            match *self {}
        }
    }
}

/// See the module docs. A `Copy` handle of signals, like [`Store`].
#[derive(Clone, Copy)]
pub struct HostStore {
    /// What the store's tasks run under: the app, not the page that started them, so leaving a page never cancels
    /// a start or a hardening.
    owner: Owner,
    /// The desktops' hosted web.
    backend: Stored<Option<Arc<Backend>>>,
    secrets: Stored<Arc<dyn SecretStore>>,
    /// The root store, once it exists.
    root: Stored<Option<Store>>,
    /// Bumped by every read of the process, so an older answer is dropped.
    revision: Signal<u64>,
    run: Signal<HostRun>,
    config: Signal<HostConfig>,
    config_problem: Signal<Option<HostProblem>>,
    logs: Signal<Vec<String>>,
    busy: Signal<Busy>,
    problem: Signal<Option<HostProblem>>,
    api_text: Signal<String>,
    config_text: Signal<String>,
    protocol: Signal<ConfigProtocol>,
    form_error: Signal<Option<FormError>>,
    hardening: Signal<HardeningState>,
    stored: Signal<StoredPasswords>,
    revealed: Signal<Option<RevealedPasswords>>,
    admin_input: Signal<Secret>,
    user_input: Signal<Secret>,
    input_errors: Signal<InputErrors>,
    deploy: Signal<DeployForm>,
    deployed: Memo<DeployOutcome>,
    /// The file the deployment page shows, by name (the first one while none is chosen).
    selected_file: Signal<Option<String>>,
    /// Whether the settings drawer is open.
    settings_open: Signal<bool>,
    /// Whether the revealed-passwords drawer is open.
    reveal_open: Signal<bool>,
    /// When the process was last read, in Unix milliseconds.
    updated: Signal<Option<f64>>,
    /// The keychain holds an owner account.
    owner_registered: Signal<bool>,
    owner_step: Signal<OwnerStep>,
    owner_problem: Signal<Option<OwnerProblem>>,
    captcha_input: Signal<String>,
    enrollment: Signal<Option<Enrollment>>,
    /// The person left while the captcha was being fetched: the window closes as soon as it opens.
    owner_abandoned: Signal<bool>,
}

impl HostStore {
    /// Its state at start.
    pub(super) fn new(env: &StoreEnv) -> Self {
        let backend = env
            .programs
            .clone()
            .filter(|_| env.platform.is_desktop())
            .and_then(Backend::open)
            .map(Arc::new);
        let deploy = signal(DeployForm::default());
        let defaults = HostConfig::default();
        Self {
            owner: Owner::current(),
            backend: stored(backend),
            secrets: stored(env.secrets.clone()),
            root: stored(None),
            revision: signal(0),
            run: signal(HostRun::Stopped),
            config: signal(defaults.clone()),
            config_problem: signal(None),
            logs: signal(Vec::new()),
            busy: signal(Busy::Idle),
            problem: signal(None),
            api_text: signal(defaults.api_port.to_string()),
            config_text: signal(defaults.config_port.to_string()),
            protocol: signal(defaults.protocol),
            form_error: signal(None),
            hardening: signal(HardeningState::Idle),
            stored: signal(StoredPasswords::default()),
            revealed: signal(None),
            admin_input: signal(Secret::default()),
            user_input: signal(Secret::default()),
            input_errors: signal(InputErrors::default()),
            deploy,
            deployed: computed(move || outcome_of(&deploy.get())),
            selected_file: signal(None),
            settings_open: signal(false),
            reveal_open: signal(false),
            updated: signal(None),
            owner_registered: signal(false),
            owner_step: signal(OwnerStep::Idle),
            owner_problem: signal(None),
            captcha_input: signal(String::new()),
            enrollment: signal(None),
            owner_abandoned: signal(false),
        }
    }

    /// Restores what the role kept across runs, once the root store exists: reads the hosted web, keeps the
    /// target lamp truthful while it runs, and stops it when the app quits.
    pub(super) fn restore(&self, store: Store) {
        self.root.with(|root| *root = Some(store));
        let Some(backend) = self.backend.get() else {
            return;
        };
        let first = backend.snapshot();
        self.fill_form(&first.config);
        self.apply(first);
        self.read_stored();
        let this = *self;
        set_interval(POLL, move || this.poll());
        cleanup(move || backend.shutdown());
    }

    /// Whether this device hosts a control plane itself (a desktop with its programs).
    pub fn can_host(&self) -> bool {
        self.backend.get().is_some()
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

    /// Where the control plane is (tracked).
    pub fn run(&self) -> HostRun {
        self.run.get()
    }

    /// Whether the control plane is starting or running (tracked).
    pub fn is_hosting(&self) -> bool {
        matches!(self.run.get(), HostRun::Starting | HostRun::Running(_))
    }

    /// The saved settings (tracked).
    pub fn config(&self) -> HostConfig {
        self.config.get()
    }

    /// Why the saved settings were unreadable, if they were (tracked).
    pub fn config_problem(&self) -> Option<HostProblem> {
        self.config_problem.get()
    }

    /// The process output, oldest first (tracked).
    pub fn logs(&self) -> Vec<String> {
        self.logs.get()
    }

    /// What the store is doing to the control plane (tracked).
    pub fn busy(&self) -> Busy {
        self.busy.get()
    }

    /// The last action's failure (tracked).
    pub fn problem(&self) -> Option<HostProblem> {
        self.problem.get()
    }

    /// Forgets the last action's failure.
    pub fn dismiss_problem(&self) {
        self.problem.set(None);
    }

    /// When the process was last read, in Unix milliseconds (tracked).
    pub fn updated_ms(&self) -> Option<f64> {
        self.updated.get()
    }

    /// Whether the settings drawer is open (tracked).
    pub fn settings_open(&self) -> bool {
        self.settings_open.get()
    }

    /// Opens the settings drawer on the saved settings.
    pub fn open_settings_editor(&self) {
        self.fill_form(&self.config.get_untracked());
        self.settings_open.set(true);
    }

    /// Closes the settings drawer and drops the edits that were not saved.
    pub fn close_settings_editor(&self) {
        self.settings_open.set(false);
        self.fill_form(&self.config.get_untracked());
    }

    // --- The settings form ---------------------------------------------------------------------------------

    /// The REST API port as typed (tracked).
    pub fn api_port_text(&self) -> String {
        self.api_text.get()
    }

    /// Sets the REST API port text.
    pub fn set_api_port_text(&self, text: &str) {
        batch(|| {
            self.api_text.set(text.to_owned());
            self.form_error.set(None);
        });
    }

    /// The config server port as typed (tracked).
    pub fn config_port_text(&self) -> String {
        self.config_text.get()
    }

    /// Sets the config server port text.
    pub fn set_config_port_text(&self, text: &str) {
        batch(|| {
            self.config_text.set(text.to_owned());
            self.form_error.set(None);
        });
    }

    /// The chosen config server protocol (tracked).
    pub fn protocol(&self) -> ConfigProtocol {
        self.protocol.get()
    }

    /// Chooses the config server protocol.
    pub fn set_protocol(&self, protocol: ConfigProtocol) {
        batch(|| {
            self.protocol.set(protocol);
            self.form_error.set(None);
        });
    }

    /// What is wrong with the form, after a press (tracked).
    pub fn form_error(&self) -> Option<FormError> {
        self.form_error.get()
    }

    /// Whether the form differs from the saved settings (tracked).
    pub fn form_dirty(&self) -> bool {
        let saved = self.config.get();
        self.api_text.get().trim() != saved.api_port.to_string()
            || self.config_text.get().trim() != saved.config_port.to_string()
            || self.protocol.get() != saved.protocol
    }

    fn fill_form(&self, config: &HostConfig) {
        batch(|| {
            self.api_text.set(config.api_port.to_string());
            self.config_text.set(config.config_port.to_string());
            self.protocol.set(config.protocol);
            self.form_error.set(None);
        });
    }

    /// The form as settings, or what is wrong with it.
    fn form_config(&self) -> Result<HostConfig, FormError> {
        let api_port = self
            .api_text
            .get_untracked()
            .trim()
            .parse::<u16>()
            .map_err(|_| FormError::ApiNotNumber)?;
        let config_port = self
            .config_text
            .get_untracked()
            .trim()
            .parse::<u16>()
            .map_err(|_| FormError::ConfigNotNumber)?;
        let config = HostConfig {
            api_port,
            config_port,
            protocol: self.protocol.get_untracked(),
            registration_disabled: self.config.get_untracked().registration_disabled,
        };
        Backend::validate(&config).map_err(|_| FormError::Invalid)?;
        Ok(config)
    }

    // --- Reading the process -------------------------------------------------------------------------------

    fn apply(&self, snapshot: Snapshot) {
        let Snapshot {
            run,
            config,
            config_problem,
            logs,
        } = snapshot;
        let api_port = match &run {
            HostRun::Running(running) => running.api_port,
            _ => config.api_port,
        };
        batch(|| {
            self.run.set(run.clone());
            self.config.set(config);
            self.config_problem.set(config_problem);
            self.logs.set(logs);
            self.updated.set(Some(unix_ms()));
            // Stopping the process closes the registration window with it.
            if matches!(run, HostRun::Stopped | HostRun::Failed(_))
                && matches!(self.owner_step.get_untracked(), OwnerStep::Waiting { .. })
            {
                self.owner_step.set(OwnerStep::Idle);
            }
        });
        self.sync_target(&run, api_port);
    }

    /// Keeps the hosted control plane's target in the switcher as truthful as the process.
    fn sync_target(&self, run: &HostRun, api_port: u16) {
        let Some(store) = self.root.get() else {
            return;
        };
        let (status, port) = match run {
            HostRun::Stopped => {
                if untracked(|| store.target(TargetId::HOSTED).is_some()) {
                    store.remove_target(TargetId::HOSTED);
                }
                return;
            }
            HostRun::Starting => (TargetStatus::Connecting, api_port),
            HostRun::Running(running) => (TargetStatus::Online, running.api_port),
            HostRun::Failed(_) => (TargetStatus::Failed, api_port),
        };
        let origin = TargetOrigin::Loopback { port };
        let current = untracked(|| store.target(TargetId::HOSTED));
        match current {
            Some(target) if target.origin == origin && target.kind == TargetKind::Hosted => {
                store.set_target_status(TargetId::HOSTED, status);
            }
            _ => store.put_target(Target {
                id: TargetId::HOSTED,
                kind: TargetKind::Hosted,
                origin,
                name: None,
                status,
            }),
        }
    }

    /// Reads the process off-thread and applies it unless a newer read started meanwhile.
    pub fn refresh(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        let this = *self;
        let revision = self.revision.get_untracked().saturating_add(1);
        self.revision.set(revision);
        self.launch(async move {
            let read = off_thread(move || backend.snapshot()).await;
            if this.revision.try_get() != Some(revision) {
                return;
            }
            match read {
                Ok(snapshot) => this.apply(snapshot),
                Err(error) => log::warn!("reading the control plane failed: {error}"),
            }
        });
    }

    /// The once-a-second read; a stopped control plane is not read.
    fn poll(&self) {
        let idle = self.busy.get_untracked() == Busy::Idle;
        let stopped = self.run.get_untracked() == HostRun::Stopped;
        if !(idle && stopped) {
            self.refresh();
        }
    }

    /// Reads which passwords the keychain holds, and whether the owner account is registered (with what machines
    /// enroll by).
    fn read_stored(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        let secrets = self.secrets.get();
        let this = *self;
        self.launch(async move {
            let read = off_thread(move || {
                let passwords = backend.passwords(secrets.as_ref());
                let registered = backend.owner_registered(secrets.as_ref());
                let enrollment = if registered {
                    backend.enrollment(secrets.as_ref())
                } else {
                    None
                };
                (passwords, registered, enrollment)
            })
            .await;
            match read {
                Ok(((admin, user), registered, enrollment)) => {
                    batch(|| {
                        this.stored.try_set(StoredPasswords {
                            admin: admin.is_some(),
                            user: user.is_some(),
                        });
                        this.owner_registered.try_set(registered);
                        this.enrollment.try_set(enrollment);
                    });
                }
                Err(error) => log::warn!("reading the keychain failed: {error}"),
            }
        });
    }

    // --- Actions -------------------------------------------------------------------------------------------

    /// Runs `job` against the hosted web off-thread under `busy`, then reads the process.
    fn act(
        &self,
        busy: Busy,
        job: impl FnOnce(&Backend) -> Result<(), HostProblem> + Send + 'static,
    ) {
        self.act_then(busy, false, job);
    }

    /// As [`act`](Self::act); a job that succeeds also closes the settings drawer when `close_settings` holds.
    fn act_then(
        &self,
        busy: Busy,
        close_settings: bool,
        job: impl FnOnce(&Backend) -> Result<(), HostProblem> + Send + 'static,
    ) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        if self.busy.get_untracked() != Busy::Idle {
            return;
        }
        batch(|| {
            self.busy.set(busy);
            self.problem.set(None);
        });
        let this = *self;
        self.launch(async move {
            let done = off_thread(move || job(backend.as_ref())).await;
            let result = done.unwrap_or_else(|error| {
                log::warn!("the control-plane task failed: {error}");
                Err(HostProblem::Interrupted)
            });
            if !this.alive() {
                return;
            }
            batch(|| {
                this.busy.set(Busy::Idle);
                match result {
                    Ok(()) => {
                        if close_settings {
                            this.settings_open.set(false);
                        }
                    }
                    Err(problem) => this.problem.set(Some(problem)),
                }
            });
            this.refresh();
        });
    }

    /// The form's settings, or the form error shown beside the fields.
    fn checked_form(&self) -> Option<HostConfig> {
        match self.form_config() {
            Ok(config) => {
                self.form_error.set(None);
                Some(config)
            }
            Err(error) => {
                self.form_error.set(Some(error));
                None
            }
        }
    }

    /// Starts the control plane with the form's settings.
    pub fn start(&self) {
        if let Some(config) = self.checked_form() {
            self.act(Busy::Starting, move |backend| backend.start(&config));
        }
    }

    /// Stops the control plane.
    pub fn stop(&self) {
        self.act(Busy::Stopping, |backend| backend.stop());
    }

    /// Restarts the control plane with the form's settings.
    pub fn restart(&self) {
        if let Some(config) = self.checked_form() {
            self.act(Busy::Restarting, move |backend| {
                backend.restart_with(&config)
            });
        }
    }

    /// Saves the form's settings for the next start; the settings drawer closes once they are saved.
    pub fn save_settings(&self) {
        if let Some(config) = self.checked_form() {
            self.act_then(Busy::Saving, true, move |backend| backend.save(&config));
        }
    }

    /// Restarts the control plane with its saved settings if it runs, so a newly activated release applies.
    pub fn restart_if_running(&self) -> impl Future<Output = ()> + use<> {
        let this = *self;
        let backend = self.backend.get();
        async move {
            let Some(backend) = backend else {
                return;
            };
            if !matches!(
                this.run.try_get(),
                Some(HostRun::Starting | HostRun::Running(_))
            ) {
                return;
            }
            this.busy.try_set(Busy::Restarting);
            let done = off_thread(move || backend.restart_saved()).await;
            let result = done.unwrap_or_else(|error| {
                log::warn!("restarting the control plane failed: {error}");
                Err(HostProblem::Interrupted)
            });
            if !this.alive() {
                return;
            }
            batch(|| {
                this.busy.set(Busy::Idle);
                if let Err(problem) = result {
                    this.problem.set(Some(problem));
                }
            });
            this.refresh();
        }
    }

    /// The username and password the console signs in with on the hosted control plane, from the keychain: `None`
    /// until hardening stored the `admin` password.
    pub fn hosted_sign_in(&self) -> impl Future<Output = Option<(String, Secret)>> + use<> {
        let backend = self.backend.get();
        let secrets = self.secrets.get();
        async move {
            let backend = backend?;
            off_thread(move || backend.sign_in(secrets.as_ref()))
                .await
                .unwrap_or_else(|error| {
                    log::warn!("reading the hosted sign-in failed: {error}");
                    None
                })
        }
    }

    /// Selects the hosted control plane in the console and opens its machines.
    pub fn manage_in_console(&self) {
        let Some(store) = self.root.get() else {
            return;
        };
        store.select_target(TargetId::HOSTED);
        store.open(Page::Machines);
    }

    // --- Hardening -----------------------------------------------------------------------------------------

    /// Where hardening is (tracked).
    pub fn hardening(&self) -> HardeningState {
        self.hardening.get()
    }

    /// Which default passwords are replaced, from what hardening reported or the keychain holds (tracked).
    pub fn checks(&self) -> HardeningChecks {
        let stored = self.stored.get();
        let saved = self.config.get();
        let report = match self.hardening.get() {
            HardeningState::Done(outcome) => Some(outcome),
            HardeningState::Idle | HardeningState::Running => None,
        };
        let line = |reported: Option<bool>, known: bool| match (reported, known) {
            (Some(true), _) | (None, true) => Check::Pass,
            (Some(false), _) => Check::Open,
            (None, false) => Check::Unknown,
        };
        HardeningChecks {
            admin: line(report.as_ref().map(|r| r.admin_replaced), stored.admin),
            user: line(report.as_ref().map(|r| r.user_replaced), stored.user),
            registration: if matches!(
                self.owner_step.get(),
                OwnerStep::Waiting { .. } | OwnerStep::Submitting { .. }
            ) {
                // The owner-registration window is open on loopback until the captcha is answered or given up.
                Check::Open
            } else if report.is_none() && !self.is_hosting() {
                // Registration is off from every start, but nothing running has been checked.
                Check::Unknown
            } else {
                line(
                    report.as_ref().map(|r| r.registration_disabled),
                    saved.registration_disabled,
                )
            },
            owner: if self.owner_registered.get() {
                Check::Pass
            } else if self.is_hosting() {
                Check::Open
            } else {
                Check::Unknown
            },
        }
    }

    /// Whether anything is known about this control plane's hardening (tracked): what the keychain holds or what
    /// a hardening run reported. That registration starts off is a fact of how it is built, not something found out.
    pub fn hardening_known(&self) -> bool {
        let checks = self.checks();
        [checks.admin, checks.user, checks.owner]
            .iter()
            .any(|check| *check != Check::Unknown)
    }

    /// Which passwords the keychain holds (tracked).
    pub fn stored_passwords(&self) -> StoredPasswords {
        self.stored.get()
    }

    /// The typed `admin` password (tracked).
    pub fn admin_input(&self) -> Secret {
        self.admin_input.get()
    }

    /// Sets the typed `admin` password.
    pub fn set_admin_input(&self, text: &str) {
        batch(|| {
            self.admin_input.set(Secret::new(text));
            self.input_errors
                .update(|errors| errors.admin_short = false);
        });
    }

    /// The typed `user` password (tracked).
    pub fn user_input(&self) -> Secret {
        self.user_input.get()
    }

    /// Sets the typed `user` password.
    pub fn set_user_input(&self, text: &str) {
        batch(|| {
            self.user_input.set(Secret::new(text));
            self.input_errors.update(|errors| errors.user_short = false);
        });
    }

    /// Fills the `admin` password field with a generated password.
    pub fn generate_admin(&self) {
        self.generate_into(self.admin_input);
    }

    /// Fills the `user` password field with a generated password.
    pub fn generate_user(&self) {
        self.generate_into(self.user_input);
    }

    fn generate_into(&self, field: Signal<Secret>) {
        match Secret::generate() {
            Ok(secret) => {
                field.set(secret);
                self.input_errors.set(InputErrors::default());
            }
            Err(error) => {
                log::warn!("a password could not be generated: {error}");
                self.problem.set(Some(HostProblem::Random));
            }
        }
    }

    /// Which typed passwords are too short, after a press (tracked).
    pub fn input_errors(&self) -> InputErrors {
        self.input_errors.get()
    }

    /// Replaces the default passwords (typed ones, or generated for an empty field), starting the control plane
    /// if it is stopped, and closes registration.
    pub fn harden(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        if self.busy.get_untracked() != Busy::Idle {
            return;
        }
        let typed = |secret: Secret| (!secret.is_empty()).then_some(secret);
        let choice = PasswordChoice {
            admin: typed(self.admin_input.get_untracked()),
            user: typed(self.user_input.get_untracked()),
        };
        let short = Backend::short_passwords(&choice);
        self.input_errors.set(short);
        if short.admin_short || short.user_short {
            return;
        }
        batch(|| {
            self.busy.set(Busy::Hardening);
            self.hardening.set(HardeningState::Running);
            self.problem.set(None);
            self.revealed.set(None);
            self.reveal_open.set(false);
        });
        let secrets = self.secrets.get();
        let this = *self;
        self.launch(async move {
            let done = off_thread(move || backend.harden(choice, secrets.as_ref())).await;
            if !this.alive() {
                return;
            }
            batch(|| {
                this.busy.set(Busy::Idle);
                match done {
                    Ok(outcome) => this.hardening.set(HardeningState::Done(outcome)),
                    Err(error) => {
                        log::warn!("hardening ended without a result: {error}");
                        this.hardening.set(HardeningState::Idle);
                        this.problem.set(Some(HostProblem::Interrupted));
                    }
                }
                this.admin_input.set(Secret::default());
                this.user_input.set(Secret::default());
            });
            this.read_stored();
            this.refresh();
        });
    }

    /// The passwords revealed on request (tracked).
    pub fn revealed(&self) -> Option<RevealedPasswords> {
        self.revealed.get()
    }

    /// Whether the revealed-passwords drawer is open (tracked).
    pub fn reveal_open(&self) -> bool {
        self.reveal_open.get()
    }

    /// Opens the drawer that shows the passwords the keychain holds, and reads them: they are shown only because
    /// the person asked.
    pub fn open_reveal(&self) {
        self.reveal_open.set(true);
        self.read_revealed();
    }

    /// Closes the drawer and forgets the revealed passwords.
    pub fn close_reveal(&self) {
        batch(|| {
            self.reveal_open.set(false);
            self.revealed.set(None);
        });
    }

    /// Reads the passwords from the keychain to show them.
    fn read_revealed(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        let secrets = self.secrets.get();
        let this = *self;
        self.launch(async move {
            let read = off_thread(move || backend.passwords(secrets.as_ref())).await;
            match read {
                Ok((admin, user)) => {
                    // The drawer may have been closed while the keychain answered.
                    if this.reveal_open.try_get() == Some(true) {
                        this.revealed
                            .try_set(Some(RevealedPasswords { admin, user }));
                    }
                }
                Err(error) => log::warn!("reading the keychain failed: {error}"),
            }
        });
    }

    /// The Hardening page was built: forgets revealed passwords and re-reads the keychain, after the build.
    pub fn hardening_opened(&self) {
        let this = *self;
        self.launch(async move {
            this.revealed.try_set(None);
            this.reveal_open.try_set(false);
            this.read_stored();
        });
    }

    // --- The owner account ---------------------------------------------------------------------------------

    /// Whether the keychain holds an owner account (tracked).
    pub fn owner_registered(&self) -> bool {
        self.owner_registered.get()
    }

    /// Where the owner-account step is (tracked).
    pub fn owner_step(&self) -> OwnerStep {
        self.owner_step.get()
    }

    /// Why the owner-account step failed, until the next attempt (tracked).
    pub fn owner_problem(&self) -> Option<OwnerProblem> {
        self.owner_problem.get()
    }

    /// The captcha text as typed (tracked).
    pub fn captcha_input(&self) -> String {
        self.captcha_input.get()
    }

    /// Sets the captcha text.
    pub fn set_captcha_input(&self, text: &str) {
        self.captcha_input.set(text.to_owned());
        // Only a standing problem is cleared: setting it again would redraw the step under the person's hands.
        if self.owner_problem.get_untracked().is_some() {
            self.owner_problem.set(None);
        }
    }

    /// What a machine enrolls with under the owner account, once it is registered (tracked).
    pub fn enrollment(&self) -> Option<Enrollment> {
        self.enrollment.get()
    }

    /// Opens the short registration window (on this computer only) and asks easytier-web for a captcha, which
    /// the person reads and types.
    pub fn begin_owner(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        if self.busy.get_untracked() != Busy::Idle
            || self.owner_step.get_untracked() != OwnerStep::Idle
        {
            return;
        }
        batch(|| {
            self.busy.set(Busy::Owner);
            self.owner_step.set(OwnerStep::Preparing);
            self.owner_problem.set(None);
            self.problem.set(None);
            self.captcha_input.set(String::new());
            self.owner_abandoned.set(false);
        });
        let secrets = self.secrets.get();
        let this = *self;
        self.launch(async move {
            let done = off_thread(move || backend.begin_owner(secrets.as_ref()))
                .await
                .unwrap_or_else(|error| {
                    log::warn!("opening the owner registration failed: {error}");
                    Err(OwnerProblem::Host(HostProblem::Interrupted))
                });
            if !this.alive() {
                return;
            }
            let abandoned = this.owner_abandoned.try_get() == Some(true);
            batch(|| {
                this.busy.set(Busy::Idle);
                match done {
                    Ok(_) if abandoned => this.owner_step.set(OwnerStep::Idle),
                    Ok(png) => this.owner_step.set(OwnerStep::Waiting {
                        captcha: format!("data:image/png;base64,{}", STANDARD.encode(png)),
                    }),
                    Err(problem) => {
                        this.owner_step.set(OwnerStep::Idle);
                        this.owner_problem.set(Some(problem));
                    }
                }
            });
            if abandoned {
                this.close_owner_window();
            }
            this.read_stored();
            this.refresh();
        });
    }

    /// Submits the typed captcha text; a registered owner account lands in the keychain.
    pub fn submit_captcha(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        if self.busy.get_untracked() != Busy::Idle {
            return;
        }
        let OwnerStep::Waiting { captcha } = self.owner_step.get_untracked() else {
            return;
        };
        let typed = self.captcha_input.get_untracked().trim().to_owned();
        if typed.is_empty() {
            return;
        }
        batch(|| {
            self.busy.set(Busy::Owner);
            self.owner_step.set(OwnerStep::Submitting {
                captcha: captcha.clone(),
            });
            self.owner_problem.set(None);
        });
        let secrets = self.secrets.get();
        let this = *self;
        self.launch(async move {
            let done = off_thread(move || backend.complete_owner(&typed, secrets.as_ref()))
                .await
                .unwrap_or_else(|error| {
                    log::warn!("registering the owner account failed: {error}");
                    Err(OwnerProblem::Host(HostProblem::Interrupted))
                });
            if !this.alive() {
                return;
            }
            batch(|| {
                this.busy.set(Busy::Idle);
                this.captcha_input.set(String::new());
                match done {
                    Ok(()) => this.owner_step.set(OwnerStep::Idle),
                    // The window is still open: the same image can be answered again.
                    Err(OwnerProblem::CaptchaRefused) => {
                        this.owner_step.set(OwnerStep::Waiting { captcha });
                        this.owner_problem.set(Some(OwnerProblem::CaptchaRefused));
                    }
                    Err(problem) => {
                        this.owner_step.set(OwnerStep::Idle);
                        this.owner_problem.set(Some(problem));
                    }
                }
            });
            this.read_stored();
            this.refresh();
        });
    }

    /// Gives up the registration: one that waits for its captcha closes its window now, one still fetching the
    /// captcha closes it as soon as it opens.
    pub fn cancel_owner(&self) {
        // The app is quitting (the page's cleanup runs then too): the process is stopped with it.
        if !self.alive() {
            return;
        }
        match self.owner_step.get_untracked() {
            OwnerStep::Preparing => self.owner_abandoned.set(true),
            OwnerStep::Waiting { .. } => {
                batch(|| {
                    self.owner_step.set(OwnerStep::Idle);
                    self.owner_problem.set(None);
                    self.captcha_input.set(String::new());
                });
                self.close_owner_window();
            }
            OwnerStep::Idle | OwnerStep::Submitting { .. } => {}
        }
    }

    /// Closes the registration window of the hosted web, off-thread.
    fn close_owner_window(&self) {
        let Some(backend) = self.backend.get() else {
            return;
        };
        let this = *self;
        self.launch(async move {
            let done = off_thread(move || backend.cancel_owner()).await;
            match done {
                Ok(Ok(())) => {}
                Ok(Err(problem)) => {
                    this.problem.try_set(Some(problem));
                }
                Err(error) => log::warn!("closing the owner registration failed: {error}"),
            }
            this.refresh();
        });
    }

    // --- Deployment ----------------------------------------------------------------------------------------

    /// The deployment form (tracked).
    pub fn deploy_form(&self) -> DeployForm {
        self.deploy.get()
    }

    /// Changes the deployment form.
    pub fn edit_deploy(&self, change: impl FnOnce(&mut DeployForm)) {
        self.deploy.update(change);
    }

    /// The file the deployment page shows: the one chosen, else the first (tracked).
    pub fn selected_artifact(&self) -> Option<Artifact> {
        let DeployOutcome::Ready(files) = self.deployed.get() else {
            return None;
        };
        let index = self
            .selected_file
            .get()
            .and_then(|name| files.iter().position(|file| file.file_name == name))
            .unwrap_or(0);
        files.into_iter().nth(index)
    }

    /// Shows the file named `name` on the deployment page.
    pub fn select_artifact(&self, name: &str) {
        self.selected_file.set(Some(name.to_owned()));
    }

    /// What the deployment form renders to (tracked).
    pub fn deploy_outcome(&self) -> DeployOutcome {
        self.deployed.get()
    }
}
