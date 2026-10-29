//! Empty states: what is missing and what will show here, centered with 32 pt around it.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::icon::{ICON_SIZE_EMPTY, Icon, icon};

/// The empty state `title` with its explanation `body`.
pub fn empty_state(
    title: impl IntoReactive<SharedString>,
    body: impl IntoReactive<SharedString>,
) -> View {
    view()
        .items_center()
        .gap_2()
        .py_8()
        .px_4()
        .child(
            text(title)
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .text_center()
                .text_color(move || theme().foreground),
        )
        .child(
            text(body)
                .text_sm()
                .max_w_96()
                .text_center()
                .text_color(move || theme().muted_foreground),
        )
}

/// The empty state of a table or panel (DESIGN.md "Empty states"): a 32 pt `icon`, the `title`, one sentence and,
/// when there is one, the primary `action` (a button) under them. No illustration.
///
/// ```ignore
/// empty_panel(Icon::Network, store.tr(|s| s.networks_empty_title), store.tr(|s| s.networks_empty_body),
///     Some(action_button(ButtonKind::Primary, create, move || store.create()).into_element()))
/// ```
pub fn empty_panel(
    icon_name: Icon,
    title: impl IntoReactive<SharedString>,
    body: impl IntoReactive<SharedString>,
    action: Option<AnyElement>,
) -> View {
    view()
        .items_center()
        .gap_3()
        .py_8()
        .px_4()
        .child(icon(icon_name, ICON_SIZE_EMPTY))
        .child(
            view()
                .items_center()
                .gap_1()
                .child(
                    text(title)
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_center()
                        .text_color(move || theme().foreground),
                )
                .child(
                    text(body)
                        .text_sm()
                        .max_w_96()
                        .text_center()
                        .text_color(move || theme().muted_foreground),
                ),
        )
        .child(action)
}
