//! `remote`: the console role on every platform — an easytier-web server's REST API.
//!
//! [`RemoteClient`] is the engine-thread client (async methods over NSH's `fetch`, so it runs unchanged in the web
//! build, where the REST API is [`ServerAddress::SameOrigin`]). Everything that can be tested without an engine is
//! separate: [`api`] builds requests and judges replies, [`wire`] maps the JSON to the app's types in
//! [`model`] and [`form`]. The server's field names, paths and RPC services appear only in this module.
//!
//! Login sends the MD5 hex of the password (what easytier-web's own frontend sends), so a remote server must be
//! HTTPS ([`ServerAddress::parse`]); the session cookie is a [`Secret`](crate::core::secret::Secret).

mod address;
mod api;
mod client;
mod error;
mod form;
mod ids;
mod model;
mod patch;
mod rpc_model;
mod timestamp;
mod wire;

pub use address::ServerAddress;
pub use api::{CredentialRequest, password_digest};
pub use client::RemoteClient;
pub use error::RemoteError;
pub use form::{AclState, Encryption, NetworkConfigForm, Networking};
pub use ids::new_instance_id;
pub use model::{
    ConfigSource, Credential, Machine, MachineNetworks, NetworkMeta, NetworkRuntime, NewCredential,
    ONLINE_WINDOW_MS, PeerLink, PeerLinks, PeersSummary, ServerNetwork, ServerSession,
    ServerSummary,
};
pub use patch::ConfigPatch;
pub use rpc_model::{
    AclAction, AclConnection, AclProtocol, AclRuleStats, AclStats, ConnTrackState, Connector,
    ConnectorStatus, ForeignNetwork, ForeignNetworkHost, ForeignNetworkSummary,
    GlobalForeignNetwork, LogLevel, Metric, NodeFacts, PeerFeatures, PeerMapLink, PeerMapNode,
    PortForwardEntry, PublicIpv6Info, PublicIpv6Lease, RouteEntry, StunFacts, TransportProtocol,
    VpnPortalInfo, Whitelist,
};
