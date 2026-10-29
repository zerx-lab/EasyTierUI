//! Why managing EasyTier releases failed, in variants the UI can act on (retry, pick another release, remove,
//! choose a directory). Messages never carry secrets: nothing here handles any.

use std::path::PathBuf;

use crate::core::version::CoreVersion;

/// Why a release could not be listed, installed, adopted, switched or removed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    /// The app's data directory is unusable.
    #[error("the app's data directory: {0}")]
    DataDir(String),
    /// A file or directory operation failed.
    #[error("file error: {0}")]
    Io(String),
    /// The background task that does the file work ended without a result.
    #[error("the background task ended without a result: {0}")]
    Interrupted(String),
    /// The request did not reach GitHub (offline, DNS, TLS, blocked by the app's network capability).
    #[error("could not reach GitHub: {0}")]
    Network(String),
    /// GitHub refused because of its anonymous rate limit; retry later.
    #[error("GitHub's rate limit was reached; try again later")]
    RateLimited,
    /// GitHub answered with an unexpected status.
    #[error("GitHub answered with status {0}")]
    Http(u16),
    /// The release list was not the JSON array that was expected.
    #[error("the release list could not be read: {0}")]
    BadResponse(String),
    /// The release has no zip for this device.
    #[error("this release has no download for this device")]
    NoAsset,
    /// The release API gave no sha256 digest for the zip, so it cannot be verified and is not installed.
    #[error("the release publishes no sha256 digest for its download; it cannot be verified")]
    MissingDigest,
    /// The zip is bigger than the app accepts.
    #[error("the download is larger than allowed ({0} bytes)")]
    TooLarge(u64),
    /// The download's length is not the length the release lists.
    #[error("the download is {actual} bytes but the release lists {expected}")]
    SizeMismatch {
        /// Bytes the release lists.
        expected: u64,
        /// Bytes received.
        actual: u64,
    },
    /// The download's sha256 is not the release's digest: nothing was written.
    #[error("the download does not match the release's sha256 digest")]
    DigestMismatch,
    /// The release is older than the oldest supported one, or a chosen directory holds such a core.
    #[error("EasyTier {0} is older than the oldest supported release")]
    Unsupported(CoreVersion),
    /// The zip could not be read.
    #[error("the archive is damaged: {0}")]
    BadArchive(String),
    /// A zip entry would leave its directory, is a link, a device or anything but a regular file or directory.
    #[error("the archive holds an entry that is not allowed: {0}")]
    BadEntry(String),
    /// A program of the release is missing from the directory.
    #[error("{0} is missing")]
    MissingProgram(String),
    /// A program did not report a version (it could not run, timed out or printed something else).
    #[error("could not read the version of {program}: {reason}")]
    VersionUnreadable {
        /// The program file name.
        program: String,
        /// Why.
        reason: String,
    },
    /// The unpacked core is not the release that was downloaded.
    #[error("the downloaded core reports {found}, not {expected}")]
    VersionMismatch {
        /// The release's number.
        expected: String,
        /// What the core reported.
        found: String,
    },
    /// That release is already installed and works.
    #[error("EasyTier {0} is already installed")]
    AlreadyInstalled(String),
    /// That release is not installed.
    #[error("EasyTier {0} is not installed")]
    NotInstalled(String),
    /// The active release cannot be removed; switch first.
    #[error("the active release cannot be removed")]
    ActiveInUse,
    /// There is no previous release to go back to.
    #[error("there is no previous release to go back to")]
    NoPrevious,
    /// A chosen directory must be an absolute path without `..`.
    #[error("{0:?} is not an absolute directory path")]
    NotAbsolute(PathBuf),
    /// No `easytier-core` is on the `PATH`.
    #[error("easytier-core was not found on the PATH")]
    NotOnPath,
}

impl InstallError {
    /// An I/O error with what was being done, e.g. `Io("creating /x: permission denied")`.
    pub(crate) fn io(doing: &str, error: &std::io::Error) -> Self {
        Self::Io(format!("{doing}: {error}"))
    }
}
