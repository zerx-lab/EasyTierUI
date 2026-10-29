//! Linux: NSH's window is a `gtk::ApplicationWindow` of the default `gtk::Application`. Closing it is caught with
//! GTK's `close-request` (hidden instead while the app keeps to the tray), and the application's `quit` action (Ctrl+Q)
//! is replaced so that it still quits. Requests from other threads go through the default main context's idle queue.
//! The tray host is a StatusNotifierWatcher on the session bus (KDE, and GNOME with the AppIndicator extension);
//! without one there is no tray, and closing the window quits.

use std::cell::Cell;

use gtk4::{self as gtk, gio, glib, prelude::*};

use crate::{
    ShellError,
    shell::{self, Action},
};

/// The StatusNotifierWatcher's bus name and object path.
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
/// How long the tray host check waits for D-Bus, in milliseconds.
const PROBE_TIMEOUT_MS: i32 = 1000;

thread_local! {
    /// `hook_window` ran: the close request and the quit action are ours.
    static HOOKED: Cell<bool> = const { Cell::new(false) };
    /// The app is quitting: closes pass through.
    static QUITTING: Cell<bool> = const { Cell::new(false) };
}

fn application() -> Option<gtk::Application> {
    gio::Application::default()?
        .downcast::<gtk::Application>()
        .ok()
}

fn window() -> Option<gtk::Window> {
    application()?.windows().into_iter().next()
}

pub(crate) fn tray_host_known() -> Option<bool> {
    None
}

pub(crate) fn probe_tray_host() -> bool {
    match tray_host_registered() {
        Ok(registered) => registered,
        Err(error) => {
            log::info!("no tray host on this desktop: {error}");
            false
        }
    }
}

/// Asks the StatusNotifierWatcher whether a host (a panel that draws the icons) has registered.
fn tray_host_registered() -> Result<bool, glib::Error> {
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)?;
    let reply = bus.call_sync(
        Some(WATCHER),
        WATCHER_PATH,
        "org.freedesktop.DBus.Properties",
        "Get",
        Some(&(WATCHER, "IsStatusNotifierHostRegistered").to_variant()),
        Some(glib::VariantTy::new("(v)").map_err(|error| {
            glib::Error::new(gio::IOErrorEnum::InvalidArgument, &error.to_string())
        })?),
        gio::DBusCallFlags::NONE,
        PROBE_TIMEOUT_MS,
        gio::Cancellable::NONE,
    )?;
    Ok(reply
        .child_value(0)
        .as_variant()
        .and_then(|value| value.get::<bool>())
        .unwrap_or(false))
}

pub(crate) fn has_window() -> bool {
    gtk::is_initialized_main_thread() && window().is_some()
}

/// Nothing to do before the window shows: it hides once the main loop runs ([`request`]).
pub(crate) fn conceal() {}

pub(crate) fn reveal() {}

/// The UI thread is already in GTK's main loop: no wait is needed.
pub(crate) fn defer(f: impl FnOnce() + Send + 'static) {
    f();
}

pub(crate) fn hook_window() -> Result<(), ShellError> {
    if HOOKED.get() {
        return Ok(());
    }
    let app = application().ok_or(ShellError::NoWindow)?;
    let window = window().ok_or(ShellError::NoWindow)?;
    window.connect_close_request(|window| {
        if shell::keeps_in_tray() && !QUITTING.get() {
            window.set_visible(false);
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    // NSH's Ctrl+Q closes every window, which would now hide them: quit for real instead.
    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(|_, _| shell::perform(Action::Quit));
    app.add_action(&quit);
    HOOKED.set(true);
    Ok(())
}

pub(crate) fn show_window() {
    let Some(window) = window() else {
        log::warn!("no window to show");
        return;
    };
    window.set_visible(true);
    window.present();
}

pub(crate) fn hide_window() {
    if let Some(window) = window() {
        window.set_visible(false);
    }
}

pub(crate) fn quit() {
    QUITTING.set(true);
    // Closing the last window ends the application's run, as NSH's own Quit does.
    if let Some(app) = application() {
        for window in app.windows() {
            window.close();
        }
    }
}

/// Runs `action` on the UI thread once it is idle (behind the window's first presentation).
pub(crate) fn request(action: Action) {
    glib::idle_add_once(move || shell::perform(action));
}

/// The window manager decides focus on Linux: nothing to grant.
pub(crate) fn allow_foreground() {}
