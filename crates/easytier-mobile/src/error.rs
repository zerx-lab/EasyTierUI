//! The bridge's one error type. Variants are what a host app (or the Rust app's `embedded` backend) can act on;
//! messages are redacted before they are put in a variant (see `redact`), so they are safe to show and log.

/// Why a bridge call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum MobileError {
    /// The instance name is empty, too long, or has characters outside `[A-Za-z0-9_-]`.
    #[error("invalid instance name: {reason}")]
    InvalidName {
        /// What is wrong with it.
        reason: String,
    },
    /// The config is not valid TOML or EasyTier rejected it. The config text is never part of the message.
    #[error("the config was rejected: {message}")]
    ConfigRejected {
        /// The parser's complaint, with the position when known and without any config content.
        message: String,
    },
    /// An instance with this name already runs; stop it first.
    #[error("an instance named {name} is already running")]
    AlreadyRunning {
        /// The instance name.
        name: String,
    },
    /// No instance with this name runs.
    #[error("no instance named {name}")]
    NotFound {
        /// The instance name.
        name: String,
    },
    /// EasyTier refused to start the instance.
    #[error("the instance could not start: {message}")]
    StartFailed {
        /// EasyTier's message, redacted.
        message: String,
    },
    /// The instance is not ready for, or refused, a TUN file descriptor.
    #[error("the TUN descriptor was not accepted: {message}")]
    TunFd {
        /// Why.
        message: String,
    },
    /// The library failed in a way the caller cannot fix (a panic inside EasyTier, the runtime could not start).
    #[error("internal error: {message}")]
    Internal {
        /// What failed.
        message: String,
    },
}
