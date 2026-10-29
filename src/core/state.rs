//! What a node reports: the state of one network instance and, while it runs, a snapshot of the node and its peers.
//!
//! These are the app's types; each backend (sidecar, embedded, remote) maps EasyTier's output into them in one
//! adapter. A snapshot carries the time it was read: the UI never shows a peer list without its age.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

use super::version::CoreVersion;

/// The state of one network instance, as its backend last reported it.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum NetworkState {
    /// Not running.
    #[default]
    Stopped,
    /// Started; the core has not answered yet. Never shown as connected.
    Starting,
    /// The core answered with this snapshot.
    Running(Box<NodeSnapshot>),
    /// Asked to stop; not confirmed yet.
    Stopping,
    /// It failed; `reason` says why and what the user can do.
    Failed {
        /// Why.
        reason: FailReason,
    },
}

impl NetworkState {
    /// The snapshot while running.
    pub fn snapshot(&self) -> Option<&NodeSnapshot> {
        match self {
            Self::Running(snapshot) => Some(snapshot),
            Self::Stopped | Self::Starting | Self::Stopping | Self::Failed { .. } => None,
        }
    }

    /// Whether the instance is up or on its way up (a start would duplicate it).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Starting | Self::Running(_))
    }
}

/// Why an instance failed, in terms the UI can act on. Messages are already redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailReason {
    /// Creating the TUN interface needs privileges the core does not have.
    NoPrivileges,
    /// No EasyTier program is installed or the configured one is gone.
    CoreMissing,
    /// The installed program is older than the app supports.
    CoreUnsupported(CoreVersion),
    /// The core rejected the config (`--check-config` or at start), with its message.
    ConfigRejected(String),
    /// The process exited on its own; the last lines it printed.
    Exited {
        /// The exit code, if the OS reported one (none for a signal).
        code: Option<i32>,
        /// Its last log lines, redacted.
        last_log: String,
    },
    /// The core runs but its management portal did not answer.
    Unreachable(String),
    /// The OS refused or the user cancelled the administrator prompt.
    ElevationDenied,
    /// Anything else, with a message for the log view.
    Other(String),
}

/// One coherent read of a running node.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeSnapshot {
    /// When it was read, in Unix milliseconds (`nsh_core::time::unix_ms`).
    pub read_at_ms: f64,
    /// This node.
    pub node: NodeInfo,
    /// Every other node in the network, this node excluded.
    pub peers: Vec<PeerSummary>,
}

/// This node, as the core reports it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeInfo {
    /// The core's numeric peer id.
    pub peer_id: u32,
    /// The hostname peers see.
    pub hostname: String,
    /// The virtual IPv4 with its prefix; `None` until assigned (DHCP) or when the network has none.
    pub virtual_ipv4: Option<Ipv4Net>,
    /// Listener URLs the core bound, as it prints them.
    pub listeners: Vec<String>,
    /// Public addresses STUN found.
    pub public_ips: Vec<String>,
    /// The UDP NAT type STUN found.
    pub nat_type: NatType,
    /// Subnets this node proxies.
    pub proxy_cidrs: Vec<Ipv4Net>,
    /// The core's version string (`2.6.4-8428a89d`).
    pub version: String,
}

/// A peer of this node.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PeerSummary {
    /// The core's numeric peer id: the row's stable key.
    pub peer_id: u32,
    /// Its hostname.
    pub hostname: String,
    /// Its virtual IPv4 with prefix, if it has one.
    pub virtual_ipv4: Option<Ipv4Net>,
    /// How traffic reaches it.
    pub route: RouteKind,
    /// Round-trip latency in milliseconds, when measured.
    pub latency_ms: Option<f64>,
    /// Packet loss as a fraction in `0.0..=1.0`, when measured.
    pub loss: Option<f64>,
    /// Bytes received from it, when counted.
    pub rx_bytes: Option<u64>,
    /// Bytes sent to it, when counted.
    pub tx_bytes: Option<u64>,
    /// The tunnel protocol of a direct link (`tcp`, `udp`, `quic`, …).
    pub tunnel: Option<String>,
    /// Its NAT type.
    pub nat_type: NatType,
    /// Its EasyTier version, if reported.
    pub version: Option<String>,
}

/// How a peer is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RouteKind {
    /// A direct (P2P) link.
    Direct,
    /// Through other nodes: `hops` is the path length the core reports (at least 2).
    Relayed {
        /// The number of hops.
        hops: u32,
    },
    /// The core did not say.
    #[default]
    Unknown,
}

/// A NAT type as STUN classifies it (EasyTier's `NatType` numbering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NatType {
    /// Not determined.
    #[default]
    Unknown,
    /// Open internet.
    OpenInternet,
    /// No PAT.
    NoPat,
    /// Full cone.
    FullCone,
    /// Restricted cone.
    Restricted,
    /// Port restricted cone.
    PortRestricted,
    /// Symmetric.
    Symmetric,
    /// Symmetric UDP firewall.
    SymUdpFirewall,
    /// Symmetric with easily predicted increasing ports.
    SymmetricEasyInc,
    /// Symmetric with easily predicted decreasing ports.
    SymmetricEasyDec,
}

impl NatType {
    /// From EasyTier's numeric code (`stun_info.udp_nat_type`).
    pub fn from_code(code: i64) -> Self {
        match code {
            1 => Self::OpenInternet,
            2 => Self::NoPat,
            3 => Self::FullCone,
            4 => Self::Restricted,
            5 => Self::PortRestricted,
            6 => Self::Symmetric,
            7 => Self::SymUdpFirewall,
            8 => Self::SymmetricEasyInc,
            9 => Self::SymmetricEasyDec,
            _ => Self::Unknown,
        }
    }

    /// From the name the CLI prints (`PortRestricted`).
    pub fn from_name(name: &str) -> Self {
        match name {
            "OpenInternet" => Self::OpenInternet,
            "NoPat" | "NoPAT" => Self::NoPat,
            "FullCone" => Self::FullCone,
            "Restricted" => Self::Restricted,
            "PortRestricted" => Self::PortRestricted,
            "Symmetric" => Self::Symmetric,
            "SymUdpFirewall" => Self::SymUdpFirewall,
            "SymmetricEasyInc" => Self::SymmetricEasyInc,
            "SymmetricEasyDec" => Self::SymmetricEasyDec,
            _ => Self::Unknown,
        }
    }
}

/// Which stream a log line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    /// The program's standard output.
    Stdout,
    /// The program's standard error.
    Stderr,
    /// A line the app wrote about the process (started, exited, restarted).
    App,
}

/// One line of a program's output, already redacted.
#[derive(Debug, Clone, PartialEq)]
pub struct LogLine {
    /// When it was captured, in Unix milliseconds.
    pub at_ms: f64,
    /// Where it came from.
    pub stream: LogStream,
    /// The text, secrets replaced by `***`.
    pub text: String,
}

/// An IPv4 address with a prefix, from `a.b.c.d/len` or a bare address (`/32`); `None` for `""`, `-` and garbage.
pub fn parse_ipv4_net(text: &str) -> Option<Ipv4Net> {
    let text = text.trim();
    if let Ok(net) = text.parse::<Ipv4Net>() {
        return Some(net);
    }
    text.parse::<Ipv4Addr>()
        .ok()
        .and_then(|addr| Ipv4Net::new(addr, 32).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_parse_with_or_without_a_prefix() {
        assert_eq!(parse_ipv4_net("10.144.144.1/24").unwrap().prefix_len(), 24);
        assert_eq!(parse_ipv4_net("10.144.144.1").unwrap().prefix_len(), 32);
        assert_eq!(parse_ipv4_net("-"), None);
        assert_eq!(parse_ipv4_net(""), None);
    }

    #[test]
    fn nat_codes_and_names_agree() {
        assert_eq!(NatType::from_code(5), NatType::from_name("PortRestricted"));
        assert_eq!(NatType::from_code(42), NatType::Unknown);
    }
}
