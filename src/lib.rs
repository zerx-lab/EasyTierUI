//! EasyTierUI: one client for EasyTier on every platform. The root component mounts the shell over the app's
//! state; `nsh run`, `nsh package` and `cargo run` mount it.

pub mod components;
pub mod core;
pub mod i18n;
pub mod pages;
mod shell;
pub mod store;

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{core::platform::Platform, store::Store};

/// The app's UI set (`nsh.json` `"rust"`): what a TSX entry may import from
/// `easytierui/native` (`README.md`, "Adding a TSX entry").
#[native_module("easytierui/native")]
fn native(registry: &mut Registry) {
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
