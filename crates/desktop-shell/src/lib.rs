//! EasyTierUI's desktop integration that NSH 0.3 has no API for: a tray icon with a menu (show the window, quit),
//! closing the window to the tray instead of quitting, starting with the window hidden, one running instance per
//! user, and the login item that starts the app when the user signs in.
//!
//! NSH owns the window and its event loop; this crate finds that window and attaches to it: AppKit's window delegate
//! (macOS, `macos.rs`), a Win32 subclass of NSH's window (Windows, `windows.rs`), GTK's `close-request` (Linux,
//! `linux.rs`). The window only ever hides while a tray icon exists, so it can always be brought back. NSH runs the
//! app's code on its engine thread, so the API may be called from any thread: every window and tray step is posted to
//! the UI thread that owns the window, as are the requests from the tray's and the instance listener's threads.
//!
//! On targets without desktop windows the crate is empty.

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod error;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub mod instance;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub mod login;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod shell;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod tray;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub use self::{
    error::ShellError,
    shell::{Shell, TrayIcons, TrayLabels, has_window, probe_tray_host, quit, tray_host_known},
};
