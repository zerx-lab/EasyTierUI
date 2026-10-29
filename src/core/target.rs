//! Targets: what the pages act on — this device, an easytier-web server, the control plane hosted here.

use nsh_core::prelude::SharedString;

use super::platform::Platform;

/// A target's stable key: list rows and per-target state are keyed by it, never by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetId(u32);

impl TargetId {
    /// The first target of every installation: this device, or the serving server in the web build.
    pub const PRIMARY: TargetId = TargetId(0);
    /// The easytier-web this desktop hosts, while it is set up.
    pub const HOSTED: TargetId = TargetId(1);
    /// The first id of a server the user added; later ones count up.
    const FIRST_ADDED: u32 = 16;

    /// The id a newly added server gets: after every id in `taken` and after `high_water`, the highest id ever
    /// handed out (persisted by the store), so an id is never reused, even after its server was removed. `None`
    /// once ids run out.
    pub fn next_after(
        taken: impl IntoIterator<Item = TargetId>,
        high_water: u32,
    ) -> Option<TargetId> {
        let highest = taken
            .into_iter()
            .map(|TargetId(n)| n)
            .filter(|n| *n >= Self::FIRST_ADDED)
            .chain(std::iter::once(high_water))
            .max()
            .unwrap_or(0);
        if highest < Self::FIRST_ADDED {
            Some(TargetId(Self::FIRST_ADDED))
        } else {
            highest.checked_add(1).map(TargetId)
        }
    }

    /// The id as a number, for storage keys.
    pub fn number(self) -> u32 {
        self.0
    }

    /// The id stored as `number`; `None` for the reserved ids, which are never stored.
    pub fn from_number(number: u32) -> Option<TargetId> {
        (number >= Self::FIRST_ADDED).then_some(TargetId(number))
    }
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
    /// An easytier-web the user added: the base URL of its REST API (`https://et.example.com`), validated by the
    /// console when added.
    Url(String),
    /// The easytier-web this desktop hosts, on loopback.
    Loopback {
        /// Its REST API port on `127.0.0.1`.
        port: u16,
    },
}

/// What the app knows about a target's state, as its backend reported it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetStatus {
    /// Reachable, and nothing of it is running.
    Stopped,
    /// The app has not connected to it yet: nothing about it is known.
    NotConnected,
    /// Signing in or starting: not confirmed yet.
    Connecting,
    /// Signed in (a server) or running (this device's networks, the hosted control plane), as last reported.
    Online,
    /// Unreachable, rejected the session, or failed to start.
    Failed,
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

    /// Whether the console pages can act on it (an easytier-web server, added or hosted).
    pub fn is_server(&self) -> bool {
        matches!(self.kind, TargetKind::Server | TargetKind::Hosted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn added_servers_get_fresh_ids_past_the_reserved_ones() {
        let first = TargetId::next_after([TargetId::PRIMARY, TargetId::HOSTED], 0).unwrap();
        assert_eq!(first.number(), 16);
        let second = TargetId::next_after([TargetId::PRIMARY, first], 0).unwrap();
        assert_eq!(second.number(), 17);
        assert_eq!(TargetId::from_number(17), Some(second));
        assert_eq!(TargetId::from_number(1), None);
    }

    #[test]
    fn an_id_is_never_handed_out_again_after_its_server_was_removed() {
        // 16 and 17 were handed out; 17 was removed. The high-water mark keeps 17 from coming back.
        let first = TargetId::from_number(16).unwrap();
        assert_eq!(
            TargetId::next_after([TargetId::PRIMARY, first], 17)
                .unwrap()
                .number(),
            18
        );
        // Without the mark the highest listed id decides; a mark below it changes nothing.
        assert_eq!(
            TargetId::next_after([TargetId::PRIMARY, first], 3)
                .unwrap()
                .number(),
            17
        );
        assert_eq!(TargetId::next_after([], 0).unwrap().number(), 16);
        // The ids run out instead of wrapping.
        assert_eq!(TargetId::next_after([], u32::MAX), None);
    }
}
