//! Chips (DESIGN.md "Chips"): 20 pt pills for route type, NAT type, protocol, versions and counts.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    lamp::{Lamp, lamp_dot},
    text::app_color,
};

/// A status chip: a 20 pt pill with a 10 % tint of the lamp's hue, a lamp and the word `label` in `foreground`
/// (the word carries the state; the tint and lamp only repeat it). `Idle` and `Busy` use the well fill with a hollow
/// ring.
///
/// ```ignore
/// status_chip(move || if peer.direct { Lamp::Good } else { Lamp::Degraded }, store.tr(|s| s.route_p2p))
/// ```
pub fn status_chip(
    lamp: impl Fn() -> Lamp + Copy + 'static,
    label: impl IntoReactive<SharedString>,
) -> View {
    let tint = move || match lamp() {
        Lamp::Good => app_color("success-muted"),
        Lamp::Degraded => app_color("warning-muted"),
        Lamp::Failed => app_color("destructive-muted"),
        Lamp::Idle | Lamp::Busy => theme().muted,
    };
    view()
        .flex_row()
        .items_center()
        .flex_shrink_0()
        .gap_1p5()
        .px_2()
        .min_h_5()
        .rounded_full()
        .bg(tint)
        .child(lamp_dot(lamp))
        .child(chip_label(label, move || theme().foreground))
}

/// A neutral chip: well fill and a `muted-foreground` label, for tags, versions and counts. Most chips are neutral.
///
/// ```ignore
/// neutral_chip(store.tr(|s| s.target_this_device))
/// ```
pub fn neutral_chip(label: impl IntoReactive<SharedString>) -> View {
    view()
        .flex_row()
        .items_center()
        .flex_shrink_0()
        .px_2()
        .min_h_5()
        .rounded_full()
        .bg(move || theme().muted)
        .child(chip_label(label, move || theme().muted_foreground))
}

/// A kind chip: the neutral chip that names what an entity is ("Network", "Machine", "This device"), set beside its
/// name in a drawer header or the target switcher. The name is the entity's; the chip is a tag on it and never
/// carries state.
///
/// ```ignore
/// kind_chip(store.tr(|s| s.kind_network))
/// ```
pub fn kind_chip(label: impl IntoReactive<SharedString>) -> View {
    neutral_chip(label)
}

/// The chip's text in Label type.
fn chip_label(
    label: impl IntoReactive<SharedString>,
    color: impl Fn() -> AdaptiveColor + 'static,
) -> Text {
    text(label)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
}
