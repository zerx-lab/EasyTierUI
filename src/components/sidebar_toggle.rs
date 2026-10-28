//! The button that shows and hides the sidebar, with a sidebar glyph drawn from views (a framed window with its
//! leading column ruled off), as the platforms' own sidebar buttons look.

use nsh_base::{ButtonElement, button, theme};
use nsh_core::prelude::*;

use super::text::control_radius;

/// The glyph's ruling: a 1 pt hairline, a physical size, not a spacing step.
const STROKE: Points = Points(1.0);

/// A 32 pt toolbar button named `label` for screen readers; `on_press` shows or hides the sidebar.
pub fn sidebar_toggle(
    label: impl Fn() -> &'static str + 'static,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let ink = move || theme().muted_foreground;
    let glyph = view()
        .w_4()
        .h_3p5()
        .rounded_sm()
        .border_1()
        .border_color(ink)
        .child(
            view()
                .absolute()
                .left_1p5()
                .top_0()
                .bottom_0()
                .w(STROKE)
                .bg(ink),
        );
    button()
        .accessibility_label(move || Some(SharedString::from(label())))
        .on_press(on_press)
        .size_8()
        .items_center()
        .justify_center()
        .rounded(control_radius)
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().accent))
        .child(glyph)
}
