//! Roles and the core boundary: the app's domain types and what each platform can do.
//!
//! Pages and the store speak these types only; nothing above this module names an EasyTier crate, binary, flag or
//! field (AGENTS.md, "Roles and the core boundary").

pub mod platform;
pub mod role;
pub mod target;
