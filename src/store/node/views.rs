//! What the node store keeps per network at run time and what pages read of it.

use super::problem::SaveFailure;
use crate::{
    core::{
        secret::Secret,
        state::{FailReason, LogLine, NetworkState},
        version::CoreVersion,
    },
    profile::ProfileId,
};

/// An action in flight on a network; its button is busy until it ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Starting (or restarting) it.
    Starting,
    /// Stopping it.
    Stopping,
    /// Deleting it: stopping, removing its service and files, forgetting its secret.
    Removing,
}

/// Why a network is not running, beyond what the core reports.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// The core, or the way to it, failed with this reason.
    Core(FailReason),
    /// The network secret is not in the keychain (it was lost with a restart where secrets are not kept, or removed).
    SecretMissing,
}

/// One network's state as a card shows it.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Not running.
    Stopped,
    /// Started; the core has not answered yet (never shown as connected).
    Starting,
    /// The core answered.
    Running,
    /// Asked to stop; not confirmed yet.
    Stopping,
    /// Being deleted.
    Removing,
    /// Failed, and why.
    Failed(Failure),
}

impl Status {
    /// Up or on its way up: a start would duplicate it.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }
}

/// What the store keeps of one network between polls.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Runtime {
    /// The last state the backend reported.
    pub(super) state: NetworkState,
    /// The action in flight.
    pub(super) op: Option<Op>,
    /// Why the last start failed (shown while the backend says stopped).
    pub(super) failure: Option<Failure>,
    /// Why the last stop failed.
    pub(super) stop_error: Option<FailReason>,
    /// Bumped by every action; an answer that carries an older one is dropped.
    pub(super) revision: u64,
}

impl Runtime {
    /// The status a card shows.
    pub(super) fn status(&self) -> Status {
        match self.op {
            Some(Op::Starting) => Status::Starting,
            Some(Op::Stopping) => Status::Stopping,
            Some(Op::Removing) => Status::Removing,
            None => match &self.state {
                NetworkState::Stopped => {
                    self.failure.clone().map_or(Status::Stopped, Status::Failed)
                }
                NetworkState::Starting => Status::Starting,
                NetworkState::Running(_) => Status::Running,
                NetworkState::Stopping => Status::Stopping,
                NetworkState::Failed { reason } => Status::Failed(Failure::Core(reason.clone())),
            },
        }
    }
}

/// Whether the EasyTier core is ready on this device.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreStatus {
    /// Not asked yet.
    Checking,
    /// The active release.
    Ready(CoreVersion),
    /// None installed (or the active one is gone).
    Missing,
    /// Installed but older than the oldest supported release.
    Unsupported(CoreVersion),
    /// This platform needs no installed program (the core is part of the app).
    NotNeeded,
}

impl CoreStatus {
    /// Networks can start.
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Ready(_) | Self::NotNeeded | Self::Checking)
    }
}

/// A message above the list of networks.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// The stored list is not valid; it is left as it is and nothing can be saved over it.
    Damaged,
    /// A newer version of the app wrote the stored list.
    NewerSchema,
    /// Some stored entries could not be read and were kept as they are.
    Skipped(usize),
    /// An action of the list failed (deleting a network, for one).
    Failed(SaveFailure),
}

/// The log tab's content.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LogView {
    /// The network the lines belong to.
    pub id: Option<ProfileId>,
    /// The core's recent output, oldest first, already redacted.
    pub lines: Vec<LogLine>,
    /// Being read.
    pub loading: bool,
    /// When they were read (Unix milliseconds).
    pub read_at_ms: Option<f64>,
}

/// Why an export could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFailure {
    /// The profile does not render.
    Invalid,
    /// The keychain refused the secret.
    Keychain,
    /// The link is too long for a QR code (the text and link still work).
    QrTooLong,
}

/// The export tab's content.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExportView {
    /// The network exported.
    pub id: Option<ProfileId>,
    /// Whether the network secret is part of the export (off by default).
    pub include_secret: bool,
    /// The TOML config.
    pub toml: Option<Secret>,
    /// The share link.
    pub link: Option<Secret>,
    /// The link as a QR code: modules per side and the modules, row by row.
    pub qr: Option<(usize, Vec<bool>)>,
    /// What went wrong.
    pub failure: Option<ExportFailure>,
    /// The secret was asked for and the keychain has none.
    pub secret_missing: bool,
    /// Being made.
    pub loading: bool,
}
