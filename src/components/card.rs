//! Panels (DESIGN.md "Elevation & Depth", "Network Card"): cards and grouped lists. Flat at rest: one layer step and a
//! hairline, never a shadow.

use nsh_base::theme;
use nsh_core::prelude::*;

/// A card: panel fill, 1 pt hairline, container radius, 16 pt padding, 12 pt between its children, no shadow.
///
/// ```ignore
/// card().child(text(name)).child(machine_text(ip))
/// ```
pub fn card() -> View {
    view()
        .p_4()
        .gap_3()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
}

/// A grouped list: panel fill, hairline, container radius, clipping its rows to the corners. Fill it with
/// [`group_row`]s.
///
/// ```ignore
/// group().child(group_row(false).child(text("A"))).child(group_row(true).child(text("B")))
/// ```
pub fn group() -> View {
    view()
        .overflow_hidden()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
}

/// A row of a [`group`]: 16 pt gutters, a 36 pt (44 pt on touch screens) minimum, laid out as a row. Rows after the
/// first pass `separated = true` for the hairline above them.
pub fn group_row(separated: bool) -> View {
    let row = view()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_2()
        .when(separated, |row| {
            row.border_t_1().border_color(move || theme().border)
        });
    if is_desktop() {
        row.min_h_9()
    } else {
        row.min_h_11()
    }
}
