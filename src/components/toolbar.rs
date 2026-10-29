//! The page toolbar (DESIGN.md "Layout"): the row under the title bar that holds a page's context on the left (search,
//! filters, group-by) and its actions on the right, with a hairline under it.

use nsh_base::theme;
use nsh_core::prelude::*;

/// The gutter around a page's content in points (spacing step 6): the shell pads every page by it, and a toolbar
/// cancels it to reach the page's edges.
pub const PAGE_GUTTER: f32 = 24.0;

/// A toolbar: a wrapping row with 12 pt between its children and a hairline under it. Make it the first child of a
/// page: it cancels the shell's 24 pt page gutter above and at the sides so its hairline spans the page, and the page's
/// own gap separates it from the content. Put context first, then [`toolbar_spacer`], then the actions with the
/// primary one last.
///
/// ```ignore
/// toolbar()
///     .child(search_field(placeholder, move || store.query(), move |q| store.set_query(q)))
///     .child(toolbar_spacer())
///     .child(action_button(ButtonKind::Primary, store.tr(|s| s.action_create), move || store.create()))
/// ```
pub fn toolbar() -> View {
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x_3()
        .gap_y_2()
        .px_6()
        .py_2()
        .mx(Points(-PAGE_GUTTER))
        .mt(Points(-PAGE_GUTTER))
        .border_b_1()
        .border_color(move || theme().border)
}

/// The flexible gap that pushes what follows to the toolbar's trailing edge.
///
/// ```ignore
/// toolbar().child(left).child(toolbar_spacer()).child(right)
/// ```
pub fn toolbar_spacer() -> View {
    view().flex_1()
}
