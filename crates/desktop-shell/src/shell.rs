//! The shell's state on the UI thread and the API the app uses: [`Shell`] attaches to the window and owns the tray
//! icon; [`quit`] ends the app as its menu's Quit would.

use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
};

use crate::{ShellError, platform, tray::Tray};

/// What the tray's menu and tooltip say, in the UI's language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayLabels {
    /// The tooltip (the app's name).
    pub tooltip: String,
    /// The item that shows the window.
    pub show: String,
    /// The item that quits the app.
    pub quit: String,
}

/// The tray icon's images: 8-bit RGBA PNGs.
#[derive(Debug, Clone, Copy)]
pub struct TrayIcons {
    /// macOS: a black glyph on transparency, drawn by the menu bar as a template in both appearances.
    pub template: &'static [u8],
    /// Windows and Linux: the full-color icon.
    pub color: &'static [u8],
}

/// Something the tray, the instance listener or the app asked the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    /// Show the window and bring it to the front.
    Show,
    /// Hide the window (only while a tray icon exists to bring it back).
    Hide,
    /// End the app: the window closes for real, NSH stops the engine.
    Quit,
}

struct Config {
    icons: TrayIcons,
    labels: TrayLabels,
}

thread_local! {
    /// Closing the window hides it. True only while [`TRAY`] holds an icon.
    static KEEP: Cell<bool> = const { Cell::new(false) };
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
    static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
}

/// Whether closing the window hides it now (read by the platform's close handler).
pub(crate) fn keeps_in_tray() -> bool {
    KEEP.get()
}

/// Whether this desktop shows tray icons, when that is known without asking: `Some(true)` on macOS and Windows; `None`
/// on Linux, where [`probe_tray_host`] asks D-Bus.
pub fn tray_host_known() -> Option<bool> {
    platform::tray_host_known()
}

/// Whether this desktop shows tray icons. Blocking on Linux (a D-Bus call to the StatusNotifierWatcher, up to a
/// second): run it off the UI thread.
pub fn probe_tray_host() -> bool {
    platform::probe_tray_host()
}

/// Whether this thread has the app's NSH window: false in headless runs, where nothing here may touch the OS.
pub fn has_window() -> bool {
    platform::has_window()
}

/// Ends the app after the current event, as its menu's Quit would: NSH closes the window and stops the engine.
pub fn quit() {
    platform::request(Action::Quit);
}

/// The window's tray integration, attached while it lives. Made and used on the UI thread only.
pub struct Shell {
    _ui_thread: PhantomData<Rc<()>>,
}

impl Shell {
    /// Attaches to the app's window. With `keep_in_tray` a tray icon shows and closing the window hides it; with
    /// `start_hidden` too, the window hides as soon as it is up (the tray icon brings it back). On macOS the work runs
    /// once AppKit's event loop has started, which status items need.
    ///
    /// # Errors
    ///
    /// [`ShellError::NoWindow`] in a headless run. A tray icon that cannot be made is logged and leaves closing the
    /// window to quit the app, since nothing could bring a hidden window back.
    pub fn install(
        icons: TrayIcons,
        labels: TrayLabels,
        keep_in_tray: bool,
        start_hidden: bool,
    ) -> Result<Self, ShellError> {
        if !platform::has_window() {
            return Err(ShellError::NoWindow);
        }
        CONFIG.set(Some(Config { icons, labels }));
        let start_hidden = start_hidden && keep_in_tray;
        if start_hidden {
            platform::conceal();
        }
        platform::defer(move || {
            if let Err(error) = platform::hook_window() {
                log::error!("the tray cannot attach to the window: {error}");
                platform::reveal();
                return;
            }
            apply_keep(keep_in_tray);
        });
        if start_hidden {
            // After the window is first shown: posted behind NSH's own show.
            platform::request(Action::Hide);
        }
        Ok(Self {
            _ui_thread: PhantomData,
        })
    }

    /// Shows or removes the tray icon; closing the window hides it while the icon shows.
    pub fn set_keep_in_tray(&self, on: bool) {
        platform::defer(move || apply_keep(on));
    }

    /// Words the tray's menu and tooltip in `labels`.
    pub fn set_labels(&self, labels: TrayLabels) {
        TRAY.with_borrow(|tray| {
            if let Some(tray) = tray {
                tray.relabel(&labels);
            }
        });
        CONFIG.with_borrow_mut(|config| {
            if let Some(config) = config {
                config.labels = labels;
            }
        });
    }

    /// Whether the tray icon shows now.
    pub fn tray_shown(&self) -> bool {
        TRAY.with_borrow(Option::is_some)
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        // The platform hooks stay (they may be running this very drop, inside the window's close); without the flag
        // they pass every close through.
        KEEP.set(false);
        drop(TRAY.take());
    }
}

/// Makes or drops the tray icon; closing hides the window only while the icon exists.
fn apply_keep(on: bool) {
    if on && TRAY.with_borrow(Option::is_none) {
        let made = CONFIG.with_borrow(|config| {
            config
                .as_ref()
                .map(|config| Tray::new(config.icons, &config.labels))
        });
        match made {
            Some(Ok(tray)) => {
                log::info!("the tray icon is shown");
                TRAY.set(Some(tray));
            }
            Some(Err(error)) => log::error!("{error}; closing the window quits the app"),
            None => log::warn!("the tray has no configuration yet"),
        }
    } else if !on {
        drop(TRAY.take());
    }
    KEEP.set(on && TRAY.with_borrow(Option::is_some));
}

/// Does `action` on the UI thread (where the platform's posted requests land).
pub(crate) fn perform(action: Action) {
    match action {
        Action::Show => platform::show_window(),
        Action::Hide if keeps_in_tray() => platform::hide_window(),
        // Nothing would bring it back: undo a start-hidden concealment instead.
        Action::Hide => platform::reveal(),
        Action::Quit => {
            KEEP.set(false);
            platform::quit();
        }
    }
}
