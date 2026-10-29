//! Sidebar items (DESIGN.md "Navigation"): 32 pt rows in Body type; hover and selection share `sidebar-accent`; the
//! selected item adds a 2 pt Cobalt marker at its leading edge.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    icon::{ICON_SIZE, Icon, icon_colored},
    text::control_radius,
};

/// The selection marker's width: DESIGN.md's 2 pt hairline accent, a physical size, not a spacing step.
const MARKER_WIDTH: Points = Points(2.0);

/// A navigation row showing `icon` and `label`, highlighted while `selected`, found in tests as `id:<test_id>`. A
/// plain view with the button role and the selected state (nsh-base's `Button` does the same, but takes no test id).
pub fn nav_item(
    icon: Icon,
    label: impl IntoReactive<SharedString>,
    test_id: &str,
    selected: impl Fn() -> bool + Copy + 'static,
    on_press: impl Fn() + 'static,
) -> View {
    let row = view()
        .test_id(test_id)
        .role(AccessibilityRole::Button)
        .accessibility_state(move || AccessibilityState {
            selected: selected(),
            ..AccessibilityState::default()
        })
        .on_press(on_press)
        .flex_row()
        .items_center()
        .gap_2()
        .px_2p5()
        .py_1p5()
        .rounded(control_radius)
        .bg(move || {
            if selected() {
                theme().sidebar_accent
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .hover(|s| s.bg(move || theme().sidebar_accent))
        .child(
            view()
                .absolute()
                .left_0()
                .top_1p5()
                .bottom_1p5()
                .w(MARKER_WIDTH)
                .rounded_full()
                .bg(move || {
                    if selected() {
                        theme().sidebar_primary
                    } else {
                        AdaptiveColor::TRANSPARENT
                    }
                }),
        )
        .child(icon_colored(icon, ICON_SIZE, move || {
            if selected() {
                theme().sidebar_foreground
            } else {
                theme().muted_foreground
            }
        }))
        .child(
            text(label)
                .text_sm()
                .flex_1()
                .text_color(move || theme().sidebar_foreground),
        );
    if is_desktop() {
        row.min_h_8()
    } else {
        row.min_h_11()
    }
}
