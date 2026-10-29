//! Buttons (DESIGN.md "Buttons"): control radius, 32 pt on the desktops, 44 pt on touch screens.

use nsh_base::{ButtonElement, button, theme};
use nsh_core::prelude::*;

use super::{
    motion::pulse,
    text::{app_color, control_radius},
};

/// A button's emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Cobalt fill: the one primary action of a screen region.
    Primary,
    /// Well fill: other actions.
    Secondary,
    /// No fill: toolbars and dismissals.
    Ghost,
    /// Fault Red fill: only in a confirmation that names the object being destroyed.
    Destructive,
}

/// The label color that reads on `kind`'s fill.
fn label_color(kind: ButtonKind) -> AdaptiveColor {
    match kind {
        ButtonKind::Primary => theme().primary_foreground,
        ButtonKind::Destructive => app_color("destructive-foreground"),
        ButtonKind::Secondary | ButtonKind::Ghost => theme().foreground,
    }
}

/// The button without content: shape, size, fills and states of `kind`, pressing runs `on_press`.
fn shell(kind: ButtonKind, on_press: impl Fn() + 'static) -> ButtonElement {
    let base = button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .justify_center()
        .px_3()
        .py_1p5()
        .rounded(control_radius)
        .disabled_style(|s| s.opacity(0.5));
    let sized = if is_desktop() {
        base.min_h_8()
    } else {
        base.min_h_11().px_4()
    };
    match kind {
        ButtonKind::Primary => sized
            .bg(move || theme().primary)
            .active(|s| s.opacity(0.85)),
        ButtonKind::Destructive => sized
            .bg(move || theme().destructive)
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

/// The button's label in Control type.
fn label_text(kind: ButtonKind, label: impl IntoReactive<SharedString>) -> Text {
    let text = text(label)
        .font_weight(FontWeight::MEDIUM)
        .text_color(move || label_color(kind));
    if is_desktop() {
        text.text_sm()
    } else {
        text.text_base()
    }
}

/// A labelled button of `kind`.
///
/// ```ignore
/// action_button(ButtonKind::Primary, store.tr(|s| s.action_start), move || store.start())
/// ```
pub fn action_button(
    kind: ButtonKind,
    label: impl IntoReactive<SharedString>,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    shell(kind, on_press).child(label_text(kind, label))
}

/// A labelled button that is disabled while `busy()` holds and shows a pulsing ring over its label (DESIGN.md
/// "Focus / disabled / busy": it keeps the label's width, so the layout does not jump, and never takes a second
/// click). The label stays in the layout, hidden, so screen readers still hear it.
///
/// ```ignore
/// busy_button(ButtonKind::Primary, store.tr(|s| s.action_save), move || store.saving(), move || store.save())
/// ```
pub fn busy_button(
    kind: ButtonKind,
    label: impl IntoReactive<SharedString>,
    busy: impl Fn() -> bool + Copy + 'static,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let ring_pulse = pulse(busy);
    let spinner = when(busy, move || {
        view()
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .child(
                view()
                    .size_4()
                    .rounded_full()
                    .border_2()
                    .border_color(move || label_color(kind))
                    .bind_opacity(&ring_pulse),
            )
    });
    shell(kind, move || {
        // A press that lands in the frame before the disabled state shows must not start the work twice.
        if !busy() {
            on_press();
        }
    })
    .disabled(busy)
    .child(
        view()
            .items_center()
            .justify_center()
            .child(label_text(kind, label).opacity(move || if busy() { 0.0_f32 } else { 1.0_f32 }))
            .child(spinner),
    )
}
