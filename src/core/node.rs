//! The node role's backend contract: what the store asks of `sidecar` (desktops, the official `easytier-core`
//! process) and `embedded` (iOS / Android, EasyTier's library in process).
//!
//! Every method blocks (process spawns, CLI reads, library calls): the store calls them through
//! [`super::work::off_thread`], never on the engine thread. Implementations are `Send + Sync` and keep their own
//! per-instance state behind locks.

use std::{fmt, path::PathBuf};

use super::{
    secret::Secret,
    state::{FailReason, LogLine, NetworkState},
    version::CoreVersion,
};

/// One node instance's stable key, derived from its profile id (never from user text): `p-` and 1–64 ASCII
/// alphanumerics. It names the instance's config file, log directory and OS service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceKey(String);

impl InstanceKey {
    /// The key of the profile `profile_id` (`[A-Za-z0-9]{1,64}`); `None` for anything else.
    pub fn for_profile(profile_id: &str) -> Option<Self> {
        let valid = (1..=64).contains(&profile_id.len())
            && profile_id.chars().all(|c| c.is_ascii_alphanumeric());
        valid.then(|| Self(format!("p-{profile_id}")))
    }

    /// The key as text (`p-…`).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstanceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What the core needs to be allowed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privilege {
    /// No TUN interface (`no_tun`): runs as the user, as the app's child process.
    User,
    /// A TUN interface: on the desktops, an OS service the user agreed to install (one administrator prompt per
    /// install, start or stop); it keeps running after the app quits.
    System,
}

/// Where an instance's network config comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchSource {
    /// A profile edited here, rendered to EasyTier's TOML (it holds the network secret, hence `Secret`).
    Local {
        /// The rendered config file.
        config_toml: Secret,
    },
    /// Enrolled to a control plane, which pushes the configs. Read-only here except "leave management".
    Managed {
        /// `udp://host:22020/<user>`: an enrollment credential, never put on a command line.
        config_server: Secret,
        /// This installation's machine id (a UUID, generated once and kept).
        machine_id: String,
        /// The hostname to report, if the user set one.
        hostname: Option<String>,
    },
}

/// Everything a backend needs to start one instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeLaunch {
    /// The instance.
    pub key: InstanceKey,
    /// What it must be allowed to do. Managed instances always run as [`Privilege::User`].
    pub privilege: Privilege,
    /// Its config.
    pub source: LaunchSource,
}

/// Why a backend could not do what the store asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NodeError {
    /// No EasyTier program is installed (or the active one is gone).
    #[error("EasyTier is not installed")]
    CoreMissing,
    /// The installed program is too old.
    #[error("EasyTier {0} is older than the oldest supported release")]
    CoreUnsupported(CoreVersion),
    /// The core rejected the config; the message is redacted.
    #[error("the config was rejected: {0}")]
    ConfigRejected(String),
    /// The process could not be started.
    #[error("the core could not start: {0}")]
    Spawn(String),
    /// TUN needs privileges this launch does not have.
    #[error("creating the network interface needs administrator rights")]
    NoPrivileges,
    /// The user cancelled the administrator prompt or the OS refused it.
    #[error("administrator approval was not given")]
    ElevationDenied,
    /// Installing, starting, stopping or removing the OS service failed.
    #[error("the system service failed: {0}")]
    Service(String),
    /// A file of the instance could not be written or read.
    #[error("file error: {0}")]
    Io(String),
    /// This backend cannot do it on this platform.
    #[error("not available on this platform: {0}")]
    Unsupported(&'static str),
    /// Anything else.
    #[error("{0}")]
    Other(String),
}

impl NodeError {
    /// The UI state of an instance that failed with this error.
    pub fn into_fail_reason(self) -> FailReason {
        match self {
            Self::CoreMissing => FailReason::CoreMissing,
            Self::CoreUnsupported(version) => FailReason::CoreUnsupported(version),
            Self::ConfigRejected(message) => FailReason::ConfigRejected(message),
            Self::NoPrivileges => FailReason::NoPrivileges,
            Self::ElevationDenied => FailReason::ElevationDenied,
            Self::Spawn(message)
            | Self::Service(message)
            | Self::Io(message)
            | Self::Other(message) => FailReason::Other(message),
            Self::Unsupported(what) => FailReason::Other(what.to_owned()),
        }
    }
}

/// The EasyTier programs of one release on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Programs {
    /// Their version, from `easytier-core --version`.
    pub version: CoreVersion,
    /// The directory they are in.
    pub dir: PathBuf,
    /// `easytier-core`.
    pub core: PathBuf,
    /// `easytier-cli` of the same release.
    pub cli: PathBuf,
    /// `easytier-web` of the same release.
    pub web: PathBuf,
}

/// Where the active EasyTier release is: implemented by `core::install`, read by `sidecar` and `controlplane` at
/// each start so a version switch applies to the next start.
pub trait ProgramSource: Send + Sync {
    /// The active release.
    ///
    /// # Errors
    ///
    /// [`NodeError::CoreMissing`] when none is installed, [`NodeError::CoreUnsupported`] when it is too old.
    fn active(&self) -> Result<Programs, NodeError>;
}

/// A node backend. See the module docs: every method blocks.
pub trait NodeBackend: Send + Sync {
    /// Validates `launch`'s config with the core without starting a network.
    ///
    /// # Errors
    ///
    /// [`NodeError::ConfigRejected`] with the core's (redacted) message, or why the core could not be asked.
    fn check(&self, launch: &NodeLaunch) -> Result<(), NodeError>;

    /// Starts the instance; for [`Privilege::System`] on the desktops this (re)installs and starts its OS service,
    /// asking for administrator approval. Starting a running instance restarts it with the new config.
    ///
    /// # Errors
    ///
    /// Why it did not start.
    fn start(&self, launch: &NodeLaunch) -> Result<(), NodeError>;

    /// Stops the instance (an OS service is stopped, not removed).
    ///
    /// # Errors
    ///
    /// Why it could not be stopped.
    fn stop(&self, key: &InstanceKey) -> Result<(), NodeError>;

    /// Removes what the instance left on this device: its OS service, config file and logs. For a deleted profile.
    ///
    /// # Errors
    ///
    /// Why something could not be removed; what could be removed is gone.
    fn remove(&self, key: &InstanceKey) -> Result<(), NodeError>;

    /// Reads the instance's state now: `Running` only with a snapshot the core just returned.
    fn poll(&self, key: &InstanceKey) -> NetworkState;

    /// The instance's recent output, oldest first, redacted.
    fn logs(&self, key: &InstanceKey) -> Vec<LogLine>;

    /// Whether the instance is installed as an OS service (it then outlives the app).
    fn service_installed(&self, key: &InstanceKey) -> bool;

    /// Stops every child process this backend started (not OS services): the app is quitting.
    fn shutdown(&self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_keys_come_only_from_plain_ids() {
        assert_eq!(
            InstanceKey::for_profile("0f3a9c").unwrap().as_str(),
            "p-0f3a9c"
        );
        assert!(InstanceKey::for_profile("").is_none());
        assert!(InstanceKey::for_profile("../etc").is_none());
        assert!(InstanceKey::for_profile("家").is_none());
    }
}
