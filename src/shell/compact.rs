//! Phones and narrow windows: a navigation stack with a home page (the target switcher and every role's pages) and
//! the opened page above it. iOS and Android show the system header and back gesture; the desktops and the web show
//! the app's header, under the custom title bar on the desktops.

use nsh_base::{nav_page, nav_stack, theme};
use nsh_core::prelude::*;

use super::{nav, page_body, window_bar};
use crate::{
    components::{ButtonKind, action_button},
    store::{Page, Store},
};

/// The compact layout.
pub fn compact(store: Store) -> View {
    let platform = store.platform();
    let home = nav_page()
        .title(store.tr(|s| s.app_name))
        .when(!platform.is_desktop(), |page| {
            page.header(header(store, None))
        })
        .child(scroll().flex_1().child(home(store)));
    view()
        .flex_1()
        .when(platform.is_desktop(), |shell| shell.child(window_bar()))
        .child(nav_stack().flex_1().child(home).child(each(
            move || store.stack(),
            |page: &Page| *page,
            move |page, depth| {
                let page = page.get_untracked();
                nav_page()
                    .title(move || page.title(store.strings()))
                    .header(header(store, Some(page)))
                    .on_dismiss(move || store.pop_to(depth.get_untracked()))
                    .child(page_body(store, page))
            },
        )))
}

/// The home page: the target switcher, then every role's pages and the settings.
fn home(store: Store) -> View {
    view()
        .p_4()
        .gap_6()
        .child(nav::target_switcher(store))
        .child(nav::role_sections(store, false))
        .child(nav::page_item(store, Page::Settings, false))
}

/// The desktops' and the web's header of a page: a back button over the home page, and the title.
fn header(store: Store, page: Option<Page>) -> View {
    let title = move || match page {
        Some(page) => page.title(store.strings()),
        None => store.strings().app_name,
    };
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .px_4()
        .py_2()
        .min_h_11()
        .border_b_1()
        .border_color(move || theme().border)
        .when(page.is_some(), |row| {
            row.child(action_button(
                ButtonKind::Ghost,
                store.tr(|s| s.back),
                move || store.pop_to(0),
            ))
        })
        .child(
            text(title)
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .role(AccessibilityRole::Header)
                .text_color(move || theme().foreground),
        )
}
