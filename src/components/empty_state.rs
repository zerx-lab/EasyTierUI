//! Empty states: what is missing and what will show here, centered with 32 pt around it.

use nsh_base::theme;
use nsh_core::prelude::*;

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
