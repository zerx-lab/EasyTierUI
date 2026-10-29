//! The search field (DESIGN.md "Layout", toolbar): an input with a search icon at its leading edge and, while it holds
//! text, a clear button at the trailing edge. Filtering is the caller's; the field only reports edits.

use std::rc::Rc;

use nsh_core::prelude::*;

use super::{
    data::read,
    field::styled_input,
    icon::{ICON_SIZE, Icon, icon},
};

/// The field's width in points on the desktops (a toolbar leaves it room beside the filters).
const WIDTH: f32 = 260.0;

/// Room kept inside the input for the icon at its leading edge, in points.
const ICON_ROOM: f32 = 32.0;

/// A search field under construction.
pub struct SearchField {
    placeholder: Reactive<SharedString>,
    value: Rc<dyn Fn() -> String>,
    on_change: Rc<dyn Fn(&str)>,
    clear_label: Option<Reactive<SharedString>>,
    test_id: Option<SharedString>,
}

/// A search field showing `value()` and reporting edits to `on_change`; `placeholder` says what is searched
/// ("Search by name or IP"), and is the field's accessible name.
///
/// ```ignore
/// search_field(store.tr(|s| s.search_peers), move || store.query(), move |q| store.set_query(q))
///     .clear_label(store.tr(|s| s.action_clear))
///     .test_id("peer-search")
/// ```
pub fn search_field(
    placeholder: impl IntoReactive<SharedString>,
    value: impl Fn() -> String + 'static,
    on_change: impl Fn(&str) + 'static,
) -> SearchField {
    SearchField {
        placeholder: placeholder.into_reactive(),
        value: Rc::new(value),
        on_change: Rc::new(on_change),
        clear_label: None,
        test_id: None,
    }
}

impl SearchField {
    /// The accessible name of the clear button; without it the field offers no clear button.
    #[must_use]
    pub fn clear_label(mut self, label: impl IntoReactive<SharedString>) -> Self {
        self.clear_label = Some(label.into_reactive());
        self
    }

    /// The hook tests find the input by (`id:<test_id>`); the clear button is `<test_id>-clear`.
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view.
    pub fn build(self) -> View {
        let Self {
            placeholder,
            value,
            on_change,
            clear_label,
            test_id,
        } = self;
        let shown = value.clone();
        let hint = placeholder.clone();
        let mut input = styled_input(|| false, false)
            .label(placeholder)
            .pl(Points(ICON_ROOM))
            .pr(Points(ICON_ROOM))
            .value(move || shown())
            .placeholder(move || read(&hint).to_string())
            .on_change({
                let on_change = on_change.clone();
                move |text: &str| on_change(text)
            });
        if let Some(id) = test_id.clone() {
            input = input.test_id(&id);
        }
        let magnifier = view()
            .absolute()
            .top_0()
            .bottom_0()
            .left_2p5()
            .justify_center()
            .pointer_events(PointerEvents::None)
            .child(icon(Icon::Search, ICON_SIZE));
        let has_text = value.clone();
        let clear = clear_label.map(|label| {
            let clear_id = test_id.as_ref().map(|id| format!("{id}-clear"));
            when(
                move || !has_text().is_empty(),
                move || {
                    let mut button = view()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_1()
                        .justify_center()
                        .child(
                            view()
                                .role(AccessibilityRole::Button)
                                .label(label.clone())
                                .p_1p5()
                                .rounded_full()
                                .hover(|s| s.bg(move || nsh_base::theme().accent))
                                .on_press({
                                    let on_change = on_change.clone();
                                    move || on_change("")
                                })
                                .child(icon(Icon::Close, 12.0)),
                        );
                    if let Some(id) = clear_id.clone() {
                        button = button.test_id(&id);
                    }
                    button
                },
            )
        });
        let row = view().min_w(Points(WIDTH)).child(input).child(magnifier);
        let row = if is_desktop() {
            row.w(Points(WIDTH))
        } else {
            row.flex_1()
        };
        row.child(clear)
    }
}

impl IntoElement for SearchField {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}
