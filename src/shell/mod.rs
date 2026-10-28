//! The app shell: the custom desktop title bar, then a sidebar beside the selected page on wide windows, or a
//! navigation stack on phones and narrow windows (DESIGN.md "Layout"). Page logic stays in `pages`.

mod compact;
mod nav;
mod wide;

use nsh_base::theme;
use nsh_core::{desktop::TitleBarStyle, element::desktop::set_title_bar, prelude::*};

use crate::{
    components::{BarEdges, TITLE_BAR_HEIGHT, drag_region},
    pages,
    store::{Page, Store},
};

/// Below this width a window collapses to one column (DESIGN.md: "narrower than about 640 pt").
const COMPACT_WIDTH: Points = Points(640.0);

/// The whole surface for `store`.
pub fn shell(store: Store) -> AnyElement {
    if store.platform().is_desktop() {
        // The app draws the title bar; the native window controls stay where `title_bar()` reports them.
        set_title_bar(TitleBarStyle::Hidden, Some(TITLE_BAR_HEIGHT));
    }
    let compact = move || store.platform().is_mobile() || window().width < COMPACT_WIDTH;
    when(compact, move || compact::compact(store))
        .otherwise(move || wide::wide(store))
        .into_element()
}

/// A full-width title bar (the desktops' compact layout): room for the window controls, a drag region.
fn window_bar() -> View {
    drag_region(BarEdges {
        leading: true,
        trailing: true,
    })
    .border_b_1()
    .border_color(move || theme().border)
}

/// The scrolling body of `page`, padded as a page.
fn page_body(store: Store, page: Page) -> Scroll {
    scroll()
        .flex_1()
        .child(view().p_6().gap_6().child(pages::content(store, page)))
}
