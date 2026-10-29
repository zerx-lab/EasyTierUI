//! Roles and the core boundary: the app's domain types and what each platform can do.
//!
//! Pages and the store speak these types only; nothing above this module names an EasyTier crate, binary, flag or
//! field (AGENTS.md, "Roles and the core boundary").

pub mod acl;
pub mod desktop;
pub mod keychain;
pub mod net;
pub mod node;
#[cfg(not(target_arch = "wasm32"))]
pub mod pairs;
pub mod platform;
pub mod role;
pub mod secret;
pub mod settings;
pub mod state;
pub mod target;
pub mod version;
pub mod work;

#[cfg(not(target_arch = "wasm32"))]
pub mod paths;

pub mod remote;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub mod controlplane;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub mod install;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub mod sidecar;

#[cfg(any(test, target_os = "ios", target_os = "android"))]
pub mod embedded;
