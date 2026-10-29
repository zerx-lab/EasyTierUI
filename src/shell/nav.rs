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

/// The Target Switcher (DESIGN.md signature): the selected target's name and lamp; opening it lists every target the
/// work mode serves. A node alone has one target, so there it is a plain row without a menu.
pub fn target_switcher(store: Store) -> impl IntoElement {
    let switches = move || {
        let roles = store.roles();
        roles.contains(Role::Console) || roles.contains(Role::Host)
    };
    when(switches, move || switcher_menu(store)).otherwise(move || switcher_face(store, false))
}

/// The selected target's name, kind and lamp; `opens_menu` adds the hover and the chevron of a menu's trigger.
fn switcher_face(store: Store, opens_menu: bool) -> View {
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
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .min_h_9()
        .px_2p5()
        .py_1()
        .rounded(control_radius)
        .when(opens_menu, |face| {
            face.hover(|s| s.bg(move || theme().sidebar_accent))
        })
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
        .when(opens_menu, |face| {
            face.child(icon(Icon::ChevronDown, ICON_SIZE))
        })
}

/// The switcher's menu: the listed targets, then adding a server and hosting here where the mode shows them.
fn switcher_menu(store: Store) -> impl IntoElement {
    let hosts_here = store.capabilities().host.is_local();
    let adds_server = move || store.roles().contains(Role::Console);
    let hosts = move || hosts_here && store.roles().contains(Role::Host);
    menu()
        .popup_class("min-w-56 rounded-(--radius) border border-border bg-popover p-1 shadow-md")
        .trigger(switcher_face(store, true))
        .child(each(
            move || store.listed_targets(),
            |target: &Target| target.id,
            move |target, _| target_item(store, target),
        ))
        .child(when(
            move || adds_server() || hosts(),
            move || {
                menu_separator()
                    .my_1()
                    .border_t_1()
                    .border_color(move || theme().border)
            },
        ))
        .child(when(adds_server, move || {
            menu_action(store.tr(|s| s.target_add_server), move || {
                store.console().request_add_server();
            })
        }))
        .child(when(hosts, move || {
            menu_action(store.tr(|s| s.target_host_here), move || {
                store.open(Page::HostStatus);
            })
        }))
}

/// An action at the end of the switcher's menu.
fn menu_action(
    label: impl IntoReactive<SharedString>,
    on_select: impl Fn() + 'static,
) -> impl IntoElement {
    menu_item()
        .on_select(on_select)
        .px_2p5()
        .py_1p5()
        .min_h_8()
        .rounded(control_radius)
        .hover(|s| s.bg(move || theme().accent))
        .child(
            text(label)
                .text_sm()
                .text_color(move || theme().popover_foreground),
        )
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

/// The sections of the roles the work mode shows: each role's label (left out when it is the only one) and its
/// pages, then a row for each hidden role that still runs something here. `highlight` marks the selected page (the
/// sidebar does; a phone's home list does not).
pub fn role_sections(store: Store, highlight: bool) -> View {
    view()
        .gap_4()
        .child(each(
            move || store.roles().roles(),
            |role: &Role| *role,
            move |role, _| role_section(store, role.get_untracked(), highlight),
        ))
        .child(each(
            move || store.hidden_running(),
            |role: &Role| *role,
            move |role, _| hidden_running_item(store, role.get_untracked(), highlight),
        ))
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
        .child(when(
            move || store.roles().roles().len() > 1,
            move || view().px_2p5().pb_1().child(section_label(store.tr(label))),
        ))
        .child(items)
}

/// The way to a hidden role that still runs something here (this device's networks, the hosted control plane):
/// hiding a role never hides a running process. It opens the role's first page.
fn hidden_running_item(store: Store, role: Role, highlight: bool) -> View {
    let label: Pick = match role {
        Role::Node => |s| s.hidden_node_running,
        Role::Console => |s| s.role_console,
        Role::Host => |s| s.hidden_host_running,
    };
    let first = Page::of_role(role)
        .first()
        .copied()
        .unwrap_or(Page::Settings);
    nav_item(
        page_icon(first),
        store.tr(label),
        &format!("hidden-{}", first.key()),
        move || highlight && store.page().role() == Some(role),
        move || store.open(first),
    )
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
