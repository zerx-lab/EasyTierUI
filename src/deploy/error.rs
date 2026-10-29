//! Why a deployment plan was refused. Every variant names the field so the form can put the message under it.

use std::fmt;

/// The plan field a [`DeployError`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Public domain name.
    Domain,
    /// ACME contact email.
    AcmeEmail,
    /// Container image tag.
    ImageTag,
    /// Container image digest.
    ImageDigest,
    /// Release version for the systemd runtime.
    Version,
    /// sha256 of the release zip.
    AssetSha256,
    /// Download URL of the release zip.
    DownloadUrl,
    /// REST API port.
    ApiPort,
    /// Config-server port.
    ConfigPort,
    /// Owner (easytier-web username) the server's own node enrolls under.
    NodeOwner,
    /// Docker volume name for the database.
    DataVolume,
    /// systemd state directory name.
    StateDir,
    /// Directory holding EasyTierUI's web build.
    WebDir,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Domain => "domain",
            Self::AcmeEmail => "ACME email",
            Self::ImageTag => "image tag",
            Self::ImageDigest => "image digest",
            Self::Version => "version",
            Self::AssetSha256 => "release sha256",
            Self::DownloadUrl => "download URL",
            Self::ApiPort => "API port",
            Self::ConfigPort => "config-server port",
            Self::NodeOwner => "node owner",
            Self::DataVolume => "data volume",
            Self::StateDir => "state directory",
            Self::WebDir => "web build directory",
        })
    }
}

/// What is wrong with the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Nothing was entered.
    Empty,
    /// Longer than allowed.
    TooLong,
    /// Shorter than allowed.
    TooShort,
    /// A character outside the allowed set.
    BadCharacter,
    /// Not the expected shape (for example a hostname without a dot, a digest that is not 64 hex digits).
    BadFormat,
    /// A name that is easy to guess or reserved.
    Reserved,
    /// Collides with another field or a port the reverse proxy needs.
    Conflict,
    /// Out of the accepted range (ports start at 1).
    OutOfRange,
    /// A scheme other than https, or credentials inside the URL.
    Insecure,
    /// Does not belong to the selected release (the file name differs from the expected asset).
    Mismatch,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "is empty",
            Self::TooLong => "is too long",
            Self::TooShort => "is too short",
            Self::BadCharacter => "contains a character that is not allowed",
            Self::BadFormat => "has the wrong format",
            Self::Reserved => "is reserved or easy to guess",
            Self::Conflict => "collides with another setting",
            Self::OutOfRange => "is out of range",
            Self::Insecure => "must be a plain https URL",
            Self::Mismatch => "does not match the selected release",
        })
    }
}

/// A deployment plan field was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{field} {reason}")]
pub struct DeployError {
    /// The field at fault.
    pub field: Field,
    /// What is wrong with it.
    pub reason: Reason,
}

impl DeployError {
    /// Builds the error for `field`.
    pub const fn new(field: Field, reason: Reason) -> Self {
        Self { field, reason }
    }
}
