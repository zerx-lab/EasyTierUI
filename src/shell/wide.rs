//! Wide windows (desktops and the web): the sidebar beside the selected page, each owning its scrolling. On the
//! desktops the sidebar's top and the page header are the title bar: they move the window and keep clear of the
//! native window controls. The sidebar toggle stays at one place after the window controls whether the sidebar
//! shows or not; hiding the sidebar slides it shut and the page widens into its room, in one native layout
//! animation.

use nsh_base::theme;
use nsh_core::{
    element::{LayoutAnimation, LayoutCurve, LayoutTransition, desktop::title_bar},
    prelude::*,
};

use super::{nav, page_body};
use crate::{
    components::{BarEdges, CONTROLS_GAP, bar_height, drag_region, sidebar_toggle},
    pages,
    store::{Page, Store},
};

/// The sidebar's width (DESIGN.md: about 240 pt, `w_60`), animated to and from zero.
const SIDEBAR_WIDTH: Points = Points(240.0);
/// The toggle's width (32 pt, `size_8`) and the gap after it (spacing step 3), which the page title keeps clear of
/// while the sidebar is hidden.
const TOGGLE_ROOM: f32 = 44.0;
/// How long the sidebar takes to open or close: a critically damped spring, quick and without overshoot.
const SLIDE_MS: f32 = 280.0;

/// The wide layout.
pub fn wide(store: Store) -> View {
    view()
        .flex_1()
        .flex_row()
        .child(sidebar(store))
        .child(
            view()
                .flex_1()
                .child(page_header(store))
                .child(dynamic(move || {
                    let page = store.page();
                    untracked(move || page_body(store, page))
                })),
        )
        .child(toggle(store))
}

/// Shows or hides the sidebar, animated unless the user asked for less motion.
fn toggle_sidebar(store: Store) {
    if !reduce_motion() {
        animate_layout(LayoutAnimation::all(LayoutTransition::new(
            LayoutCurve::Spring { damping: 1.0 },
            SLIDE_MS,
        )));
    }
    store.toggle_sidebar();
}

/// The sidebar toggle, after the window controls at the title bar's leading edge.
fn toggle(store: Store) -> View {
    let label = move || {
        let strings = store.strings();
        if store.sidebar_open() {
            strings.sidebar_hide
        } else {
            strings.sidebar_show
        }
    };
    view()
        .absolute()
        .top_0()
        .left(move || Points(title_bar().left.0 + CONTROLS_GAP))
        .h(bar_height)
        .justify_center()
        .child(sidebar_toggle(label, move || toggle_sidebar(store)))
}

/// The sidebar: its outer box animates its width and clips, its content keeps its own width so nothing reflows
/// while it slides.
fn sidebar(store: Store) -> View {
    let content = view()
        .w(SIDEBAR_WIDTH)
        .flex_1()
        .bg(move || theme().sidebar)
        .border_r_1()
        .border_color(move || theme().sidebar_border)
        // The top row is the title bar: the window controls and the toggle are drawn over it.
        .child(drag_region(BarEdges {
            leading: true,
            trailing: false,
        }))
        .child(view().px_2().pb_2().child(nav::target_switcher(store)))
        .child(
            scroll()
                .flex_1()
                .child(view().px_2().py_2().child(nav::role_sections(store, true))),
        )
        .child(
            view()
                .px_2()
                .py_2()
                .border_t_1()
                .border_color(move || theme().sidebar_border)
                .child(nav::page_item(store, Page::Settings, true)),
        );
    view()
        .w(move || {
            if store.sidebar_open() {
                SIDEBAR_WIDTH
            } else {
                Points(0.0)
            }
        })
        .flex_shrink_0()
        .overflow_hidden()
        .child(content)
}

/// The selected page's title, in the title bar's row; while the sidebar is hidden it starts after the window
/// controls and the toggle.
fn page_header(store: Store) -> View {
    view()
        .window_drag(true)
        .flex_row()
        .items_center()
        .min_h(bar_height)
        .pl(move || {
            if store.sidebar_open() {
                Points(CONTROLS_GAP * 2.0)
            } else {
                Points(title_bar().left.0 + CONTROLS_GAP + TOGGLE_ROOM)
            }
        })
        .pr(move || Points(title_bar().right.0 + CONTROLS_GAP))
        .border_b_1()
        .border_color(move || theme().border)
        .child(
            text(move || store.page().title(store.strings()))
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .number_of_lines(1)
                .role(AccessibilityRole::Header)
                .text_color(move || theme().foreground),
        )
        .child(view().flex_1())
        // The page's own actions at the trailing edge; the header moves the window, its buttons still press.
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(dynamic(move || {
                    let page = store.page();
                    untracked(move || pages::header_actions(store, page))
                })),
        )
}
