//! The app shell: the custom desktop title bar, then a sidebar beside the selected page on wide windows, or a
//! navigation stack on phones and narrow windows (DESIGN.md "Layout"). Until the user picks a work mode, the first
//! run's question takes the whole window. Page logic stays in `pages`.

mod compact;
mod nav;
mod wide;

use nsh_base::{nav_page, nav_stack, theme};
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
    let layout =
        move || when(compact, move || compact::compact(store)).otherwise(move || wide::wide(store));
    view()
        .flex_1()
        .child(when(move || store.mode().is_some(), layout).otherwise(move || first_run(store)))
        .child(pages::overlays(store))
        .into_element()
}

/// The first run's work-mode question: under the system header on phones, under the app's title bar on the
/// desktops, as a plain page on the web.
fn first_run(store: Store) -> View {
    let platform = store.platform();
    let body = move || scroll().flex_1().child(pages::welcome(store));
    if platform.is_mobile() {
        return view().flex_1().child(
            nav_stack()
                .flex_1()
                .child(nav_page().title(store.tr(|s| s.app_name)).child(body())),
        );
    }
    view()
        .flex_1()
        .when(platform.is_desktop(), |shell| shell.child(window_bar()))
        .child(body())
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

/// The scrolling body of `page`, padded as a page: a 24 pt gutter all round (`components::PAGE_GUTTER`, which a page's
/// `toolbar()` cancels to span the page) and 24 pt between sections.
fn page_body(store: Store, page: Page) -> Scroll {
    scroll()
        .flex_1()
        .child(view().p_6().gap_6().child(pages::content(store, page)))
}
