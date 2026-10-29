//! Why the hosted control plane did not start, stopped, or could not be hardened.

use crate::core::{node::NodeError, version::CoreVersion};

/// Which of the two ports a conflict is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPort {
    /// The REST API port (loopback, TCP).
    Api,
    /// The config-server port (every interface, UDP or TCP by protocol).
    Config,
}

/// What is wrong with a password the user supplied for a default account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasswordProblem {
    /// Empty.
    #[error("the password is empty")]
    Empty,
    /// Shorter than [`MIN_PASSWORD_CHARS`](super::MIN_PASSWORD_CHARS).
    #[error("the password is too short")]
    TooShort,
}

/// Why a control-plane operation failed. Messages never carry passwords or cookies; they are safe to show.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostError {
    /// No EasyTier release is installed.
    #[error("EasyTier is not installed")]
    CoreMissing,
    /// The active release is older than the app supports.
    #[error("EasyTier {0} is older than the oldest supported release")]
    CoreUnsupported(CoreVersion),
    /// The active release has no `easytier-web` program.
    #[error("the active EasyTier release has no easytier-web program")]
    ProgramMissing,
    /// The settings cannot work; the text names the field.
    #[error("invalid settings: {0}")]
    InvalidSettings(&'static str),
    /// The settings file could not be read as settings.
    #[error("the saved settings are unreadable: {0}")]
    SettingsCorrupt(String),
    /// Another program already listens on a port the control plane needs.
    #[error("port {port} is already in use")]
    PortInUse {
        /// Which port.
        which: HostPort,
        /// The number.
        port: u16,
    },
    /// A start was asked while the process is starting or running.
    #[error("the control plane is already running")]
    AlreadyRunning,
    /// An operation needs the control plane to be running.
    #[error("the control plane is not running")]
    NotRunning,
    /// The process could not be spawned.
    #[error("easytier-web could not start: {0}")]
    Spawn(String),
    /// The process could not be killed or reaped.
    #[error("easytier-web could not be stopped: {0}")]
    Stop(String),
    /// The process ended on its own; the last lines it printed (redacted).
    #[error("easytier-web exited unexpectedly (code {code:?})")]
    Exited {
        /// The exit code; none when a signal ended it.
        code: Option<i32>,
        /// Its last output lines, redacted.
        last_log: String,
    },
    /// The process runs but its API never answered.
    #[error("easytier-web did not answer in time")]
    StartTimeout,
    /// A file or directory under the data directory failed.
    #[error("file error: {0}")]
    Io(String),
    /// The keychain refused a read or write.
    #[error("the system keychain failed: {0}")]
    Keychain(String),
    /// No secure random source for generated passwords.
    #[error("could not generate a password: {0}")]
    Random(String),
    /// A supplied password was refused before anything changed.
    #[error("the {account} password was refused: {problem}")]
    Password {
        /// `admin` or `user`.
        account: &'static str,
        /// What is wrong with it.
        problem: PasswordProblem,
    },
    /// The API could not be reached or answered something unexpected.
    #[error("the control-plane API failed: {0}")]
    Api(String),
    /// An account no longer accepts its default password but no password of ours signs in either, so it cannot be
    /// managed from here (the database came from elsewhere, or the keychain entry is gone).
    #[error(
        "the {account} account was changed by someone else; its password is not in the keychain"
    )]
    PasswordUnknown {
        /// `admin` or `user`.
        account: &'static str,
    },
    /// The new password was accepted but the old one still signs in, or the new one does not.
    #[error("the {account} password change could not be confirmed")]
    ChangeUnconfirmed {
        /// `admin` or `user`.
        account: &'static str,
    },
    /// Registration is still open after the restart that should have closed it.
    #[error("registration is still open after the restart")]
    RegistrationStillOpen,
}

impl From<NodeError> for HostError {
    fn from(error: NodeError) -> Self {
        match error {
            NodeError::CoreMissing => Self::CoreMissing,
            NodeError::CoreUnsupported(version) => Self::CoreUnsupported(version),
            other => Self::Spawn(other.to_string()),
        }
    }
}
