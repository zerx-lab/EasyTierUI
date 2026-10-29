//! The threshold bar (DESIGN.md "Threshold bar"): a thin bar beside its number for latency and loss, `success` below
//! the first threshold, `warning` above it, `destructive` only past the second one (loss over 10 %, unreachable). The
//! number stays readable, so the color only repeats it.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::text::{app_color, machine_text};

/// The bar's height in points: a physical size, thinner than any spacing step.
const BAR_HEIGHT: f32 = 4.0;

/// The narrowest the number beside a bar gets in points, so the bars of a column start at one place.
const NUMBER_MIN_WIDTH: f32 = 48.0;

/// The narrowest a bar gets in points, so a tight column still shows a readable length.
const BAR_MIN_WIDTH: f32 = 64.0;

/// Where a metric turns amber and red, and what a full bar means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    /// At or above this the bar is `warning`.
    pub warn: f64,
    /// At or above this the bar is `destructive`; `None` for a metric that never turns red (latency).
    pub bad: Option<f64>,
    /// The value that fills the bar.
    pub full: f64,
}

impl Thresholds {
    /// The level of `value`: 0 good, 1 attention, 2 failing.
    fn level(&self, value: f64) -> u8 {
        if self.bad.is_some_and(|bad| value >= bad) {
            2
        } else if value >= self.warn {
            1
        } else {
            0
        }
    }

    /// How much of the bar `value` fills, 0 to 1.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a fraction clamped to 0..=1 fits an f32"
    )]
    fn fraction(&self, value: f64) -> f32 {
        if self.full <= 0.0 || !value.is_finite() {
            return 0.0;
        }
        (value / self.full).clamp(0.0, 1.0) as f32
    }
}

/// A bar for `value()` (`None`: no data, shown as a muted dash and no bar) next to `label` (the formatted number in
/// Mono, never wrapping), colored by `thresholds`. Fill the column's header with a tooltip that explains the
/// thresholds.
///
/// ```ignore
/// threshold_bar(
///     move || peer.latency_ms,
///     move || format_latency(peer.latency_ms),
///     Thresholds { warn: 100.0, bad: None, full: 300.0 },
/// )
/// ```
pub fn threshold_bar(
    value: impl Fn() -> Option<f64> + Copy + 'static,
    label: impl IntoReactive<SharedString>,
    thresholds: Thresholds,
) -> View {
    let hue = move || match value().map(|v| thresholds.level(v)) {
        Some(2) => theme().destructive,
        Some(1) => app_color("warning"),
        Some(_) => app_color("success"),
        None => theme().muted_foreground,
    };
    let bar = move || {
        view()
            .flex_1()
            .min_w(Points(BAR_MIN_WIDTH))
            .h(Points(BAR_HEIGHT))
            .rounded_full()
            .bg(move || theme().muted)
            .accessible(false)
            .child(
                view()
                    .h_full()
                    .rounded_full()
                    .bg(hue)
                    .w(move || relative(value().map_or(0.0, |v| thresholds.fraction(v)))),
            )
    };
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(when(move || value().is_some(), bar))
        .child(
            machine_text(label)
                .number_of_lines(1)
                .text_right()
                .min_w(Points(NUMBER_MIN_WIDTH)),
        )
}
