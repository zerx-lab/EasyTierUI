//! The Status Lamp (DESIGN.md signature): an 8 pt pill in the lamp hue followed by its word, never color alone.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::text::app_color;

/// What a lamp reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lamp {
    /// Running, direct, passed: Lamp Green.
    Good,
    /// Degraded, relayed, attention: Relay Amber.
    Degraded,
    /// Failed, unreachable: Fault Red.
    Failed,
    /// Stopped or unknown: a hollow ring.
    Idle,
}

/// The lamp `lamp` beside its word `label`.
pub fn status_lamp(
    lamp: impl Fn() -> Lamp + Copy + 'static,
    label: impl IntoReactive<SharedString>,
) -> View {
    let fill = move || match lamp() {
        Lamp::Good => app_color("success"),
        Lamp::Degraded => app_color("warning"),
        Lamp::Failed => theme().destructive,
        Lamp::Idle => AdaptiveColor::TRANSPARENT,
    };
    let ring = move || match lamp() {
        Lamp::Idle => theme().muted_foreground,
        Lamp::Good | Lamp::Degraded | Lamp::Failed => fill(),
    };
    view()
        .flex_row()
        .items_center()
        .gap_1p5()
        .child(
            view()
                .size_2()
                .flex_shrink_0()
                .rounded_full()
                .border_1()
                .border_color(ring)
                .bg(fill),
        )
        .child(
            text(label)
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().muted_foreground),
        )
}
