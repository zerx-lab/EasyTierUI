//! The Access tab: the ports the node accepts from the mesh (changeable) and the ACL's rules, tracked connections
//! and counters (read-only).

use nsh_core::prelude::*;

use super::{
    super::format::format_bytes, ListLook, POLL, Pick, ask, clear_button, editable_list, filled,
    form_row, input, loaded, page, protocol_choice, protocol_name, section, submit_button, tr,
};
use crate::{
    components::{column, columns, data_table, meta_text, neutral_chip, section_label, table_row},
    core::remote::{
        AclAction, AclConnection, AclProtocol, AclRuleStats, ConnTrackState, TransportProtocol,
    },
    store::{
        Store,
        console::{Form, Scope},
    },
};

/// The tab: reads the whitelist and the ACL's counters, and the counters again while it shows.
pub(super) fn tab(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    console.begin_runtime_view();
    console.refresh_access(scope);
    let _poll = set_interval(POLL, move || console.refresh_acl_stats(scope));
    page(
        store,
        scope,
        vec![whitelist(store, scope), acl(store, scope)],
    )
}

/// One open port or range with its protocol.
type Entry = (TransportProtocol, String);

/// The open ports: TCP first, then UDP.
fn entries(store: Store, scope: Scope) -> Vec<Entry> {
    let list = store.console().whitelist(scope).value.unwrap_or_default();
    list.tcp_ports
        .into_iter()
        .map(|port| (TransportProtocol::Tcp, port))
        .chain(
            list.udp_ports
                .into_iter()
                .map(|port| (TransportProtocol::Udp, port)),
        )
        .collect()
}

/// The ports other nodes may reach this node on, with a form to open one and a clear per protocol.
fn whitelist(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let fields = console.runtime_fields();
    let items = move || entries(store, scope);
    let cols = columns(vec![
        column(tr(store, |c| c.col_protocol)).width(88.0),
        column(tr(store, |c| c.col_ports)).flex().mono(),
        column("").width(56.0).actions(),
    ]);
    let list = loaded(
        store,
        move || console.whitelist(scope),
        move || console.refresh_whitelist(scope),
        move || {
            editable_list(
                store,
                ListLook {
                    test_id: "whitelist",
                    empty: |c| c.whitelist_empty,
                    cols: &cols,
                },
                items,
                |(protocol, port): &Entry| format!("{}-{port}", protocol_name(*protocol)),
                move |row, entry| {
                    row.text(move || {
                        entry.with(|(protocol, _)| protocol_name(*protocol).to_owned())
                    })
                    .text(move || entry.with(|(_, port)| port.clone()))
                },
                move |(protocol, port): Entry| {
                    let target = port.clone();
                    ask(
                        store,
                        filled(
                            store,
                            |c| c.remove_port_title,
                            &[
                                ("protocol", protocol_name(protocol)),
                                ("port", port.as_str()),
                            ],
                        ),
                        tr(store, |c| c.remove_port_body)(),
                        tr(store, |c| c.remove)(),
                        move || console.remove_whitelist_port(scope, protocol, target),
                    );
                },
            )
        },
    );
    let add = form_row(
        vec![
            protocol_choice(store, fields.whitelist_protocol, "whitelist-protocol"),
            input(
                store,
                |c| c.field_port,
                |c| c.placeholder_port,
                fields.whitelist_port,
                "whitelist-port",
                Some(Form::Whitelist),
            )
            .build(),
        ],
        submit_button(
            store,
            scope,
            |c| c.add,
            "whitelist-add",
            move || {
                console.add_whitelist_port(scope);
            },
        ),
    );
    let clear = |protocol: TransportProtocol, label: Pick, test_id: &'static str| {
        clear_button(
            store,
            label,
            test_id,
            move || {
                entries(store, scope)
                    .iter()
                    .any(|(known, _)| *known == protocol)
            },
            move || {
                ask(
                    store,
                    filled(
                        store,
                        |c| c.clear_ports_title,
                        &[("protocol", protocol_name(protocol))],
                    ),
                    tr(store, |c| c.clear_ports_body)(),
                    tr(store, |c| c.clear)(),
                    move || console.clear_whitelist(scope, protocol),
                );
            },
        )
    };
    section(
        store,
        |c| c.whitelist_title,
        Some(|c| c.whitelist_hint),
        "whitelist-section",
        vec![
            list,
            add.into_element(),
            clear(
                TransportProtocol::Tcp,
                |c| c.clear_tcp,
                "whitelist-clear-tcp",
            ),
            clear(
                TransportProtocol::Udp,
                |c| c.clear_udp,
                "whitelist-clear-udp",
            ),
        ],
    )
}

/// What the ACL counted: rules, tracked connections and the core's own counters.
fn acl(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let body = loaded(
        store,
        move || console.acl_stats(scope),
        move || console.refresh_acl_stats(scope),
        move || {
            view()
                .gap_4()
                .child(rules(store, scope))
                .child(connections(store, scope))
                .child(counters(store, scope))
                .into_element()
        },
    );
    section(
        store,
        |c| c.acl_title,
        Some(|c| c.acl_hint),
        "acl-section",
        vec![body],
    )
}

/// A caption over one of the ACL's tables.
fn caption(store: Store, title: Pick, body: AnyElement) -> View {
    view()
        .gap_2()
        .child(section_label(tr(store, title)))
        .child(body)
}

/// The protocol of an ACL rule or connection in words.
fn acl_protocol(store: Store, protocol: AclProtocol) -> String {
    let c = store.strings().console_runtime;
    match protocol {
        AclProtocol::Tcp => "TCP".to_owned(),
        AclProtocol::Udp => "UDP".to_owned(),
        AclProtocol::Icmp => "ICMP".to_owned(),
        AclProtocol::Icmpv6 => "ICMPv6".to_owned(),
        AclProtocol::Any => c.acl_any.to_owned(),
        AclProtocol::Unspecified | AclProtocol::Unknown => c.acl_unknown.to_owned(),
    }
}

/// The rules with how often each matched.
fn rules(store: Store, scope: Scope) -> View {
    let console = store.console();
    let items = move || console.acl_stats(scope).value.unwrap_or_default().rules;
    let cols = columns(vec![
        column(tr(store, |c| c.col_rule)).flex(),
        column(tr(store, |c| c.col_action)).width(104.0),
        column(tr(store, |c| c.col_protocol)).width(88.0),
        column(tr(store, |c| c.col_ports)).width(136.0).mono(),
        column(tr(store, |c| c.col_packets))
            .width(88.0)
            .mono()
            .right(),
        column(tr(store, |c| c.col_bytes))
            .width(96.0)
            .mono()
            .right()
            .hide_below(900.0),
    ]);
    let none = computed(move || items().is_empty());
    let table = move || {
        let rows_columns = cols.clone();
        data_table(&cols)
            .body(each(
                items,
                |rule: &AclRuleStats| format!("{}-{}", rule.name, rule.priority),
                move |rule, _| {
                    let action = move || {
                        rule.with(|rule| {
                            let c = store.strings().console_runtime;
                            if !rule.enabled {
                                return c.acl_rule_off;
                            }
                            match rule.action {
                                AclAction::Allow => c.acl_allow,
                                AclAction::Drop => c.acl_drop,
                                AclAction::Noop => c.acl_count_only,
                                AclAction::Unknown => c.acl_unknown,
                            }
                        })
                    };
                    table_row(&rows_columns)
                        .text(move || rule.with(|rule| rule.name.clone()))
                        .cell(neutral_chip(action))
                        .text(move || rule.with(|rule| acl_protocol(store, rule.protocol)))
                        .text(move || rule.with(|rule| rule.ports.join(", ")))
                        .text(move || rule.with(|rule| rule.packets.to_string()))
                        .text(move || rule.with(|rule| format_bytes(rule.bytes)))
                        .build()
                },
            ))
            .test_id("acl-rules")
            .into_element()
    };
    caption(
        store,
        |c| c.acl_rules,
        dynamic(move || {
            if none.get() {
                meta_text(tr(store, |c| c.acl_rules_empty)).into_element()
            } else {
                untracked(&table)
            }
        }),
    )
}

/// The connections the stateful ACL tracks.
fn connections(store: Store, scope: Scope) -> View {
    let console = store.console();
    let items = move || {
        console
            .acl_stats(scope)
            .value
            .unwrap_or_default()
            .connections
    };
    let cols = columns(vec![
        column(tr(store, |c| c.col_source)).width(176.0).mono(),
        column(tr(store, |c| c.col_destination)).flex().mono(),
        column(tr(store, |c| c.col_protocol)).width(80.0),
        column(tr(store, |c| c.col_state)).width(112.0),
        column(tr(store, |c| c.col_packets))
            .width(80.0)
            .mono()
            .right()
            .hide_below(900.0),
        column(tr(store, |c| c.col_bytes))
            .width(88.0)
            .mono()
            .right(),
    ]);
    let none = computed(move || items().is_empty());
    let table = move || {
        let rows_columns = cols.clone();
        data_table(&cols)
            .body(each(
                items,
                |connection: &AclConnection| {
                    format!(
                        "{:?}-{:?}-{:?}-{:?}",
                        connection.src,
                        connection.dst,
                        connection.protocol,
                        connection.created_at_ms
                    )
                },
                move |connection, _| {
                    let state = move || {
                        connection.with(|connection| {
                            let c = store.strings().console_runtime;
                            match connection.state {
                                ConnTrackState::New => c.conn_new,
                                ConnTrackState::Established => c.conn_established,
                                ConnTrackState::Related => c.conn_related,
                                ConnTrackState::Invalid => c.conn_invalid,
                                ConnTrackState::Unknown => c.acl_unknown,
                            }
                        })
                    };
                    table_row(&rows_columns)
                        .text(move || {
                            connection.with(|connection| {
                                connection
                                    .src
                                    .map(|addr| addr.to_string())
                                    .unwrap_or_default()
                            })
                        })
                        .text(move || {
                            connection.with(|connection| {
                                connection
                                    .dst
                                    .map(|addr| addr.to_string())
                                    .unwrap_or_default()
                            })
                        })
                        .text(move || connection.with(|c| acl_protocol(store, c.protocol)))
                        .cell(neutral_chip(state))
                        .text(move || connection.with(|c| c.packets.to_string()))
                        .text(move || connection.with(|c| format_bytes(c.bytes)))
                        .build()
                },
            ))
            .max_height(320.0)
            .test_id("acl-connections")
            .into_element()
    };
    caption(
        store,
        |c| c.acl_connections,
        dynamic(move || {
            if none.get() {
                meta_text(tr(store, |c| c.acl_connections_empty)).into_element()
            } else {
                untracked(&table)
            }
        }),
    )
}

/// The core's own counters of the ACL (cache size and so on).
fn counters(store: Store, scope: Scope) -> View {
    let console = store.console();
    let items = move || {
        console
            .acl_stats(scope)
            .value
            .unwrap_or_default()
            .global
            .into_iter()
            .collect::<Vec<(String, u64)>>()
    };
    let cols = columns(vec![
        column(tr(store, |c| c.col_counter)).flex().mono(),
        column(tr(store, |c| c.col_value))
            .width(136.0)
            .mono()
            .right(),
    ]);
    let none = computed(move || items().is_empty());
    let table = move || {
        let rows_columns = cols.clone();
        data_table(&cols)
            .body(each(
                items,
                |(name, _): &(String, u64)| name.clone(),
                move |counter, _| {
                    table_row(&rows_columns)
                        .text(move || counter.with(|(name, _)| name.clone()))
                        .text(move || counter.with(|(_, value)| value.to_string()))
                        .build()
                },
            ))
            .test_id("acl-counters")
            .into_element()
    };
    caption(
        store,
        |c| c.acl_counters,
        dynamic(move || {
            if none.get() {
                meta_text(tr(store, |c| c.acl_counters_empty)).into_element()
            } else {
                untracked(&table)
            }
        }),
    )
}
