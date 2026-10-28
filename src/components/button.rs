//! Buttons (DESIGN.md "Buttons"): control radius, 32 pt on the desktops, 44 pt on touch screens.

use nsh_base::{ButtonElement, button, theme};
use nsh_core::prelude::*;

use super::text::control_radius;

/// A button's emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Cobalt fill: the one primary action of a screen region.
    Primary,
    /// Well fill: other actions.
    Secondary,
    /// No fill: toolbars and dismissals.
    Ghost,
}

/// A labelled button of `kind`.
pub fn action_button(
    kind: ButtonKind,
    label: impl IntoReactive<SharedString>,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let label_color = move || match kind {
        ButtonKind::Primary => theme().primary_foreground,
        ButtonKind::Secondary | ButtonKind::Ghost => theme().foreground,
    };
    let base = button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .justify_center()
        .px_3()
        .py_1p5()
        .rounded(control_radius)
        .disabled_style(|s| s.opacity(0.5))
        .child(
            text(label)
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(label_color),
        );
    let sized = if is_desktop() {
        base.min_h_8()
    } else {
        base.min_h_11().px_4()
    };
    match kind {
        ButtonKind::Primary => sized
            .bg(move || theme().primary)
            .active(|s| s.opacity(0.85)),
        ButtonKind::Secondary => sized
            .bg(move || theme().secondary)
            .hover(|s| s.bg(move || theme().accent))
            .active(|s| s.bg(move || theme().accent)),
        ButtonKind::Ghost => sized
            .hover(|s| s.bg(move || theme().accent))
            .active(|s| s.bg(move || theme().accent)),
    }
}
