//! The desktop app's own behavior: keeping to the tray when the window closes, starting quietly (window hidden), and
//! starting at login. The first two are the user's choices kept in storage; the login item's truth is the OS's, read
//! at start. Elsewhere (phones, the web) nothing here runs and Settings says so.
//!
//! The tray attaches once the app starts: at once on macOS and Windows, after a D-Bus check for a tray host on Linux
//! (off the UI thread). Without a host the window keeps quitting on close, and "keep to the tray" cannot turn on.

use std::sync::Arc;

use nsh_core::{prelude::*, storage::Storage};

use super::Store;
use crate::{
    core::{
        desktop::{self, DesktopError, LoginItem, Tray, TrayText},
        platform::Platform,
        work::off_thread,
    },
    i18n::Strings,
};

/// The storage key of "keep to the tray", set to [`OFF`] while the user turned it off (it is on by default: closing
/// the window then never stops this device's networks by surprise).
const TRAY_KEY: &str = "desktop.tray";
/// The storage key of "start quietly", set to [`ON`] while it is on.
const QUIET_KEY: &str = "desktop.quiet-start";
const ON: &str = "on";
const OFF: &str = "off";
/// Test runs set this storage key to `memory` so the developer's login items are never read or written.
#[cfg(debug_assertions)]
const TEST_LOGIN_ITEM_KEY: &str = "test.login-item";

/// Where the login item stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginStatus {
    /// Being read from the OS.
    Reading,
    /// Known: [`DesktopStore::login`] is the OS's state.
    Ready,
    /// Being changed.
    Changing,
    /// The last read or change failed, in the OS's words; the switch shows what was last known.
    Failed(String),
}

/// See the module docs. A `Copy` handle of signals, like [`Store`].
#[derive(Clone, Copy)]
pub struct DesktopStore {
    /// This platform is a desktop.
    local: bool,
    /// Goes false when the owner is disposed; tasks check it before touching a signal.
    alive: Signal<bool>,
    tray: Stored<Option<Tray>>,
    /// Whether this desktop draws tray icons; `None` until known.
    tray_host: Signal<Option<bool>>,
    keep_in_tray: Signal<bool>,
    quiet_start: Signal<bool>,
    login_item: Stored<Option<Arc<dyn LoginItem>>>,
    login: Signal<bool>,
    login_status: Signal<LoginStatus>,
}

/// The tray's words in `strings`' language.
fn tray_text(strings: &Strings) -> TrayText {
    TrayText {
        tooltip: strings.desktop.tray_tooltip.to_owned(),
        show: strings.desktop.tray_show.to_owned(),
        quit: strings.desktop.tray_quit.to_owned(),
    }
}

/// The login item: the OS's, or in a debug build a test's in-memory one.
fn login_item() -> Option<Arc<dyn LoginItem>> {
    #[cfg(debug_assertions)]
    if Storage::get_item(TEST_LOGIN_ITEM_KEY).as_deref() == Some("memory") {
        return Some(desktop::memory_login_item());
    }
    desktop::system_login_item()
}

impl DesktopStore {
    /// The choices the user kept.
    pub(super) fn new(platform: Platform) -> Self {
        let local = platform.is_desktop();
        Self {
            local,
            alive: signal(true),
            tray: stored(None),
            tray_host: signal(None),
            keep_in_tray: signal(Storage::get_item(TRAY_KEY).as_deref() != Some(OFF)),
            quiet_start: signal(Storage::get_item(QUIET_KEY).as_deref() == Some(ON)),
            login_item: stored(if local { login_item() } else { None }),
            login: signal(false),
            login_status: signal(LoginStatus::Reading),
        }
    }

    /// Reads the login item and attaches the tray to the window (a real window only: headless runs have none). The
    /// tray's words follow the language.
    pub(super) fn start(&self, store: Store) {
        if !self.local {
            return;
        }
        self.read_login();
        match desktop::tray_host_known() {
            Some(host) => self.attach(store, host),
            None => {
                let this = *self;
                spawn(async move {
                    let host = off_thread(desktop::probe_tray_host)
                        .await
                        .unwrap_or_else(|error| {
                            log::warn!("the tray host check did not finish: {error}");
                            false
                        });
                    if this.alive() {
                        this.attach(store, host);
                    }
                });
            }
        }
        let this = *self;
        effect(move || {
            let text = tray_text(store.strings());
            untracked(|| {
                this.tray.with(|tray| {
                    if let Some(tray) = tray {
                        tray.set_text(text);
                    }
                });
            });
        });
    }

    /// Attaches the tray now that it is known whether this desktop draws tray icons.
    fn attach(&self, store: Store, host: bool) {
        self.tray_host.set(Some(host));
        if !host {
            return;
        }
        let keep = self.keep_in_tray.get_untracked();
        let quiet = keep && self.quiet_start.get_untracked();
        let text = untracked(|| tray_text(store.strings()));
        match desktop::install_tray(text, keep, quiet) {
            Ok(tray) => {
                self.tray.with(|slot| *slot = Some(tray));
                // A real app window: point an existing login item at this executable (the app may have moved).
                self.refresh_login();
            }
            Err(DesktopError::NoWindow) => log::info!("no app window here: no tray"),
            Err(error) => log::error!("the tray: {error}"),
        }
    }

    /// Whether the owner is still there.
    fn alive(&self) -> bool {
        self.alive.try_get().is_some()
    }

    /// Whether this platform has these settings (tracked nowhere: it never changes).
    pub fn available(&self) -> bool {
        self.local
    }

    /// Whether this desktop draws tray icons (tracked); `None` until known.
    pub fn tray_host(&self) -> Option<bool> {
        self.tray_host.get()
    }

    /// Whether closing the window keeps the app in the tray (tracked): the user's choice, and a tray to keep to.
    pub fn keep_in_tray(&self) -> bool {
        self.keep_in_tray.get() && self.tray_host.get() != Some(false)
    }

    /// Turns keeping to the tray on or off, now and for later starts.
    pub fn set_keep_in_tray(&self, on: bool) {
        self.keep_in_tray.set(on);
        let kept = if on {
            Storage::remove_item(TRAY_KEY);
            Ok(())
        } else {
            Storage::set_item(TRAY_KEY, OFF)
        };
        if let Err(full) = kept {
            log::warn!(
                "the tray choice was not kept: storage full at {} bytes",
                full.bytes
            );
        }
        self.tray.with(|tray| {
            if let Some(tray) = tray {
                tray.set_keep_in_tray(on);
            }
        });
    }

    /// Whether the app starts with its window hidden (tracked): the user's choice, while it keeps to the tray.
    pub fn quiet_start(&self) -> bool {
        self.quiet_start.get() && self.keep_in_tray()
    }

    /// Turns starting quietly on or off; it applies from the next start.
    pub fn set_quiet_start(&self, on: bool) {
        self.quiet_start.set(on);
        let kept = if on {
            Storage::set_item(QUIET_KEY, ON)
        } else {
            Storage::remove_item(QUIET_KEY);
            Ok(())
        };
        if let Err(full) = kept {
            log::warn!(
                "the quiet start choice was not kept: storage full at {} bytes",
                full.bytes
            );
        }
    }

    /// Whether the app starts at login (tracked), as last read or set.
    pub fn login(&self) -> bool {
        self.login.get()
    }

    /// Where the login item stands (tracked).
    pub fn login_status(&self) -> LoginStatus {
        self.login_status.get()
    }

    /// Reads the login item from the OS.
    fn read_login(&self) {
        let Some(item) = self.login_item.get() else {
            self.login_status
                .set(LoginStatus::Failed(DesktopError::Unsupported.to_string()));
            return;
        };
        self.login_status.set(LoginStatus::Reading);
        let this = *self;
        spawn(async move {
            let answer = off_thread(move || item.enabled()).await;
            this.settle_login(answer.map_err(|error| error.to_string()), None);
        });
    }

    /// Rewrites an enabled login item so it starts this executable.
    fn refresh_login(&self) {
        let Some(item) = self.login_item.get() else {
            return;
        };
        spawn(async move {
            let answer = off_thread(move || match item.enabled() {
                Ok(true) => item.set_enabled(true),
                Ok(false) => Ok(()),
                Err(error) => Err(error),
            })
            .await;
            match answer {
                Ok(Ok(())) => {}
                Ok(Err(error)) => log::warn!("the login item was not refreshed: {error}"),
                Err(error) => log::warn!("the login item was not refreshed: {error}"),
            }
        });
    }

    /// Turns starting at login on or off in the OS.
    pub fn set_login(&self, on: bool) {
        let Some(item) = self.login_item.get() else {
            return;
        };
        if self.login_status.get_untracked() == LoginStatus::Changing {
            return;
        }
        self.login_status.set(LoginStatus::Changing);
        let this = *self;
        spawn(async move {
            let answer = off_thread(move || item.set_enabled(on).map(|()| on)).await;
            this.settle_login(answer.map_err(|error| error.to_string()), Some(on));
        });
    }

    /// Records the answer of a read (`asked` `None`) or a change to `asked`.
    fn settle_login(
        &self,
        answer: Result<Result<bool, DesktopError>, String>,
        asked: Option<bool>,
    ) {
        if !self.alive() {
            return;
        }
        match answer {
            Ok(Ok(on)) => {
                batch(|| {
                    self.login.set(on);
                    self.login_status.set(LoginStatus::Ready);
                });
            }
            Ok(Err(error)) => {
                log::warn!("the login item ({asked:?}): {error}");
                self.login_status
                    .set(LoginStatus::Failed(error.to_string()));
            }
            Err(error) => {
                log::warn!("the login item ({asked:?}): {error}");
                self.login_status.set(LoginStatus::Failed(error));
            }
        }
    }
}
