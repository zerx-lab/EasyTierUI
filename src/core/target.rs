//! Targets: what the pages act on — this device, an easytier-web server, the control plane hosted here.

use nsh_core::prelude::SharedString;

use super::platform::Platform;

/// A target's stable key: list rows and per-target state are keyed by it, never by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetId(u32);

impl TargetId {
    /// The first target of every installation: this device, or the serving server in the web build.
    pub const PRIMARY: TargetId = TargetId(0);
}

/// What a target is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// The device the app runs on.
    ThisDevice,
    /// An easytier-web server reached through its REST API.
    Server,
    /// The easytier-web this desktop hosts.
    Hosted,
}

/// Where a target's server is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOrigin {
    /// This device.
    Local,
    /// The origin that served the web build (its REST API is same-origin).
    SameOrigin,
}

/// What the app knows about a target's state, as its backend reported it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetStatus {
    /// Reachable, and nothing of it is running.
    Stopped,
    /// The app has not connected to it yet: nothing about it is known.
    NotConnected,
}

/// A target in the switcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Its key.
    pub id: TargetId,
    /// What it is.
    pub kind: TargetKind,
    /// Where it is.
    pub origin: TargetOrigin,
    /// The name the user gave it; `None` shows the name of its kind and origin.
    pub name: Option<SharedString>,
    /// Its state.
    pub status: TargetStatus,
}

impl Target {
    /// The targets a fresh installation starts with on `platform`: this device, or in a browser the server
    /// that served the page.
    pub fn initial(platform: Platform) -> Vec<Target> {
        let primary = match platform {
            Platform::Web => Target {
                id: TargetId::PRIMARY,
                kind: TargetKind::Server,
                origin: TargetOrigin::SameOrigin,
                name: None,
                status: TargetStatus::NotConnected,
            },
            Platform::MacOs
            | Platform::Windows
            | Platform::Linux
            | Platform::Ios
            | Platform::Android => Target {
                id: TargetId::PRIMARY,
                kind: TargetKind::ThisDevice,
                origin: TargetOrigin::Local,
                name: None,
                status: TargetStatus::Stopped,
            },
        };
        vec![primary]
    }
}
