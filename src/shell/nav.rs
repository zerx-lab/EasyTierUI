//! Navigation shared by both layouts: the target switcher and the role sections with their pages.

use nsh_base::{menu, menu_item, theme};
use nsh_core::prelude::*;

use crate::{
    components::{Lamp, control_radius, nav_item, section_label, status_lamp},
    core::{
        role::Role,
        target::{Target, TargetOrigin, TargetStatus},
    },
    i18n::{Pick, Strings},
    store::{Page, Store},
};

/// A target's name: the one the user gave it, else what it is.
fn target_name(target: &Target, strings: &'static Strings) -> SharedString {
    target.name.clone().unwrap_or_else(|| {
        match target.origin {
            TargetOrigin::Local => strings.target_this_device,
            TargetOrigin::SameOrigin => strings.target_this_server,
        }
        .into()
    })
}

/// A target's lamp and its word.
fn target_lamp(status: TargetStatus, strings: &'static Strings) -> (Lamp, &'static str) {
    match status {
        TargetStatus::Stopped => (Lamp::Idle, strings.status_stopped),
        TargetStatus::NotConnected => (Lamp::Idle, strings.status_not_connected),
    }
}

/// The Target Switcher (DESIGN.md signature): the selected target's name and lamp; opening it lists every target.
pub fn target_switcher(store: Store) -> impl IntoElement {
    let name = move || {
        store
            .current_target()
            .map(|target| target_name(&target, store.strings()))
            .unwrap_or_default()
    };
    let status = move || {
        store
            .current_target()
            .map_or(TargetStatus::NotConnected, |target| target.status)
    };
    let trigger = view()
        .flex_row()
        .items_center()
        .gap_2()
        .min_h_9()
        .px_2p5()
        .py_1()
        .rounded(control_radius)
        .hover(|s| s.bg(move || theme().sidebar_accent))
        .child(
            view()
                .flex_1()
                .gap_0p5()
                .child(
                    text(name)
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .number_of_lines(1)
                        .text_color(move || theme().sidebar_foreground),
                )
                .child(status_lamp(
                    move || target_lamp(status(), store.strings()).0,
                    move || target_lamp(status(), store.strings()).1,
                )),
        );
    menu()
        .popup_class("min-w-56 rounded-(--radius) border border-border bg-popover p-1 shadow-md")
        .trigger(trigger)
        .child(each(
            move || store.targets(),
            |target: &Target| target.id,
            move |target, _| target_item(store, target),
        ))
}

/// A target in the switcher's menu: choosing it selects it.
fn target_item(store: Store, target: ReadSignal<Target>) -> impl IntoElement {
    let id = target.get_untracked().id;
    let status = move || target.with(|t| t.status);
    menu_item()
        .on_select(move || store.select_target(id))
        .flex_row()
        .items_center()
        .gap_3()
        .px_2p5()
        .py_1p5()
        .min_h_8()
        .rounded(control_radius)
        .hover(|s| s.bg(move || theme().accent))
        .child(
            text(move || target.with(|t| target_name(t, store.strings())))
                .text_sm()
                .flex_1()
                .text_color(move || theme().popover_foreground),
        )
        .child(status_lamp(
            move || target_lamp(status(), store.strings()).0,
            move || target_lamp(status(), store.strings()).1,
        ))
}

/// Every role's section: its label and its pages. `highlight` marks the selected page (the sidebar does; a
/// phone's home list does not).
pub fn role_sections(store: Store, highlight: bool) -> View {
    let sections: Vec<_> = Role::ALL
        .into_iter()
        .map(|role| role_section(store, role, highlight))
        .collect();
    view().gap_4().child(sections)
}

fn role_section(store: Store, role: Role, highlight: bool) -> View {
    let label: Pick = match role {
        Role::Node => |s| s.role_node,
        Role::Console => |s| s.role_console,
        Role::Host => |s| s.role_host,
    };
    let items: Vec<_> = Page::of_role(role)
        .iter()
        .map(|&page| page_item(store, page, highlight))
        .collect();
    view()
        .gap_0p5()
        .child(view().px_2p5().pb_1().child(section_label(store.tr(label))))
        .child(items)
}

/// The navigation row of `page`.
pub fn page_item(store: Store, page: Page, highlight: bool) -> impl IntoElement {
    nav_item(
        move || page.title(store.strings()),
        &format!("page-{}", page.key()),
        move || highlight && store.page() == page,
        move || store.open(page),
    )
}
