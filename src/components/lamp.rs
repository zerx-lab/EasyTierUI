//! The Status Lamp (DESIGN.md signature): an 8 pt pill in the lamp hue followed by its word, never color alone.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{motion::pulse, text::app_color};

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
    /// Starting, connecting, stopping: a hollow ring that pulses slowly (held still under Reduce Motion).
    Busy,
}

impl Lamp {
    /// The hue that reports this state in a bar or a mark: the lamp's fill, or `muted-foreground` for the hollow
    /// states.
    pub(super) fn hue(self) -> AdaptiveColor {
        match self {
            Self::Good => app_color("success"),
            Self::Degraded => app_color("warning"),
            Self::Failed => theme().destructive,
            Self::Idle | Self::Busy => theme().muted_foreground,
        }
    }
}

/// The lamp's 8 pt pill alone (no word): the caller puts the word next to it (a chip, a row), because a lamp is
/// never color alone. A `Busy` lamp pulses its opacity slowly and stops under Reduce Motion.
pub(super) fn lamp_dot(lamp: impl Fn() -> Lamp + Copy + 'static) -> View {
    let fill = move || match lamp() {
        Lamp::Good => app_color("success"),
        Lamp::Degraded => app_color("warning"),
        Lamp::Failed => theme().destructive,
        Lamp::Idle | Lamp::Busy => AdaptiveColor::TRANSPARENT,
    };
    let ring = move || match lamp() {
        Lamp::Idle | Lamp::Busy => theme().muted_foreground,
        Lamp::Good | Lamp::Degraded | Lamp::Failed => fill(),
    };
    let opacity = pulse(move || lamp() == Lamp::Busy);
    view()
        .size_2()
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .border_color(ring)
        .bg(fill)
        .bind_opacity(&opacity)
}

/// The lamp `lamp` beside its word `label`.
///
/// ```ignore
/// status_lamp(move || if store.running() { Lamp::Good } else { Lamp::Idle }, store.tr(|s| s.state_running))
/// ```
pub fn status_lamp(
    lamp: impl Fn() -> Lamp + Copy + 'static,
    label: impl IntoReactive<SharedString>,
) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_1p5()
        .child(lamp_dot(lamp))
        .child(
            text(label)
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().muted_foreground),
        )
}
