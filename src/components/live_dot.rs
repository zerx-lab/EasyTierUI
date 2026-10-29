//! The live indicator (DESIGN.md "Live indicator"): an 8 pt dot that pulses slowly while the page polls a running
//! instance. Always placed beside words ("Updated 3 s ago"), never alone.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{motion::pulse, text::app_color};

/// The dot: `success` and pulsing while `live()` holds (the pulse stops under Reduce Motion), a hollow
/// `muted-foreground` ring when nothing is polling. It is decoration for screen readers; the words beside it carry
/// the meaning.
///
/// ```ignore
/// view().flex_row().items_center().gap_2()
///     .child(live_dot(move || store.polling()))
///     .child(age_text(updated, template, just_now))
/// ```
pub fn live_dot(live: impl Fn() -> bool + Copy + 'static) -> View {
    let opacity = pulse(live);
    view()
        .size_2()
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .accessible(false)
        .border_color(move || {
            if live() {
                app_color("success")
            } else {
                theme().muted_foreground
            }
        })
        .bg(move || {
            if live() {
                app_color("success")
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .bind_opacity(&opacity)
}
