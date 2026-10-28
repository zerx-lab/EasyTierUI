//! The Capability Notice (DESIGN.md): where this device cannot run a feature, the page says so in one sentence and
//! offers the remote alternative. Never a disabled page without a way forward.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    button::{ButtonKind, action_button},
    text::app_color,
};

/// The notice `message`, followed by the alternative `action` that `on_action` takes.
pub fn capability_notice(
    message: impl IntoReactive<SharedString>,
    action: impl IntoReactive<SharedString>,
    on_action: impl Fn() + 'static,
) -> View {
    view()
        .role(AccessibilityRole::Summary)
        .items_start()
        .gap_3()
        .px_4()
        .py_3()
        .rounded(move || theme().radius)
        .bg(move || app_color("primary-muted"))
        .child(
            text(message)
                .text_sm()
                .text_color(move || theme().foreground),
        )
        .child(action_button(ButtonKind::Primary, action, on_action))
}
