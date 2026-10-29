//! Network profiles: the model, validation, TOML rendering, import / export, share links and secret storage.
//!
//! Pure Rust that compiles on every target (the web build included); only random ids need an OS source and report
//! [`ProfileError::NoRandomSource`] without one. Nothing here starts a process or touches the filesystem: the node
//! backends take the [`NodeLaunch`](crate::core::node::NodeLaunch) that [`NetworkProfile::launch`] builds.

pub mod error;
pub mod import;
pub mod model;
pub mod render;
pub mod repo;
pub mod share;
#[cfg(test)]
pub(crate) mod testing;
mod validate;

pub use error::ProfileError;
pub use import::{
    ImportError, ImportPreview, Imported, PreviewAcl, PreviewForward, PreviewListener,
    PreviewSocks5, import_toml, import_toml_with_id,
};
pub use model::{
    ConfigSource, DEFAULT_MTU, Encryption, Exposure, IpMode, MAX_DEV_NAME_CHARS,
    MAX_HOSTNAME_CHARS, MAX_LIST_ENTRIES, MAX_NAME_CHARS, MTU_RANGE, NetworkProfile, ProfileId,
    RelayWhitelist, Socks5, SubnetProxy, clean_name, parse_route,
};
pub use render::check_config_server;
pub use repo::{KeyValue, LoadedProfiles, MemoryKv, NshStorage, RepoError};
pub use share::{
    QR_QUIET_ZONE, QrMatrix, ShareError, decode_qr_image, import_qr_image, parse_share_link,
    parse_share_link_with_id, qr_matrix, share_link,
};
