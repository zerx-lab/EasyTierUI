//! The Forwarding tab: port forwards, subnet proxies, manual routes and exit nodes, the node's own settings and the
//! VPN portal.

use nsh_core::prelude::*;

use super::{
    ListLook, POLL, Pick, ask, editable_list, filled, form_error, form_row, input, loaded,
    node_settings, page, protocol_choice, protocol_name, section, submit_button, tr, vpn,
};
use crate::{
    components::{CalloutKind, callout, column, columns},
    core::remote::PortForwardEntry,
    store::{
        Store,
        console::{ConsoleStore, Form, ProxyNet, RunningFacts, RuntimeFields, Scope},
    },
};

/// The tab: the lists, the node's settings and the portal, read when it opens and again while it shows.
pub(super) fn tab(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    console.begin_runtime_view();
    console.refresh_forwarding(scope);
    let _poll = set_interval(POLL, move || console.refresh_vpn_portal(scope));
    page(
        store,
        scope,
        vec![
            port_forwards(store, scope),
            running_config(store, scope),
            vpn::section(store, scope),
        ],
    )
}

/// What the running config holds that the tab lists and edits, gated once: the lists come from one read.
fn running_config(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    loaded(
        store,
        move || console.running_facts(scope),
        move || console.refresh_running_facts(scope),
        move || {
            view()
                .gap_6()
                .child(subnet_proxies(store, scope))
                .child(address_list(store, scope, ROUTES))
                .child(address_list(store, scope, EXIT_NODES))
                .child(node_settings::section(store, scope))
                .into_element()
        },
    )
}

/// The facts of the running config, empty before the first read.
fn facts(console: ConsoleStore, scope: Scope) -> RunningFacts {
    console.running_facts(scope).value.unwrap_or_default()
}

/// The protocol, bind address and destination of a port forward: connections to the bind address on this node go
/// to the destination through the network.
fn port_forwards(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let fields = console.runtime_fields();
    let items = move || console.port_forwards(scope).value.unwrap_or_default();
    let cols = columns(vec![
        column(tr(store, |c| c.col_protocol)).width(88.0),
        column(tr(store, |c| c.col_bind)).width(180.0).mono(),
        column(tr(store, |c| c.col_destination)).flex().mono(),
        column("").width(56.0).actions(),
    ]);
    let list = loaded(
        store,
        move || console.port_forwards(scope),
        move || console.refresh_port_forwards(scope),
        move || {
            editable_list(
                store,
                ListLook {
                    test_id: "port-forwards",
                    empty: |c| c.forwards_empty,
                    cols: &cols,
                },
                items,
                |entry: &PortForwardEntry| {
                    format!(
                        "{}-{}-{}",
                        protocol_name(entry.protocol),
                        entry.bind,
                        entry.dst
                    )
                },
                move |row, entry| {
                    row.text(move || entry.with(|entry| protocol_name(entry.protocol).to_owned()))
                        .text(move || entry.with(|entry| entry.bind.to_string()))
                        .text(move || entry.with(|entry| entry.dst.to_string()))
                },
                move |entry: PortForwardEntry| {
                    let bind = entry.bind.to_string();
                    let dst = entry.dst.to_string();
                    ask(
                        store,
                        filled(
                            store,
                            |c| c.remove_forward_title,
                            &[
                                ("protocol", protocol_name(entry.protocol)),
                                ("bind", bind.as_str()),
                                ("dst", dst.as_str()),
                            ],
                        ),
                        tr(store, |c| c.remove_forward_body)(),
                        tr(store, |c| c.remove)(),
                        move || console.remove_port_forward(scope, entry),
                    );
                },
            )
        },
    );
    let add = form_row(
        vec![
            protocol_choice(store, fields.forward_protocol, "forward-protocol"),
            input(
                store,
                |c| c.field_bind,
                |c| c.placeholder_bind,
                fields.forward_bind,
                "forward-bind",
                None,
            )
            .build(),
            input(
                store,
                |c| c.field_destination,
                |c| c.placeholder_destination,
                fields.forward_dst,
                "forward-dst",
                None,
            )
            .build(),
        ],
        submit_button(
            store,
            scope,
            |c| c.add,
            "forward-add",
            move || {
                console.add_port_forward(scope);
            },
        ),
    );
    section(
        store,
        |c| c.forwards_title,
        Some(|c| c.forwards_hint),
        "port-forwards-section",
        vec![
            list,
            add.into_element(),
            form_error(store, Form::PortForward, "forward-error"),
        ],
    )
}

/// The subnets this node proxies into the network, each optionally announced as another range.
fn subnet_proxies(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let fields = console.runtime_fields();
    let items = move || facts(console, scope).proxy_networks;
    let cols = columns(vec![
        column(tr(store, |c| c.col_subnet)).flex().mono(),
        column(tr(store, |c| c.col_announced_as))
            .width(180.0)
            .mono(),
        column("").width(56.0).actions(),
    ]);
    let list = editable_list(
        store,
        ListLook {
            test_id: "subnet-proxies",
            empty: |c| c.subnets_empty,
            cols: &cols,
        },
        items,
        |net: &ProxyNet| net.cidr.clone(),
        move |row, net| {
            row.text(move || net.with(|net| net.cidr.clone()))
                .text(move || net.with(|net| net.mapped.clone().unwrap_or_default()))
        },
        move |net: ProxyNet| {
            let cidr = net.cidr.clone();
            ask(
                store,
                filled(
                    store,
                    |c| c.remove_subnet_title,
                    &[("cidr", net.cidr.as_str())],
                ),
                tr(store, |c| c.remove_subnet_body)(),
                tr(store, |c| c.remove)(),
                move || console.remove_subnet_proxy(scope, cidr),
            );
        },
    );
    let add = form_row(
        vec![
            input(
                store,
                |c| c.field_subnet,
                |c| c.placeholder_subnet,
                fields.subnet,
                "subnet-cidr",
                None,
            )
            .build(),
            input(
                store,
                |c| c.field_subnet_mapped,
                |c| c.placeholder_subnet_mapped,
                fields.subnet_mapped,
                "subnet-mapped",
                None,
            )
            .build(),
        ],
        submit_button(
            store,
            scope,
            |c| c.add,
            "subnet-add",
            move || {
                console.add_subnet_proxy(scope);
            },
        ),
    );
    section(
        store,
        |c| c.subnets_title,
        Some(|c| c.subnets_hint),
        "subnet-proxies-section",
        vec![
            list,
            add.into_element(),
            form_error(store, Form::SubnetProxy, "subnet-error"),
        ],
    )
}

/// A list of addresses of the running config with a one-field add form: manual routes and exit nodes.
#[derive(Clone, Copy)]
struct AddressList {
    test_id: &'static str,
    section_id: &'static str,
    title: Pick,
    hint: Pick,
    empty: Pick,
    column: Pick,
    field_label: Pick,
    placeholder: Pick,
    remove_title: Pick,
    remove_body: Pick,
    form: Form,
    input_id: &'static str,
    add_id: &'static str,
    items: fn(&RunningFacts) -> Vec<String>,
    value: fn(&RuntimeFields) -> Signal<String>,
    add: fn(&ConsoleStore, Scope),
    remove: fn(&ConsoleStore, Scope, String),
    needs_manual_routing: bool,
}

/// Manual routes: extra subnets the node sends through the network.
const ROUTES: AddressList = AddressList {
    test_id: "routes",
    section_id: "routes-section",
    title: |c| c.routes_title,
    hint: |c| c.routes_hint,
    empty: |c| c.routes_empty,
    column: |c| c.col_route,
    field_label: |c| c.field_route,
    placeholder: |c| c.placeholder_route,
    remove_title: |c| c.remove_route_title,
    remove_body: |c| c.remove_route_body,
    form: Form::Route,
    input_id: "route-cidr",
    add_id: "route-add",
    items: |facts| facts.routes.clone(),
    value: |fields| fields.route,
    add: ConsoleStore::add_route,
    remove: ConsoleStore::remove_route,
    needs_manual_routing: true,
};

/// Exit nodes: the peers internet traffic is sent through.
const EXIT_NODES: AddressList = AddressList {
    test_id: "exit-nodes",
    section_id: "exit-nodes-section",
    title: |c| c.exit_nodes_title,
    hint: |c| c.exit_nodes_hint,
    empty: |c| c.exit_nodes_empty,
    column: |c| c.col_address,
    field_label: |c| c.field_exit_node,
    placeholder: |c| c.placeholder_exit_node,
    remove_title: |c| c.remove_exit_node_title,
    remove_body: |c| c.remove_exit_node_body,
    form: Form::ExitNode,
    input_id: "exit-node-ip",
    add_id: "exit-node-add",
    items: |facts| facts.exit_nodes.clone(),
    value: |fields| fields.exit_node,
    add: ConsoleStore::add_exit_node,
    remove: ConsoleStore::remove_exit_node,
    needs_manual_routing: false,
};

/// One address list of the running config.
fn address_list(store: Store, scope: Scope, spec: AddressList) -> AnyElement {
    let console = store.console();
    let fields = console.runtime_fields();
    let items = move || (spec.items)(&facts(console, scope));
    let cols = columns(vec![
        column(tr(store, spec.column)).flex().mono(),
        column("").width(56.0).actions(),
    ]);
    let list = editable_list(
        store,
        ListLook {
            test_id: spec.test_id,
            empty: spec.empty,
            cols: &cols,
        },
        items,
        |value: &String| value.clone(),
        move |row, value| row.text(move || value.with(|value| value.clone())),
        move |value: String| {
            let target = value.clone();
            ask(
                store,
                filled(store, spec.remove_title, &[("value", value.as_str())]),
                tr(store, spec.remove_body)(),
                tr(store, |c| c.remove)(),
                move || (spec.remove)(&console, scope, target),
            );
        },
    );
    let add = form_row(
        vec![
            input(
                store,
                spec.field_label,
                spec.placeholder,
                (spec.value)(&fields),
                spec.input_id,
                Some(spec.form),
            )
            .build(),
        ],
        submit_button(
            store,
            scope,
            |c| c.add,
            spec.add_id,
            move || {
                (spec.add)(&console, scope);
            },
        ),
    );
    section(
        store,
        spec.title,
        Some(spec.hint),
        spec.section_id,
        vec![
            list,
            add.into_element(),
            manual_routing_note(store, scope, spec),
        ],
    )
}

/// For manual routes: says that the network's config has manual routing off, so a route added here does nothing.
fn manual_routing_note(store: Store, scope: Scope, spec: AddressList) -> AnyElement {
    let console = store.console();
    dynamic(move || {
        let off = spec.needs_manual_routing && !facts(console, scope).manual_routes_on;
        off.then(|| {
            callout(CalloutKind::Warning, tr(store, |c| c.routes_off_note))
                .test_id("routes-off-note")
                .build()
                .into_element()
        })
    })
}
