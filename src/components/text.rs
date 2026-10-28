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
