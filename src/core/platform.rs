//! The platform the app runs on and what each role can do there.
//!
//! The matrix is AGENTS.md's ("Roles and the core boundary"): physical limits, not product choices. A feature the
//! platform cannot run locally is [`Availability::Elsewhere`]: its page still exists and offers the remote
//! alternative.

use nsh_core::platform::Os;

/// Where the app runs. Pages ask [`Capabilities`] what it can do here and never inspect the OS themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS desktop (AppKit).
    MacOs,
    /// Windows desktop (Win32).
    Windows,
    /// Linux desktop (GTK).
    Linux,
    /// iOS and iPadOS.
    Ios,
    /// Android.
    Android,
    /// A browser: the web build served next to easytier-web on a server.
    Web,
}

impl Platform {
    /// The platform of NSH's reported operating system.
    pub fn from_os(os: Os) -> Self {
        match os {
            Os::MacOs => Self::MacOs,
            Os::Windows => Self::Windows,
            Os::Linux => Self::Linux,
            Os::Ios => Self::Ios,
            Os::Android => Self::Android,
            Os::Web => Self::Web,
        }
    }

    /// A desktop OS: it has windows, a title bar the app draws, and can spawn EasyTier's programs.
    pub fn is_desktop(self) -> bool {
        matches!(self, Self::MacOs | Self::Windows | Self::Linux)
    }

    /// A phone or tablet: system navigation (headers, back gestures) and touch-sized targets.
    pub fn is_mobile(self) -> bool {
        matches!(self, Self::Ios | Self::Android)
    }

    /// What every role can do on this platform.
    pub fn capabilities(self) -> Capabilities {
        Capabilities::of(self)
    }
}

/// The backend that carries a feature on a platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// The official `easytier-core` release, run and supervised as a separate process.
    Sidecar,
    /// The official `easytier-web` release, run on this desktop as a separate process.
    ControlPlane,
    /// EasyTier's library in process (iOS cannot spawn processes; Android hands the VPN interface in process).
    Embedded,
    /// easytier-web's REST API on a server (same origin in the web build).
    Remote,
    /// Server deployment files rendered here; nothing runs on this device.
    Deploy,
}

/// Why this device cannot run a feature itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limitation {
    /// A browser has no TUN interface and no sockets: it cannot be a node.
    BrowserCannotJoinNetwork,
    /// A browser cannot run easytier-web: the web build is that server's dashboard.
    BrowserCannotRunPrograms,
    /// A phone or tablet cannot host a server reliably.
    MobileCannotHost,
}

/// Where a feature runs, seen from this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// On this device, through `Backend`.
    Local(Backend),
    /// Not on this device: the page acts on a remote target and says why.
    Elsewhere(Limitation),
}

impl Availability {
    /// The feature runs on this device.
    pub fn is_local(self) -> bool {
        matches!(self, Self::Local(_))
    }
}

/// What each role can do on one platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// This device in the mesh.
    pub node: Availability,
    /// Managing an easytier-web server.
    pub console: Availability,
    /// Running easytier-web here (status, first-run hardening).
    pub host: Availability,
    /// Rendering server deployment files.
    pub deploy: Availability,
}

impl Capabilities {
    /// The capabilities of `platform`.
    pub fn of(platform: Platform) -> Self {
        let console = Availability::Local(Backend::Remote);
        let deploy = Availability::Local(Backend::Deploy);
        match platform {
            Platform::MacOs | Platform::Windows | Platform::Linux => Self {
                node: Availability::Local(Backend::Sidecar),
                console,
                host: Availability::Local(Backend::ControlPlane),
                deploy,
            },
            Platform::Ios | Platform::Android => Self {
                node: Availability::Local(Backend::Embedded),
                console,
                host: Availability::Elsewhere(Limitation::MobileCannotHost),
                deploy,
            },
            Platform::Web => Self {
                node: Availability::Elsewhere(Limitation::BrowserCannotJoinNetwork),
                console,
                host: Availability::Elsewhere(Limitation::BrowserCannotRunPrograms),
                deploy,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Platform; 6] = [
        Platform::MacOs,
        Platform::Windows,
        Platform::Linux,
        Platform::Ios,
        Platform::Android,
        Platform::Web,
    ];

    #[test]
    fn desktops_run_every_role_through_official_binaries() {
        for platform in [Platform::MacOs, Platform::Windows, Platform::Linux] {
            let caps = platform.capabilities();
            assert_eq!(
                caps.node,
                Availability::Local(Backend::Sidecar),
                "{platform:?}"
            );
            assert_eq!(
                caps.host,
                Availability::Local(Backend::ControlPlane),
                "{platform:?}"
            );
        }
    }

    #[test]
    fn mobile_embeds_the_node_and_hosts_elsewhere() {
        for platform in [Platform::Ios, Platform::Android] {
            let caps = platform.capabilities();
            assert_eq!(
                caps.node,
                Availability::Local(Backend::Embedded),
                "{platform:?}"
            );
            assert_eq!(
                caps.host,
                Availability::Elsewhere(Limitation::MobileCannotHost),
                "{platform:?}"
            );
        }
    }

    #[test]
    fn a_browser_is_never_a_node_or_a_host() {
        let caps = Platform::Web.capabilities();
        assert_eq!(
            caps.node,
            Availability::Elsewhere(Limitation::BrowserCannotJoinNetwork)
        );
        assert_eq!(
            caps.host,
            Availability::Elsewhere(Limitation::BrowserCannotRunPrograms)
        );
    }

    #[test]
    fn console_and_deployment_work_everywhere() {
        for platform in ALL {
            let caps = platform.capabilities();
            assert_eq!(
                caps.console,
                Availability::Local(Backend::Remote),
                "{platform:?}"
            );
            assert_eq!(
                caps.deploy,
                Availability::Local(Backend::Deploy),
                "{platform:?}"
            );
        }
    }
}
