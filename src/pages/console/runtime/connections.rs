//! The Connections tab: the connectors the node dials by itself and the mapped listeners it announces.

use nsh_core::prelude::*;

use super::{
    ListLook, POLL, Pick, ask, clear_button, editable_list, filled, form_row, input, loaded, page,
    section, submit_button, tr,
};
use crate::{
    components::{Lamp, column, columns, status_chip},
    core::remote::{Connector, ConnectorStatus},
    store::{
        Store,
        console::{Form, Scope},
    },
};

/// The tab: reads both lists and keeps the connectors' status fresh while it shows.
pub(super) fn tab(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    console.begin_runtime_view();
    console.refresh_connections(scope);
    let _poll = set_interval(POLL, move || console.refresh_connections(scope));
    page(
        store,
        scope,
        vec![connectors(store, scope), mapped_listeners(store, scope)],
    )
}

/// The lamp and word for a connector's link.
fn status_of(status: ConnectorStatus) -> (Lamp, Pick) {
    match status {
        ConnectorStatus::Connected => (Lamp::Good, |c| c.status_connected),
        ConnectorStatus::Connecting => (Lamp::Busy, |c| c.status_connecting),
        ConnectorStatus::Disconnected => (Lamp::Degraded, |c| c.status_disconnected),
        ConnectorStatus::Unknown => (Lamp::Idle, |c| c.status_unknown),
    }
}

/// The peer addresses this node dials: a table with each link's status, a form to add one and a clear-all.
fn connectors(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let fields = console.runtime_fields();
    let feed = move || console.connectors(scope);
    let retry = move || console.refresh_connectors(scope);
    let items = move || console.connectors(scope).value.unwrap_or_default();
    let cols = columns(vec![
        column(tr(store, |c| c.col_url)).flex().mono(),
        column(tr(store, |c| c.col_status)).width(144.0),
        column("").width(56.0).actions(),
    ]);
    let list = loaded(store, feed, retry, move || {
        editable_list(
            store,
            ListLook {
                test_id: "connectors",
                empty: |c| c.connectors_empty,
                cols: &cols,
            },
            items,
            |connector: &Connector| connector.url.clone(),
            move |row, connector| {
                let status = move || connector.with(|connector| status_of(connector.status));
                row.text(move || connector.with(|connector| connector.url.clone()))
                    .cell(status_chip(
                        move || status().0,
                        move || status().1(store.strings().console_runtime),
                    ))
            },
            move |connector: Connector| {
                let url = connector.url.clone();
                ask(
                    store,
                    filled(
                        store,
                        |c| c.remove_connector_title,
                        &[("url", url.as_str())],
                    ),
                    tr(store, |c| c.remove_connector_body)(),
                    tr(store, |c| c.remove)(),
                    move || console.remove_connector(scope, url),
                );
            },
        )
    });
    let add = form_row(
        vec![
            input(
                store,
                |c| c.field_connector,
                |c| c.placeholder_url,
                fields.connector,
                "connector-url",
                Some(Form::Connector),
            )
            .build(),
        ],
        submit_button(
            store,
            scope,
            |c| c.add,
            "connector-add",
            move || {
                console.add_connector(scope);
            },
        ),
    );
    let clear = clear_button(
        store,
        |c| c.connectors_clear,
        "connectors-clear",
        move || !items().is_empty(),
        move || {
            ask(
                store,
                tr(store, |c| c.clear_connectors_title)().to_owned(),
                tr(store, |c| c.clear_connectors_body)(),
                tr(store, |c| c.clear)(),
                move || console.clear_connectors(scope),
            );
        },
    );
    section(
        store,
        |c| c.connectors_title,
        Some(|c| c.connectors_hint),
        "connectors-section",
        vec![list, add.into_element(), clear],
    )
}

/// The public addresses other nodes reach this node's listeners at.
fn mapped_listeners(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let fields = console.runtime_fields();
    let feed = move || console.mapped_listeners(scope);
    let retry = move || console.refresh_mapped_listeners(scope);
    let items = move || console.mapped_listeners(scope).value.unwrap_or_default();
    let cols = columns(vec![
        column(tr(store, |c| c.col_url)).flex().mono(),
        column("").width(56.0).actions(),
    ]);
    let list = loaded(store, feed, retry, move || {
        editable_list(
            store,
            ListLook {
                test_id: "mapped-listeners",
                empty: |c| c.mapped_empty,
                cols: &cols,
            },
            items,
            |url: &String| url.clone(),
            move |row, url| row.text(move || url.with(|url| url.clone())),
            move |url: String| {
                let target = url.clone();
                ask(
                    store,
                    filled(store, |c| c.remove_mapped_title, &[("url", url.as_str())]),
                    tr(store, |c| c.remove_mapped_body)(),
                    tr(store, |c| c.remove)(),
                    move || console.remove_mapped_listener(scope, target),
                );
            },
        )
    });
    let add = form_row(
        vec![
            input(
                store,
                |c| c.field_mapped_listener,
                |c| c.placeholder_url,
                fields.mapped_listener,
                "mapped-listener-url",
                Some(Form::MappedListener),
            )
            .build(),
        ],
        submit_button(
            store,
            scope,
            |c| c.add,
            "mapped-listener-add",
            move || {
                console.add_mapped_listener(scope);
            },
        ),
    );
    let clear = clear_button(
        store,
        |c| c.mapped_clear,
        "mapped-listeners-clear",
        move || !items().is_empty(),
        move || {
            ask(
                store,
                tr(store, |c| c.clear_mapped_title)().to_owned(),
                tr(store, |c| c.clear_mapped_body)(),
                tr(store, |c| c.clear)(),
                move || console.clear_mapped_listeners(scope),
            );
        },
    );
    section(
        store,
        |c| c.mapped_title,
        Some(|c| c.mapped_hint),
        "mapped-listeners-section",
        vec![list, add.into_element(), clear],
    )
}
