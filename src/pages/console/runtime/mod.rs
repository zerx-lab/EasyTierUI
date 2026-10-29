//! The machine drawer's runtime tabs: what a machine's running network does right now and the changes easytier-web
//! lets the console make to it without a restart.
//!
//! - **Connections**: the connectors the node dials and the mapped listeners it announces.
//! - **Forwarding**: port forwards, subnet proxies, manual routes, exit nodes, the VPN portal and the node's own
//!   settings.
//! - **Access**: the ports open to the mesh and the ACL's counters.
//! - **Logs**: the log level of the machine's core.
//!
//! Each tab is a function of the machine and the network the drawer's picker chose ([`Scope`]); the log level
//! belongs to the machine's core and takes no network. A change is a `PatchConfig` to the **running** network:
//! it applies now and is lost when the machine reconnects or restarts, unless the network's saved config says the
//! same, so every changing tab says so and offers the config editor. The store checks every value before sending
//! it; the forms show what it refused, and a removal or a clear asks first, naming what goes.

mod access;
mod connections;
mod forwarding;
mod logs;
mod node_settings;
mod vpn;

use std::time::Duration;

use nsh_core::prelude::*;
use uuid::Uuid;

use super::{format::remote_error_text, load_failed};
use crate::{
    components::{
        ButtonKind, CalloutKind, Columns, Field, Icon, SelectOption, TableRow, action_button,
        busy_button, callout, confirm, data_table, field, icon_button, meta_text, section_label,
        segmented, table_row,
    },
    core::{
        remote::{RemoteError, TransportProtocol},
        target::TargetId,
    },
    i18n::{ConsoleRuntimeStrings, fill},
    store::{
        Store,
        console::{Feed, Form, Scope},
    },
};

/// How often what changes by itself (a connector's link, the ACL's counters, the portal's clients) is read again.
const POLL: Duration = Duration::from_secs(10);

/// Picks one string of the runtime tabs' area.
type Pick = fn(&'static ConsoleRuntimeStrings) -> &'static str;

/// A binding of the runtime string `pick` chooses, following the language as it changes.
fn tr(store: Store, pick: Pick) -> impl Fn() -> &'static str + Copy + 'static {
    move || pick(store.strings().console_runtime)
}

/// The runtime template `pick` chooses, filled with `args`, in the current language.
fn filled(store: Store, pick: Pick, args: &[(&str, &str)]) -> String {
    fill(pick(store.strings().console_runtime), args)
}

/// The connectors and mapped listeners of the machine's running network.
pub(super) fn connections_tab(
    store: Store,
    id: TargetId,
    machine: Uuid,
    instance: Uuid,
) -> AnyElement {
    connections::tab(store, Scope::new(id, machine, instance))
}

/// Port forwards, subnet proxies, manual routes, exit nodes, the VPN portal and the node's settings.
pub(super) fn forwarding_tab(
    store: Store,
    id: TargetId,
    machine: Uuid,
    instance: Uuid,
) -> AnyElement {
    forwarding::tab(store, Scope::new(id, machine, instance))
}

/// The open ports and the ACL's counters.
pub(super) fn access_tab(store: Store, id: TargetId, machine: Uuid, instance: Uuid) -> AnyElement {
    access::tab(store, Scope::new(id, machine, instance))
}

/// The log level of the machine's core.
pub(super) fn logs_tab(store: Store, id: TargetId, machine: Uuid) -> AnyElement {
    logs::tab(store, Scope::machine(id, machine))
}

// --- The tab ---------------------------------------------------------------------------------------------------

/// A tab's body: the note that changes are not saved, the last failure, then the sections.
fn page(store: Store, scope: Scope, sections: Vec<AnyElement>) -> AnyElement {
    let mut page = view()
        .gap_6()
        .child(change_note(store, scope))
        .child(failure_banner(store, scope));
    for section in sections {
        page = page.child(section);
    }
    page.into_element()
}

/// Says that a change applies to the running network only, with a way to the saved config.
fn change_note(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    callout(CalloutKind::Info, tr(store, |c| c.note_unsaved))
        .action(tr(store, |c| c.edit_config), move || {
            console.open_edit(scope.target, scope.machine, scope.instance);
        })
        .test_id("runtime-note")
        .build()
        .into_element()
}

/// Why the last change on `scope` failed, with a way to dismiss it.
fn failure_banner(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    dynamic(move || {
        let error = console.runtime_failure(scope)?;
        Some(
            callout(
                CalloutKind::Danger,
                SharedString::from(remote_error_text(store.strings(), &error)),
            )
            .action(tr(store, |c| c.dismiss), move || {
                console.dismiss_runtime_failure();
            })
            .test_id("runtime-failure")
            .build()
            .into_element(),
        )
    })
}

/// A titled part of a tab: the title, one sentence under it, then `children`.
fn section(
    store: Store,
    title: Pick,
    hint: Option<Pick>,
    test_id: &'static str,
    children: Vec<AnyElement>,
) -> AnyElement {
    let mut head = view().gap_1().child(section_label(tr(store, title)));
    if let Some(hint) = hint {
        head = head.child(meta_text(tr(store, hint)));
    }
    let mut section = view().gap_3().test_id(test_id).child(head);
    for child in children {
        section = section.child(child);
    }
    section.into_element()
}

/// Asks, naming what goes, then runs `then`. The alert opens where the press happened.
fn ask(
    store: Store,
    title: String,
    body: &'static str,
    action: &'static str,
    then: impl FnOnce() + 'static,
) {
    let asked = confirm(title, body, action, store.strings().common.cancel, true);
    store.console().when_confirmed(asked, then);
}

// --- Reads -----------------------------------------------------------------------------------------------------

/// What a read shows while it has no value.
#[derive(Debug, Clone, PartialEq)]
enum Gate {
    Loading,
    Failed(RemoteError),
    Ready,
}

/// `ready()` once `feed` has a value; until then the loading line or why the read failed, with a retry. A read
/// that fails after it had a value keeps showing the value under a warning.
fn loaded<T: Clone + PartialEq + 'static>(
    store: Store,
    feed: impl Fn() -> Feed<T> + Copy + 'static,
    retry: impl Fn() + Copy + 'static,
    ready: impl Fn() -> AnyElement + 'static,
) -> AnyElement {
    let gate = computed(move || {
        let read = feed();
        match read.value {
            Some(_) => Gate::Ready,
            None => match read.error {
                Some(error) => Gate::Failed(error),
                None => Gate::Loading,
            },
        }
    });
    let stale = computed(move || {
        let read = feed();
        read.value.and(read.error)
    });
    view()
        .gap_2()
        .child(dynamic(move || {
            stale.get().map(|_| {
                callout(CalloutKind::Warning, tr(store, |c| c.refresh_failed))
                    .action(store.tr(|s| s.common.retry), retry)
                    .build()
                    .into_element()
            })
        }))
        .child(dynamic(move || match gate.get() {
            Gate::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
            Gate::Failed(error) => untracked(|| load_failed(store, &error, retry)),
            Gate::Ready => untracked(&ready),
        }))
        .into_element()
}

// --- Lists -----------------------------------------------------------------------------------------------------

/// `text` as a piece of a test id: letters and digits stay, everything else becomes a dash.
fn test_key(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// How a list looks: the test id its table and rows start with, what shows while it is empty, and its columns.
struct ListLook<'a> {
    test_id: &'static str,
    empty: Pick,
    cols: &'a Columns,
}

/// A table of `items` whose rows end in a remove button, or the empty line while there are none. `cells` fills a
/// row's data columns (the remove button is the last column); `remove` runs with the row's item when its button is
/// pressed (the caller asks first). Rows are `<test_id>-row-<key>`, their buttons `<test_id>-remove-<key>`.
fn editable_list<T: Clone + PartialEq + 'static>(
    store: Store,
    look: ListLook<'_>,
    items: impl Fn() -> Vec<T> + Copy + 'static,
    key: fn(&T) -> String,
    cells: impl Fn(TableRow, ReadSignal<T>) -> TableRow + 'static,
    remove: impl Fn(T) + Clone + 'static,
) -> AnyElement {
    let ListLook {
        test_id,
        empty,
        cols,
    } = look;
    let none = computed(move || items().is_empty());
    let rows_columns = cols.clone();
    let cols = cols.clone();
    let cells = std::rc::Rc::new(cells);
    dynamic(move || {
        if none.get() {
            return meta_text(tr(store, empty)).into_element();
        }
        let rows_columns = rows_columns.clone();
        let cells = cells.clone();
        let remove = remove.clone();
        untracked(|| {
            data_table(&cols)
                .body(each(items, key, move |item, _| {
                    let first = key_of(key, item);
                    let pressed = remove.clone();
                    (*cells)(table_row(&rows_columns), item)
                        .cell(
                            icon_button(Icon::Trash, tr(store, |c| c.remove), move || {
                                pressed(item.get_untracked());
                            })
                            .test_id(format!("{test_id}-remove-{first}")),
                        )
                        .test_id(format!("{test_id}-row-{first}"))
                        .build()
                }))
                .test_id(test_id)
                .into_element()
        })
    })
}

/// The test-id piece of `item`'s key.
fn key_of<T: Clone + PartialEq + 'static>(key: fn(&T) -> String, item: ReadSignal<T>) -> String {
    test_key(&key(&item.get_untracked()))
}

// --- Forms -----------------------------------------------------------------------------------------------------

/// A mono text field bound to `value`. With `form` the field shows what the store refused on that form.
fn input(
    store: Store,
    label: Pick,
    placeholder: Pick,
    value: Signal<String>,
    test_id: &'static str,
    form: Option<Form>,
) -> Field {
    let console = store.console();
    let input = field(
        tr(store, label),
        move || value.get(),
        move |text| value.set(text.to_owned()),
    )
    .placeholder(move || placeholder(store.strings().console_runtime).to_owned())
    .mono()
    .test_id(test_id);
    match form {
        Some(form) => input.error(move || {
            console
                .form_error(form)
                .map(|error| SharedString::from(remote_error_text(store.strings(), &error)))
        }),
        None => input,
    }
}

/// What `form` refused, under a form with several fields (a one-field form shows it on the field).
fn form_error(store: Store, form: Form, test_id: &'static str) -> AnyElement {
    let console = store.console();
    dynamic(move || {
        let error = console.form_error(form)?;
        Some(
            callout(
                CalloutKind::Danger,
                SharedString::from(remote_error_text(store.strings(), &error)),
            )
            .test_id(test_id)
            .build()
            .into_element(),
        )
    })
}

/// A row of form fields with the button that submits them after the last one.
fn form_row(fields: Vec<View>, submit: View) -> View {
    let mut row = view().flex_row().flex_wrap().items_end().gap_2();
    for field in fields {
        row = row.child(field.flex_1().min_w(Points(160.0)));
    }
    row.child(submit)
}

/// The "Add" style button of a form: busy while a change on `scope` is in flight.
fn submit_button(
    store: Store,
    scope: Scope,
    label: Pick,
    test_id: &'static str,
    on_press: impl Fn() + 'static,
) -> View {
    let console = store.console();
    view().test_id(test_id).child(busy_button(
        ButtonKind::Secondary,
        tr(store, label),
        move || console.is_applying(scope),
        on_press,
    ))
}

/// A TCP / UDP choice bound to `protocol`.
fn protocol_choice(
    store: Store,
    protocol: Signal<TransportProtocol>,
    test_id: &'static str,
) -> View {
    segmented(
        tr(store, |c| c.col_protocol),
        vec![
            SelectOption::new("tcp", store.strings().console_runtime.proto_tcp),
            SelectOption::new("udp", store.strings().console_runtime.proto_udp),
        ],
        move || match protocol.get() {
            TransportProtocol::Tcp => SharedString::from("tcp"),
            TransportProtocol::Udp => SharedString::from("udp"),
        },
        move |id| match id {
            "tcp" => protocol.set(TransportProtocol::Tcp),
            "udp" => protocol.set(TransportProtocol::Udp),
            _ => {}
        },
        test_id,
    )
}

/// The protocol's name in the machine voice.
fn protocol_name(protocol: TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::Tcp => "TCP",
        TransportProtocol::Udp => "UDP",
    }
}

/// A ghost button under a list that clears it, shown while `shown()` holds.
fn clear_button(
    store: Store,
    label: Pick,
    test_id: &'static str,
    shown: impl Fn() -> bool + 'static,
    on_press: impl Fn() + Clone + 'static,
) -> AnyElement {
    dynamic(move || {
        shown().then(|| {
            let on_press = on_press.clone();
            view()
                .flex_row()
                .justify_end()
                .test_id(test_id)
                .child(action_button(ButtonKind::Ghost, tr(store, label), on_press))
                .into_element()
        })
    })
}
