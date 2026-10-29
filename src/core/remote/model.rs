//! What the console pages show: the app's types for an easytier-web server, its machines and their networks.
//!
//! Adapters in [`wire`](super::wire) fill them from the REST JSON; nothing here names an EasyTier field.

use ipnet::Ipv4Net;
use uuid::Uuid;

use crate::core::{secret::Secret, state::NodeSnapshot};

/// A machine whose last heartbeat is older than this is shown as stale rather than online. easytier-web drops a
/// session after 30 s without traffic, so anything it still lists is recent; the margin absorbs clock skew.
pub const ONLINE_WINDOW_MS: f64 = 60_000.0;

/// What `GET /summary` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServerSummary {
    /// Machines connected to the server for this account.
    pub device_count: u32,
}

/// A machine (an `easytier-core` enrolled in the server) in the machine list.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    /// The id the REST paths use (`--machine-id` of the core).
    pub id: Uuid,
    /// Its hostname.
    pub hostname: String,
    /// Its operating system, as a label (`macOS 27.2`); empty when it did not say.
    pub os: String,
    /// The core's version string (`2.6.4-8428a89d`).
    pub version: String,
    /// When its last heartbeat was sent, in Unix milliseconds (the unit of `nsh_core::time::unix_ms`).
    pub report_time_ms: Option<f64>,
    /// Networks it reports as running (heartbeat); disabled ones come from `machine_networks`.
    pub running_networks: Vec<Uuid>,
    /// Where the server places its connection, as a label (`本地网络`, `Germany, Berlin`).
    pub location: Option<String>,
    /// How the machine reaches the server (`udp://203.0.113.9:53277`).
    pub client_url: String,
}

impl Machine {
    /// Whether its heartbeat is recent at `now_ms` (`nsh_core::time::unix_ms()`); a machine without a report time
    /// is not shown as online.
    pub fn is_online(&self, now_ms: f64) -> bool {
        self.report_time_ms
            .is_some_and(|reported| now_ms - reported <= ONLINE_WINDOW_MS)
    }
}

/// One entry of `GET /sessions`: a machine's connection to the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSession {
    /// The machine.
    pub machine_id: Uuid,
    /// How it reaches the server.
    pub client_url: String,
    /// The account that owns it (a database id). The list covers every account's machines.
    pub user_id: i64,
}

/// The instances on one machine: `GET /machines/{mid}/networks`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineNetworks {
    /// Instances that are running.
    pub running: Vec<Uuid>,
    /// Instances that are saved and switched off.
    pub disabled: Vec<Uuid>,
}

/// Who put a network's config on the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigSource {
    /// Not stated.
    #[default]
    Unspecified,
    /// A user, through the console.
    User,
    /// The server's webhook; the console should not edit it.
    Webhook,
    /// A source a newer server added.
    Other,
}

impl ConfigSource {
    /// From the numeric code (1 user, 2 webhook).
    pub fn from_code(code: i64) -> Self {
        match code {
            0 => Self::Unspecified,
            1 => Self::User,
            2 => Self::Webhook,
            _ => Self::Other,
        }
    }
}

/// What the server knows about one network instance without asking the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkMeta {
    /// The instance id.
    pub instance_id: Uuid,
    /// The network's name.
    pub network_name: String,
    /// The instance's name.
    pub instance_name: String,
    /// Who set its config.
    pub source: ConfigSource,
    /// The server's permission bits for editing the config (0 = editable).
    pub config_permission: u32,
}

/// One network instance as the machine reports it right now.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkRuntime {
    /// Whether the instance is up.
    pub running: bool,
    /// Why it is not, when it reported an error.
    pub error: Option<String>,
    /// The node and its peers, in the app's types. `proxy_cidrs` of the node stay empty: the running info does not
    /// carry them (they are in the config).
    pub snapshot: NodeSnapshot,
}

/// A count of a network's peers by how they are reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeersSummary {
    /// All peers.
    pub total: usize,
    /// Direct (P2P) links.
    pub direct: usize,
    /// Through other nodes.
    pub relayed: usize,
}

/// A network on a machine, as the server's network list shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerNetwork {
    /// The instance id.
    pub instance_id: Uuid,
    /// The network's name.
    pub name: String,
    /// The instance's name.
    pub instance_name: String,
    /// Whether it runs.
    pub running: bool,
    /// Whether it is saved and switched off.
    pub disabled: bool,
    /// The node's virtual IPv4 while it runs.
    pub virtual_ipv4: Option<Ipv4Net>,
    /// Its peers by route kind; `None` when the machine did not report (not running, or the read failed).
    pub peers: Option<PeersSummary>,
    /// The error the instance reported, if any.
    pub error: Option<String>,
    /// Who set its config.
    pub source: ConfigSource,
    /// The server's permission bits for editing it (0 = editable).
    pub config_permission: u32,
}

impl ServerNetwork {
    /// Whether the console may edit its config: a user's, with no permission restriction.
    pub fn is_editable(&self) -> bool {
        self.config_permission == 0 && !matches!(self.source, ConfigSource::Webhook)
    }
}

/// One live link to a peer, from the core's peer list.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerLink {
    /// The tunnel protocol (`tcp`, `udp`, `quic`, ...).
    pub tunnel: String,
    /// The peer's address on this link.
    pub remote: Option<String>,
    /// Round-trip time in milliseconds.
    pub latency_ms: Option<f64>,
    /// Packet loss as a fraction in `0.0..=1.0`.
    pub loss: f64,
    /// Bytes received on it.
    pub rx_bytes: u64,
    /// Bytes sent on it.
    pub tx_bytes: u64,
    /// Whether this node dialed it.
    pub is_client: bool,
    /// Whether it is closed.
    pub closed: bool,
}

/// A peer's direct links (`PeerManageRpcService/ListPeer`).
#[derive(Debug, Clone, PartialEq)]
pub struct PeerLinks {
    /// The peer's numeric id.
    pub peer_id: u32,
    /// Its links; empty for none.
    pub links: Vec<PeerLink>,
}

/// An active credential of a node (`CredentialManageRpcService/ListCredentials`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    /// Its id.
    pub id: String,
    /// ACL groups of a node using it.
    pub groups: Vec<String>,
    /// Whether a node using it may relay.
    pub allow_relay: bool,
    /// When it expires, in Unix milliseconds; `None` when the core gave none.
    pub expires_at_ms: Option<i64>,
    /// Subnets a node using it may proxy.
    pub allowed_proxy_cidrs: Vec<String>,
    /// Whether several nodes may use it at once; `None` for a core that does not say.
    pub reusable: Option<bool>,
}

/// A credential just created. The secret is shown once and is not recoverable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCredential {
    /// Its id.
    pub id: String,
    /// The private key a node joins with.
    pub secret: Secret,
}
