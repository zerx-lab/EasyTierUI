//! A row of a single-choice list: a radio indicator (Cobalt when checked) and its label.

use nsh_base::{ButtonElement, button, theme};
use nsh_core::prelude::*;

/// The option `label` of a radio group, checked while `selected`; `on_press` chooses it. Rows after the first
/// draw the hairline above them (`separated`).
pub fn choice_row(
    label: impl IntoReactive<SharedString>,
    selected: impl Fn() -> bool + Copy + 'static,
    separated: bool,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let ring = move || {
        if selected() {
            theme().primary
        } else {
            theme().input
        }
    };
    let dot = move || {
        if selected() {
            theme().primary
        } else {
            AdaptiveColor::TRANSPARENT
        }
    };
    let row = button()
        .role(AccessibilityRole::Radio)
        .selected(selected)
        .on_press(on_press)
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_2()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().accent))
        .when(separated, |row| {
            row.border_t_1().border_color(move || theme().border)
        })
        .child(
            view()
                .size_4()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_color(ring)
                .child(view().size_2().rounded_full().bg(dot)),
        )
        .child(
            text(label)
                .text_sm()
                .flex_1()
                .text_color(move || theme().card_foreground),
        );
    if is_desktop() {
        row.min_h_9()
    } else {
        row.min_h_11()
    }
}
