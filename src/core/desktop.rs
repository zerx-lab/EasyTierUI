//! The desktop app's own shell, around `crates/desktop-shell`: the tray icon (show the window, quit), closing the
//! window to the tray, starting with the window hidden, one running instance, and the login item. Nothing here is
//! EasyTier's; the store reaches the OS only through this module, and pages only through the store.
//!
//! Elsewhere (phones, the web) there is no tray, no window to hide and no login item: [`install_tray`] answers
//! [`DesktopError::Unsupported`] and [`system_login_item`] `None`.

use std::sync::{Arc, Mutex, PoisonError};

/// The app's reverse-DNS id (`nsh.json` `"id"`): the login item's label and file name.
pub const APP_ID: &str = "com.example.easytierui";
/// The app's name as its login item shows it.
pub const APP_NAME: &str = "EasyTierUI";

/// What went wrong with the desktop shell or the login item.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DesktopError {
    /// This platform has no tray, window or login item of its own.
    #[error("this platform has no tray or login items")]
    Unsupported,
    /// No app window in this process (a headless run).
    #[error("no app window to attach the tray to")]
    NoWindow,
    /// The tray could not be attached to the window.
    #[error("{0}")]
    Tray(String),
    /// The login item could not be read or changed; the OS's words.
    #[error("{0}")]
    Login(String),
}

/// The tray's words, in the UI's language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayText {
    /// The tooltip.
    pub tooltip: String,
    /// The menu item that shows the window.
    pub show: String,
    /// The menu item that quits.
    pub quit: String,
}

/// Starting the app when the user signs in.
pub trait LoginItem: Send + Sync {
    /// Whether it is on.
    ///
    /// # Errors
    ///
    /// [`DesktopError::Login`] when its place cannot be read.
    fn enabled(&self) -> Result<bool, DesktopError>;

    /// Turns it on (pointing it at the app as it is now) or off.
    ///
    /// # Errors
    ///
    /// [`DesktopError::Login`] when it cannot be written or removed.
    fn set_enabled(&self, on: bool) -> Result<(), DesktopError>;
}

/// A login item in memory: tests, so they never touch the developer's login items.
#[derive(Debug, Default)]
pub struct MemoryLoginItem(Mutex<bool>);

impl LoginItem for MemoryLoginItem {
    fn enabled(&self) -> Result<bool, DesktopError> {
        Ok(*self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn set_enabled(&self, on: bool) -> Result<(), DesktopError> {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = on;
        Ok(())
    }
}

/// What claiming the single instance found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceClaim {
    /// This process is the app.
    First,
    /// The app already runs and was asked to show its window; this process is quitting.
    Second,
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod imp {
    use std::sync::Arc;

    use desktop_shell::{
        Shell, TrayIcons, TrayLabels,
        instance::{self, Claim},
        login::{self, LoginEntry},
    };

    use super::{APP_ID, APP_NAME, DesktopError, InstanceClaim, LoginItem, TrayText};

    const ICONS: TrayIcons = TrayIcons {
        template: include_bytes!("../../brand/tray-template.png"),
        color: include_bytes!("../../brand/tray.png"),
    };

    fn labels(text: TrayText) -> TrayLabels {
        TrayLabels {
            tooltip: text.tooltip,
            show: text.show,
            quit: text.quit,
        }
    }

    /// The tray icon and the window's close-to-tray, attached while it lives (its steps run on the UI thread).
    pub struct Tray(Shell);

    impl Tray {
        /// Shows or removes the tray icon; closing the window hides it while the icon shows.
        pub fn set_keep_in_tray(&self, on: bool) {
            self.0.set_keep_in_tray(on);
        }

        /// Rewords the tray.
        pub fn set_text(&self, text: TrayText) {
            self.0.set_labels(labels(text));
        }
    }

    /// Whether this desktop shows tray icons, when known without asking (`None`: ask [`probe_tray_host`]).
    pub fn tray_host_known() -> Option<bool> {
        desktop_shell::tray_host_known()
    }

    /// Whether this desktop shows tray icons; blocking on Linux (D-Bus), so off the UI thread.
    pub fn probe_tray_host() -> bool {
        desktop_shell::probe_tray_host()
    }

    /// Attaches the tray to the window: the icon shows with `keep_in_tray`, and the window hides at once with
    /// `start_hidden` too.
    ///
    /// # Errors
    ///
    /// [`DesktopError::NoWindow`] in a headless run.
    pub fn install_tray(
        text: TrayText,
        keep_in_tray: bool,
        start_hidden: bool,
    ) -> Result<Tray, DesktopError> {
        Shell::install(ICONS, labels(text), keep_in_tray, start_hidden)
            .map(Tray)
            .map_err(|error| match error {
                desktop_shell::ShellError::NoWindow => DesktopError::NoWindow,
                other => DesktopError::Tray(other.to_string()),
            })
    }

    /// Makes this process the app's one instance, or asks the running one to show its window and quits this one
    /// (before anything of it starts). A headless run, or a lock that cannot be used, counts as the first.
    pub fn claim_instance() -> InstanceClaim {
        if !desktop_shell::has_window() {
            return InstanceClaim::First;
        }
        let dir = match crate::core::paths::data_dir() {
            Ok(dir) => dir,
            Err(error) => {
                log::warn!("no single-instance lock: {error}");
                return InstanceClaim::First;
            }
        };
        match instance::claim(&dir) {
            Ok(Claim::First) => InstanceClaim::First,
            Ok(Claim::Second) => {
                log::info!("EasyTierUI already runs; it was asked to show its window");
                desktop_shell::quit();
                InstanceClaim::Second
            }
            Err(error) => {
                log::warn!("no single-instance lock: {error}");
                InstanceClaim::First
            }
        }
    }

    /// The OS's login item for this executable.
    struct SystemLoginItem;

    impl SystemLoginItem {
        fn entry() -> Result<LoginEntry, DesktopError> {
            let program = std::env::current_exe().map_err(|error| {
                DesktopError::Login(format!("the app's own path could not be read: {error}"))
            })?;
            Ok(LoginEntry {
                id: APP_ID.to_owned(),
                name: APP_NAME.to_owned(),
                program,
            })
        }
    }

    impl LoginItem for SystemLoginItem {
        fn enabled(&self) -> Result<bool, DesktopError> {
            login::is_enabled(&Self::entry()?)
                .map_err(|error| DesktopError::Login(error.to_string()))
        }

        fn set_enabled(&self, on: bool) -> Result<(), DesktopError> {
            login::set_enabled(&Self::entry()?, on)
                .map_err(|error| DesktopError::Login(error.to_string()))
        }
    }

    /// The OS's login item.
    pub fn system_login_item() -> Option<Arc<dyn LoginItem>> {
        Some(Arc::new(SystemLoginItem))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod imp {
    use std::{convert::Infallible, sync::Arc};

    use super::{DesktopError, InstanceClaim, LoginItem, TrayText};

    /// No tray here: the type has no values.
    pub struct Tray(Infallible);

    impl Tray {
        /// Never called: there is no tray to change.
        pub fn set_keep_in_tray(&self, _on: bool) {
            match self.0 {}
        }

        /// Never called: there is no tray to reword.
        pub fn set_text(&self, _text: TrayText) {
            match self.0 {}
        }
    }

    /// No tray host here.
    pub fn tray_host_known() -> Option<bool> {
        Some(false)
    }

    /// No tray host here.
    pub fn probe_tray_host() -> bool {
        false
    }

    /// No tray here.
    ///
    /// # Errors
    ///
    /// Always [`DesktopError::Unsupported`].
    pub fn install_tray(
        _text: TrayText,
        _keep_in_tray: bool,
        _start_hidden: bool,
    ) -> Result<Tray, DesktopError> {
        Err(DesktopError::Unsupported)
    }

    /// One app per device is the platform's own rule here.
    pub fn claim_instance() -> InstanceClaim {
        InstanceClaim::First
    }

    /// No login items here.
    pub fn system_login_item() -> Option<Arc<dyn LoginItem>> {
        None
    }
}

pub use imp::{
    Tray, claim_instance, install_tray, probe_tray_host, system_login_item, tray_host_known,
};

/// A shared in-memory login item (tests).
pub fn memory_login_item() -> Arc<dyn LoginItem> {
    Arc::new(MemoryLoginItem::default())
}
