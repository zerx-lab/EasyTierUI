//! Why a profile is not valid or cannot be turned into a launch. Every variant names the field; none repeats a value
//! that could be a secret.

use crate::core::net::NetError;

/// A problem with a profile, found while validating, rendering or launching it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    /// Nothing was entered.
    #[error("{field}: this is empty")]
    Empty {
        /// The field.
        field: String,
    },
    /// Longer than the limit; names are never silently cut (the editor truncates what the person types).
    #[error("{field}: longer than {max} characters")]
    TooLong {
        /// The field.
        field: String,
        /// The limit, in characters.
        max: usize,
    },
    /// Contains a control character (newline, tab, escape, ...).
    #[error("{field}: contains control characters")]
    ControlChars {
        /// The field.
        field: String,
    },
    /// A list with more entries than a profile may carry.
    #[error("{field}: more than {max} entries")]
    TooMany {
        /// The field.
        field: String,
        /// The limit.
        max: usize,
    },
    /// An entry that appears twice.
    #[error("{field}: listed more than once")]
    Duplicate {
        /// The field.
        field: String,
    },
    /// A number outside its range.
    #[error("{field}: must be from {min} to {max}")]
    OutOfRange {
        /// The field.
        field: String,
        /// Smallest value.
        min: u32,
        /// Largest value.
        max: u32,
    },
    /// Anything else wrong with a value.
    #[error("{field}: {reason}")]
    Invalid {
        /// The field.
        field: String,
        /// What is wrong.
        reason: &'static str,
    },
    /// An address, URL, CIDR or port; the inner error names the field.
    #[error(transparent)]
    Net(#[from] NetError),
    /// A profile id that is not 32 lowercase hexadecimal characters.
    #[error("id: not a valid profile id")]
    BadId,
    /// The machine id is not a UUID.
    #[error("machine id: not a valid UUID")]
    BadMachineId,
    /// The control-plane address is not `tcp|udp|ws|wss://host[:port]/<account>` (never shown: it is a secret).
    #[error("config server: not a valid control-plane address")]
    BadConfigServer,
    /// A managed profile has no local config to render: its control plane pushes it.
    #[error("this profile is managed by a control plane and has no local config")]
    Managed,
    /// The TOML serializer refused the profile; a bug, not something the person can fix.
    #[error("the config could not be written")]
    Render,
    /// The platform has no secure random source (the web build), so no id can be generated.
    #[error("this platform has no secure random source")]
    NoRandomSource,
}

impl ProfileError {
    /// The field the problem is in, if it has one.
    pub fn field(&self) -> Option<&str> {
        match self {
            Self::Empty { field }
            | Self::TooLong { field, .. }
            | Self::ControlChars { field }
            | Self::TooMany { field, .. }
            | Self::Duplicate { field }
            | Self::OutOfRange { field, .. }
            | Self::Invalid { field, .. } => Some(field),
            Self::Net(error) => Some(error.field()),
            Self::BadId => Some("id"),
            Self::BadMachineId => Some("machine id"),
            Self::BadConfigServer => Some("config server"),
            Self::Managed | Self::Render | Self::NoRandomSource => None,
        }
    }
}
