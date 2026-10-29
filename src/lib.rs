//! EasyTierUI: one client for EasyTier on every platform. The root component mounts the shell over the app's
//! state; `nsh run`, `nsh package` and `cargo run` mount it.

pub mod components;
pub mod core;
pub mod deploy;
pub mod i18n;
pub mod pages;
pub mod profile;
mod shell;
pub mod store;

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{core::platform::Platform, store::Store};

/// The app's UI set (`nsh.json` `"rust"`): what a TSX entry may import from
/// `easytierui/native` (`README.md`, "Adding a TSX entry").
///
/// Every NSH runner (`nsh run`, `nsh package`) calls this from its `main` before it opens a window, so it is also where
/// the elevated re-run of this executable (the OS-service step of `core::sidecar::privileged`) is caught: that run
/// does its one job and exits without ever starting the UI. `cargo run` catches it in `src/main.rs`.
#[native_module("easytierui/native")]
fn native(registry: &mut Registry) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    if let Some(code) = core::sidecar::privileged::dispatch(std::env::args_os()) {
        #[expect(
            clippy::exit,
            reason = "the elevated helper run of this executable must end before any UI starts"
        )]
        std::process::exit(i32::from(code));
    }
    registry.register::<App>();
}

/// The application's root component.
#[component]
pub fn App() -> impl IntoElement {
    let store = Store::new(Platform::from_os(os()));
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(shell::shell(store))
}
