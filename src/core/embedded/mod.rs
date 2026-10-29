//! `embedded`: EasyTier's library in process on iOS and Android, behind the same [`NodeBackend`] as the desktops'
//! sidecar (a platform constraint: iOS cannot spawn processes, and a phone's TUN descriptor comes from a VPN
//! service in this process).
//!
//! The library is linked by the `easytier-mobile` bridge crate, and only [`backend`] names it. [`map`] — the adapter
//! from the library's running-info JSON to [`NodeSnapshot`](crate::core::state::NodeSnapshot) — is plain text-in,
//! types-out code that also builds for the desktops' tests, so its fixtures run there.
//!
//! This build runs every node without a TUN (NSH 0.3's Rust-app packaging cannot add an Android `VpnService` or an
//! iOS network extension): subnet proxy, SOCKS5 and port forwarding work; a TUN launch is `Unsupported`.

pub mod map;

#[cfg(any(target_os = "ios", target_os = "android"))]
mod backend;

#[cfg(any(target_os = "ios", target_os = "android"))]
pub use backend::EmbeddedBackend;

/// The EasyTier version linked into this build (`2.6.4-8428a89d`), for the settings page: the library updates with
/// the app.
#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn library_version() -> String {
    easytier_mobile::version()
}
