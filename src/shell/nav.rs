//! Navigation shared by both layouts: the target switcher and the role sections with their pages.

use nsh_base::{menu, menu_item, menu_separator, theme};
use nsh_core::prelude::*;

use crate::{
    components::{
        ICON_SIZE, Icon, Lamp, control_radius, icon, kind_chip, nav_item, section_label,
        status_lamp,
    },
    core::{
        role::Role,
        target::{Target, TargetKind, TargetOrigin, TargetStatus},
    },
    i18n::{Pick, Strings},
    store::{Page, Store},
};

/// A target's name: the one the user gave it, else what it is (a server's host).
fn target_name(target: &Target, strings: &'static Strings) -> SharedString {
    target.name.clone().unwrap_or_else(|| match &target.origin {
        TargetOrigin::Local => strings.target_this_device.into(),
        TargetOrigin::SameOrigin => strings.target_this_server.into(),
        TargetOrigin::Loopback { .. } => strings.target_hosted.into(),
        TargetOrigin::Url(url) => url::Url::parse(url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_else(|| url.clone())
            .into(),
    })
}

/// A target's lamp and its word: online is "Running" for this device and the hosted control plane (processes here),
/// "Signed in" for a server.
fn target_lamp(
    (status, kind): (TargetStatus, TargetKind),
    strings: &'static Strings,
) -> (Lamp, &'static str) {
    match status {
        TargetStatus::Stopped => (Lamp::Idle, strings.status_stopped),
        TargetStatus::NotConnected => (Lamp::Idle, strings.status_not_connected),
        TargetStatus::Connecting => (Lamp::Busy, strings.common.state_connecting),
        TargetStatus::Online => match kind {
            TargetKind::Server => (Lamp::Good, strings.common.state_online),
            TargetKind::ThisDevice | TargetKind::Hosted => {
                (Lamp::Good, strings.common.state_running)
            }
        },
        TargetStatus::Failed => (Lamp::Failed, strings.common.state_failed),
    }
}

/// The kind chip's word for a target of `kind`.
fn kind_word(kind: TargetKind, strings: &'static Strings) -> &'static str {
    match kind {
        TargetKind::ThisDevice => strings.kind_this_device,
        TargetKind::Server => strings.kind_server,
        TargetKind::Hosted => strings.kind_hosted,
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
            .map_or((TargetStatus::NotConnected, TargetKind::Server), |target| {
                (target.status, target.kind)
            })
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
                .min_w_0()
                .gap_0p5()
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            text(name)
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .number_of_lines(1)
                                .min_w_0()
                                .flex_shrink(1.0)
                                .text_color(move || theme().sidebar_foreground),
                        )
                        .child(when(
                            move || name().as_ref() != kind_word(status().1, store.strings()),
                            move || kind_chip(move || kind_word(status().1, store.strings())),
                        )),
                )
                .child(status_lamp(
                    move || target_lamp(status(), store.strings()).0,
                    move || target_lamp(status(), store.strings()).1,
                )),
        )
        .child(icon(Icon::ChevronDown, ICON_SIZE));
    let add_server = menu_item()
        .on_select(move || store.console().request_add_server())
        .px_2p5()
        .py_1p5()
        .min_h_8()
        .rounded(control_radius)
        .hover(|s| s.bg(move || theme().accent))
        .child(
            text(store.tr(|s| s.target_add_server))
                .text_sm()
                .text_color(move || theme().popover_foreground),
        );
    let host_here = store.capabilities().host.is_local().then(|| {
        menu_item()
            .on_select(move || store.open(Page::HostStatus))
            .px_2p5()
            .py_1p5()
            .min_h_8()
            .rounded(control_radius)
            .hover(|s| s.bg(move || theme().accent))
            .child(
                text(store.tr(|s| s.target_host_here))
                    .text_sm()
                    .text_color(move || theme().popover_foreground),
            )
    });
    menu()
        .popup_class("min-w-56 rounded-(--radius) border border-border bg-popover p-1 shadow-md")
        .trigger(trigger)
        .child(each(
            move || store.targets(),
            |target: &Target| target.id,
            move |target, _| target_item(store, target),
        ))
        .child(
            menu_separator()
                .my_1()
                .border_t_1()
                .border_color(move || theme().border),
        )
        .child(add_server)
        .child(host_here)
}

/// A target in the switcher's menu: choosing it selects it.
fn target_item(store: Store, target: ReadSignal<Target>) -> impl IntoElement {
    let id = target.get_untracked().id;
    let status = move || target.with(|t| (t.status, t.kind));
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
                .number_of_lines(1)
                .flex_1()
                .text_color(move || theme().popover_foreground),
        )
        .child(when(
            move || {
                target.with(|t| {
                    target_name(t, store.strings()).as_ref() != kind_word(t.kind, store.strings())
                })
            },
            move || kind_chip(move || kind_word(status().1, store.strings())),
        ))
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
        page_icon(page),
        move || page.title(store.strings()),
        &format!("page-{}", page.key()),
        move || highlight && store.page() == page,
        move || store.open(page),
    )
}

/// The icon of `page` in the sidebar and the home list.
fn page_icon(page: Page) -> Icon {
    match page {
        Page::Networks => Icon::Network,
        Page::Peers => Icon::Peers,
        Page::Machines => Icon::Server,
        Page::ServerNetworks => Icon::Globe,
        Page::Credentials => Icon::Key,
        Page::HostStatus => Icon::Activity,
        Page::Hardening => Icon::Shield,
        Page::Deployment => Icon::Rocket,
        Page::Settings => Icon::Settings,
    }
}
