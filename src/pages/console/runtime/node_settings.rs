//! Node settings of the Forwarding tab: the hostname and virtual IPv4 the node announces, and whether it relays
//! other nodes' data.

use nsh_core::prelude::*;

use super::{filled, form_row, input, submit_button, tr};
use crate::{
    components::switch_row,
    store::{
        Store,
        console::{Form, RunningFacts, Scope},
    },
};

/// The section: current values from the running config, a form for each of the two that need typing, and the
/// relay switch.
pub(super) fn section(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let fields = console.runtime_fields();
    let current = move |pick: fn(&RunningFacts) -> String| {
        console
            .running_facts(scope)
            .value
            .map(|facts| pick(&facts))
            .unwrap_or_default()
    };
    let hostname = form_row(
        vec![
            input(
                store,
                |c| c.field_hostname,
                |c| c.placeholder_hostname,
                fields.hostname,
                "node-hostname",
                Some(Form::Hostname),
            )
            .helper(move || {
                let name = current(|facts| facts.hostname.clone().unwrap_or_default());
                SharedString::from(if name.is_empty() {
                    store.strings().console_runtime.hostname_default.to_owned()
                } else {
                    filled(store, |c| c.hostname_current, &[("name", name.as_str())])
                })
            })
            .build(),
        ],
        submit_button(
            store,
            scope,
            |c| c.apply,
            "node-hostname-apply",
            move || {
                console.set_hostname(scope);
            },
        ),
    );
    let virtual_ip = form_row(
        vec![
            input(
                store,
                |c| c.field_virtual_ip,
                |c| c.placeholder_virtual_ip,
                fields.virtual_ip,
                "node-virtual-ip",
                Some(Form::VirtualIp),
            )
            .helper(move || {
                let facts = console.running_facts(scope).value.unwrap_or_default();
                SharedString::from(if facts.dhcp {
                    store.strings().console_runtime.virtual_ip_dhcp.to_owned()
                } else {
                    match facts.virtual_ipv4 {
                        Some(ip) => filled(store, |c| c.virtual_ip_current, &[("ip", ip.as_str())]),
                        None => store.strings().console_runtime.virtual_ip_none.to_owned(),
                    }
                })
            })
            .build(),
        ],
        submit_button(
            store,
            scope,
            |c| c.apply,
            "node-virtual-ip-apply",
            move || {
                console.set_virtual_ip(scope);
            },
        ),
    );
    let relay = switch_row(
        tr(store, |c| c.relay_off_label),
        move || {
            console
                .running_facts(scope)
                .value
                .is_some_and(|facts| facts.relay_data_disabled)
        },
        move |disabled| console.set_relay_data_disabled(scope, disabled),
    )
    .description(tr(store, |c| c.relay_off_help))
    .disabled(move || console.is_applying(scope))
    .test_id("node-relay-off");
    super::section(
        store,
        |c| c.node_title,
        Some(|c| c.node_hint),
        "node-settings-section",
        vec![
            hostname.into_element(),
            virtual_ip.into_element(),
            relay.build().into_element(),
        ],
    )
}
