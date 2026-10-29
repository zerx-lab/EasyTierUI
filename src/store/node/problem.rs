//! What a form can say is wrong, as data the pages turn into words in the user's language: which field, which entry
//! of a list, and what kind of problem. Converted from the profile module's errors (English, for logs) in one place.

use crate::{
    core::{acl::AclError, net::NetError},
    profile::{ImportError, ProfileError, ShareError},
};

/// A field of a node form a problem can belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// The name shown in the app.
    Name,
    /// The hostname peers see.
    Hostname,
    /// The EasyTier network name.
    NetworkName,
    /// The network secret.
    Secret,
    /// The static virtual address.
    Ip,
    /// Peers and shared nodes.
    Peers,
    /// Listeners.
    Listeners,
    /// The MTU.
    Mtu,
    /// The network interface name.
    DevName,
    /// Proxied subnets.
    SubnetProxy,
    /// Exit nodes this node uses.
    ExitNodes,
    /// The SOCKS5 proxy.
    Socks5,
    /// Port forwards.
    PortForwards,
    /// The instance's receive rate limit.
    RecvLimit,
    /// Mapped listeners.
    MappedListeners,
    /// Manual routes.
    Routes,
    /// The VPN portal's client network.
    VpnPortalCidr,
    /// The VPN portal's WireGuard port.
    VpnPortalPort,
    /// The access-control section (the editor of it shows the details).
    Acl,
    /// The relay whitelist.
    RelayWhitelist,
    /// The control-plane address of a managed network.
    ConfigServer,
    /// Pasted text or a picked image.
    Import,
    /// Anything else.
    Other,
}

impl Field {
    /// The field a profile error's name (`peers[2]`, `port_forwards[0].bind`, `network_name`) belongs to.
    pub fn of(name: &str) -> Self {
        match name {
            "vpn_portal.client_cidr" | "vpn_portal_config.client_cidr" => {
                return Self::VpnPortalCidr;
            }
            "vpn_portal.listen_port" | "vpn_portal_config.wireguard_listen" => {
                return Self::VpnPortalPort;
            }
            _ => {}
        }
        let base = name.split(['[', '.']).next().unwrap_or(name);
        match base {
            "name" => Self::Name,
            "hostname" => Self::Hostname,
            "network_name" => Self::NetworkName,
            "ip" | "ipv4" => Self::Ip,
            "peers" | "peer" => Self::Peers,
            "listeners" => Self::Listeners,
            "mtu" => Self::Mtu,
            "dev_name" => Self::DevName,
            "subnet_proxy" | "proxy_network" => Self::SubnetProxy,
            "exit_nodes" => Self::ExitNodes,
            "socks5" | "socks5_proxy" => Self::Socks5,
            "port_forwards" | "port_forward" => Self::PortForwards,
            "instance_recv_bps_limit" => Self::RecvLimit,
            "mapped_listeners" => Self::MappedListeners,
            "routes" => Self::Routes,
            "acl" => Self::Acl,
            "relay_whitelist" | "relay_network_whitelist" => Self::RelayWhitelist,
            "config server" => Self::ConfigServer,
            _ => Self::Other,
        }
    }
}

/// What is wrong with a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    /// Nothing was entered.
    Required,
    /// Longer than `max` characters.
    TooLong {
        /// The limit.
        max: usize,
    },
    /// Contains a control character.
    ControlChars,
    /// More than `max` entries.
    TooMany {
        /// The limit.
        max: usize,
    },
    /// Listed twice.
    Duplicate,
    /// A number outside `min..=max`.
    OutOfRange {
        /// Smallest value.
        min: u32,
        /// Largest value.
        max: u32,
    },
    /// An interface name with characters other than letters, digits, `-`, `_` and `.`.
    NameChars,
    /// A whitelist pattern with a space.
    NoSpaces,
    /// Not an address (URL or IP).
    NotAnAddress,
    /// A URL scheme EasyTier does not know.
    UnknownScheme,
    /// A scheme that finds peers but cannot listen.
    ConnectorOnly,
    /// A URL without a host.
    MissingHost,
    /// A listener host that is not an IP address.
    HostNotIp,
    /// A port outside 1 to 65535.
    BadPort,
    /// A `ws` / `wss` listener without a usable explicit port.
    WellKnownPort,
    /// An address with credentials, a query, a fragment or a path.
    ExtraParts,
    /// Not `a.b.c.d/len`.
    NotACidr,
    /// A prefix length outside `min..=max`.
    BadPrefix {
        /// Shortest prefix.
        min: u8,
        /// Longest prefix.
        max: u8,
    },
    /// A subnet written with a host address instead of its network address.
    HostBits,
    /// An address that cannot be used here.
    Reserved,
    /// A port forward not written as `tcp|udp bind-address destination-address`.
    ForwardShape,
    /// The control-plane address is not `udp://host:port/account`.
    ConfigServerShape,
    /// A mapped subnet that is not the same size as the subnet it stands for.
    MappedSize,
    /// A number that must be more than 0.
    MustBePositive,
    /// The access-control section is not valid; says where.
    Acl(AclError),
    /// Something else is wrong; the log has details.
    Other,
}

/// One problem: where, and what.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldProblem {
    /// The field.
    pub field: Field,
    /// The entry of a list field (1 for the first), when the problem is in one.
    pub entry: Option<usize>,
    /// What is wrong.
    pub kind: ProblemKind,
}

impl FieldProblem {
    /// A problem of a whole field.
    pub fn of(field: Field, kind: ProblemKind) -> Self {
        Self {
            field,
            entry: None,
            kind,
        }
    }

    /// The problem in entry `entry` (1-based) of a list field.
    pub fn in_entry(field: Field, entry: usize, kind: ProblemKind) -> Self {
        Self {
            field,
            entry: Some(entry),
            kind,
        }
    }
}

/// The 1-based entry a name like `peers[2]` points at (`None` for a name without an index).
fn entry_of(name: &str) -> Option<usize> {
    let (_, after) = name.split_once('[')?;
    let (index, _) = after.split_once(']')?;
    index.parse::<usize>().ok().map(|n| n.saturating_add(1))
}

impl From<&NetError> for FieldProblem {
    fn from(error: &NetError) -> Self {
        let name = error.field();
        let kind = match error {
            NetError::Empty { .. } => ProblemKind::Required,
            NetError::TooLong { max, .. } => ProblemKind::TooLong { max: *max },
            NetError::Malformed { .. } | NetError::BadAddress { .. } => ProblemKind::NotAnAddress,
            NetError::UnknownScheme { .. } => ProblemKind::UnknownScheme,
            NetError::ConnectorOnly { .. } => ProblemKind::ConnectorOnly,
            NetError::MissingHost { .. } => ProblemKind::MissingHost,
            NetError::HostNotIp { .. } => ProblemKind::HostNotIp,
            NetError::BadPort { .. } => ProblemKind::BadPort,
            NetError::WellKnownPort { .. } => ProblemKind::WellKnownPort,
            NetError::Unexpected { .. } => ProblemKind::ExtraParts,
            NetError::BadCidr { .. } => ProblemKind::NotACidr,
            NetError::BadPrefix { min, max, .. } => ProblemKind::BadPrefix {
                min: *min,
                max: *max,
            },
            NetError::HostBits { .. } => ProblemKind::HostBits,
            NetError::Reserved { .. } => ProblemKind::Reserved,
        };
        Self {
            field: Field::of(name),
            entry: entry_of(name),
            kind,
        }
    }
}

impl From<&ProfileError> for FieldProblem {
    fn from(error: &ProfileError) -> Self {
        let name = error.field().unwrap_or_default();
        let kind = match error {
            ProfileError::Empty { .. } => ProblemKind::Required,
            ProfileError::TooLong { max, .. } => ProblemKind::TooLong { max: *max },
            ProfileError::ControlChars { .. } => ProblemKind::ControlChars,
            ProfileError::TooMany { max, .. } => ProblemKind::TooMany { max: *max },
            ProfileError::Duplicate { .. } => ProblemKind::Duplicate,
            ProfileError::OutOfRange { min, max, .. } => ProblemKind::OutOfRange {
                min: *min,
                max: *max,
            },
            ProfileError::Invalid { reason, .. } => {
                if reason.contains("spaces") {
                    ProblemKind::NoSpaces
                } else if reason.contains("letters") {
                    ProblemKind::NameChars
                } else if reason.contains("same size") {
                    ProblemKind::MappedSize
                } else if reason.contains("more than 0") || reason.contains("positive") {
                    ProblemKind::MustBePositive
                } else {
                    ProblemKind::Other
                }
            }
            ProfileError::Net(net) => return Self::from(net),
            ProfileError::Acl(acl) => ProblemKind::Acl(*acl),
            ProfileError::BadConfigServer => ProblemKind::ConfigServerShape,
            ProfileError::BadId
            | ProfileError::BadMachineId
            | ProfileError::Managed
            | ProfileError::Render
            | ProfileError::NoRandomSource => ProblemKind::Other,
        };
        Self {
            field: Field::of(name),
            entry: entry_of(name),
            kind,
        }
    }
}

/// Why importing pasted text, a link or a picked image failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFailure {
    /// What went wrong.
    pub reason: ImportReason,
    /// The config key it concerns, when there is one (shown as machine text beside the reason).
    pub key: Option<String>,
    /// The line of a syntax error.
    pub line: Option<usize>,
}

/// The kinds of import failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportReason {
    /// Nothing to import.
    Empty,
    /// The config is larger than any real one.
    TooLarge,
    /// Not valid TOML.
    Syntax,
    /// A key holds the wrong kind of value.
    WrongType,
    /// A required key is missing.
    Missing,
    /// A value this app does not support (an unknown algorithm or protocol).
    Unsupported,
    /// A value that is not valid, with the kind of problem.
    Invalid(ProblemKind),
    /// Text that is neither a config nor a share link.
    NotALink,
    /// A share link of a newer format.
    LinkVersion,
    /// A share link without a config.
    LinkNoConfig,
    /// A share link whose config is damaged.
    LinkDamaged,
    /// The picked file is not a PNG or JPEG.
    ImageType,
    /// The picked image is too large.
    ImageTooLarge,
    /// The picked image could not be decoded.
    ImageDecode,
    /// No QR code in the picked image.
    NoQr,
    /// A QR code that could not be read.
    QrUnreadable,
    /// Reading the picked image failed or was cancelled.
    PickFailed,
    /// This platform cannot generate the id the profile needs.
    NoRandomSource,
}

impl ImportFailure {
    /// A failure without a key or line.
    pub fn new(reason: ImportReason) -> Self {
        Self {
            reason,
            key: None,
            line: None,
        }
    }
}

impl From<&ImportError> for ImportFailure {
    fn from(error: &ImportError) -> Self {
        let keyed = |reason: ImportReason, key: &str| Self {
            reason,
            key: Some(key.to_owned()),
            line: None,
        };
        match error {
            ImportError::TooLarge { .. } => Self::new(ImportReason::TooLarge),
            ImportError::Syntax { line, .. } => Self {
                reason: ImportReason::Syntax,
                key: None,
                line: *line,
            },
            ImportError::WrongType { field, .. } => keyed(ImportReason::WrongType, field),
            ImportError::Missing { field } => keyed(ImportReason::Missing, field),
            ImportError::UnknownValue { field } => keyed(ImportReason::Unsupported, field),
            ImportError::Profile(ProfileError::NoRandomSource) => {
                Self::new(ImportReason::NoRandomSource)
            }
            ImportError::Profile(profile) => {
                let problem = FieldProblem::from(profile);
                Self {
                    reason: ImportReason::Invalid(problem.kind),
                    key: profile.field().map(str::to_owned),
                    line: None,
                }
            }
        }
    }
}

impl From<&ShareError> for ImportFailure {
    fn from(error: &ShareError) -> Self {
        let reason = match error {
            ShareError::Profile(ProfileError::NoRandomSource) => ImportReason::NoRandomSource,
            ShareError::Profile(profile) => {
                let problem = FieldProblem::from(profile);
                return Self {
                    reason: ImportReason::Invalid(problem.kind),
                    key: profile.field().map(str::to_owned),
                    line: None,
                };
            }
            ShareError::NotAShareLink => ImportReason::NotALink,
            ShareError::UnsupportedVersion => ImportReason::LinkVersion,
            ShareError::MissingPayload => ImportReason::LinkNoConfig,
            ShareError::BadPayload => ImportReason::LinkDamaged,
            ShareError::TooLarge { .. } => ImportReason::TooLarge,
            ShareError::QrTooLong => ImportReason::TooLarge,
            ShareError::UnsupportedImage => ImportReason::ImageType,
            ShareError::ImageTooLarge { .. } => ImportReason::ImageTooLarge,
            ShareError::ImageDecode => ImportReason::ImageDecode,
            ShareError::NoQrCode => ImportReason::NoQr,
            ShareError::QrUnreadable => ImportReason::QrUnreadable,
            ShareError::Import(import) => return Self::from(import),
        };
        Self::new(reason)
    }
}

/// Why saving, starting or removing something failed, in terms a page can word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveFailure {
    /// The keychain refused the secret.
    Keychain,
    /// The app's storage is full.
    StorageFull,
    /// The stored profile list is damaged; it is left untouched.
    Damaged,
    /// A newer version of the app wrote the stored profiles.
    NewerSchema,
    /// The profile is not valid (a bug if the form checked it first).
    Invalid(FieldProblem),
    /// This platform cannot generate an id.
    NoRandomSource,
    /// Reading or writing off the engine thread failed.
    Background,
    /// The OS service or files of the network could not be removed.
    Remove(String),
}
