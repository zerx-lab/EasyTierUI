//! Typography and token helpers shared by the components.

use nsh_base::theme;
use nsh_core::prelude::*;

/// The app's own color token `name` (`success`, `warning-muted`, …) from `theme.json`. Every name the app uses is
/// defined in both appearances there; a theme without it draws in `foreground`, so the element stays visible.
pub fn app_color(name: &str) -> AdaptiveColor {
    let theme = theme();
    theme.color(name).unwrap_or(theme.foreground)
}

/// The control radius (DESIGN.md "Two-Tier Radius Rule": 6 pt), theme token `radius-control`; a theme without it
/// uses the container radius.
pub fn control_radius() -> Points {
    let theme = theme();
    theme.number("radius-control").map_or(theme.radius, Points)
}

/// A section label (DESIGN.md "Quiet Section Rule"): Label type, sentence case, `muted-foreground`.
pub fn section_label(label: impl IntoReactive<SharedString>) -> Text {
    text(label)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(move || theme().muted_foreground)
}

/// Machine data (DESIGN.md "Machine Voice Rule": versions, addresses, latency): the system monospace with
/// tabular figures, never mixed with people's text in one run.
pub fn machine_text(content: impl IntoReactive<SharedString>) -> Text {
    text(content)
        .text_sm()
        .font_family("monospace")
        .tabular_nums()
}

/// `element` carrying the test id `id`. nsh-base's components (`Button`, `Switch`, `Tab`, `MenuItem`) take no test id
/// of their own, so a plain view around them carries it; without an id the element is returned as it is.
pub(super) fn tagged(element: impl IntoElement, id: Option<&str>) -> AnyElement {
    match id {
        Some(id) => view().test_id(id).child(element).into_element(),
        None => element.into_element(),
    }
}

/// Body type (DESIGN.md "Typography"): 14/20 on the desktops, 16/24 (Body Touch) on touch screens, `foreground`.
pub fn body_text(content: impl IntoReactive<SharedString>) -> Text {
    let body = text(content).text_color(move || theme().foreground);
    if is_desktop() {
        body.text_sm()
    } else {
        body.text_base()
    }
}

/// Meta type (12/16, `muted-foreground`): timestamps, helper text, versions in footers.
pub fn meta_text(content: impl IntoReactive<SharedString>) -> Text {
    text(content)
        .text_xs()
        .text_color(move || theme().muted_foreground)
}
