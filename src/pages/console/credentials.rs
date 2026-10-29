//! Credentials: keys a node can join a network with instead of its secret, per machine and network. The machine and
//! the network are chosen in the toolbar; the credentials are a table; generating one happens in a drawer.

use nsh_base::theme;
use nsh_core::prelude::*;
use uuid::Uuid;

use super::{
    account::{failure_banner, server_toolbar},
    format::{format_utc, remote_error_text},
    gate::signed_in,
    networks::load_failed,
    tr,
};
use crate::{
    components::{
        ButtonKind, CalloutKind, CopyLabels, Icon, SelectOption, action_button, busy_button,
        callout, column, columns, copy_value, data_table, dialog_footer, drawer, empty_panel,
        field, machine_text, meta_text, neutral_chip, select, switch_row, table_row,
    },
    core::{
        remote::{Credential, RemoteError},
        target::TargetId,
    },
    i18n::fill,
    store::{Store, console::Lifetime},
};

/// What the page shows under the toolbar.
#[derive(Debug, Clone, PartialEq)]
enum Stage {
    PickMachine,
    LoadingNetworks,
    NetworksFailed(RemoteError),
    NoRunningNetwork,
    PickNetwork,
    Credentials,
}

/// What the credential list shows.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Loading,
    Failed(RemoteError),
    Empty,
    Rows,
}

/// The page of a signed-in server.
pub(super) fn ready(store: Store, id: TargetId) -> AnyElement {
    let console = store.console();
    let picked = console.credentials(id);
    if let Some(machine) = picked.machine {
        console.refresh_networks(id, machine);
        if picked.instance.is_some() {
            console.refresh_credentials(id);
        }
    }
    let machine_options = computed(move || {
        let mut machines = console.machines(id).value.unwrap_or_default();
        machines.sort_by(|a, b| a.hostname.cmp(&b.hostname));
        machines
            .into_iter()
            .map(|machine| (machine.id.to_string(), machine.hostname))
            .collect::<Vec<_>>()
    });
    let network_options = computed(move || {
        let Some(machine) = console.credentials(id).machine else {
            return Vec::new();
        };
        console
            .networks_of(id, machine)
            .value
            .unwrap_or_default()
            .into_iter()
            .filter(|network| network.running)
            .map(|network| (network.instance_id.to_string(), network.name))
            .collect::<Vec<_>>()
    });
    let stage = computed(move || {
        let state = console.credentials(id);
        let Some(machine) = state.machine else {
            return Stage::PickMachine;
        };
        let feed = console.networks_of(id, machine);
        let Some(networks) = feed.value else {
            return match feed.error {
                Some(error) => Stage::NetworksFailed(error),
                None => Stage::LoadingNetworks,
            };
        };
        if !networks.iter().any(|network| network.running) {
            return Stage::NoRunningNetwork;
        }
        if state.instance.is_none() {
            Stage::PickNetwork
        } else {
            Stage::Credentials
        }
    });
    let pickers = vec![
        dynamic(move || {
            let options = machine_options.get();
            untracked(|| machine_picker(store, id, options).into_element())
        })
        .into_element(),
        dynamic(move || {
            let options = network_options.get();
            (!options.is_empty())
                .then(|| untracked(|| network_picker(store, id, options).into_element()))
        })
        .into_element(),
    ];
    view()
        .gap_4()
        .child(server_toolbar(
            store,
            id,
            pickers,
            false,
            move || {
                console.refresh_machines(id);
                if let Some(machine) = console.credentials(id).machine {
                    console.refresh_networks(id, machine);
                }
                console.refresh_credentials(id);
            },
            move || console.credentials(id).list.read_at_ms,
        ))
        .child(failure_banner(store, id))
        .child(
            view()
                .gap_2()
                .child(callout(
                    CalloutKind::Info,
                    tr(store, |c| c.credential_explain),
                ))
                .child(callout(
                    CalloutKind::Info,
                    tr(store, |c| c.credential_no_block),
                )),
        )
        .child(dynamic(move || match stage.get() {
            Stage::PickMachine => panel(empty_panel(
                Icon::Key,
                tr(store, |c| c.credentials_section),
                tr(store, |c| c.credentials_pick_machine),
                None,
            )),
            Stage::LoadingNetworks => meta_text(store.tr(|s| s.common.loading)).into_element(),
            Stage::NetworksFailed(error) => untracked(|| {
                load_failed(store, &error, move || {
                    if let Some(machine) = console.credentials(id).machine {
                        console.refresh_networks(id, machine);
                    }
                })
            }),
            Stage::NoRunningNetwork => panel(empty_panel(
                Icon::Network,
                tr(store, |c| c.credentials_no_network_title),
                tr(store, |c| c.credentials_no_network_body),
                None,
            )),
            Stage::PickNetwork => panel(empty_panel(
                Icon::Key,
                tr(store, |c| c.credentials_section),
                tr(store, |c| c.credentials_pick_network),
                None,
            )),
            Stage::Credentials => untracked(|| list(store, id)),
        }))
        .into_element()
}

/// An empty state inside a panel with a hairline, like a table.
fn panel(content: View) -> AnyElement {
    view()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .child(content)
        .into_element()
}

/// The actions this page puts in the page header: generating a credential, once a machine's network is chosen.
pub(super) fn header_actions(store: Store) -> AnyElement {
    dynamic(move || {
        let id = signed_in(store)?;
        let picked = store.console().credentials(id);
        (picked.machine.is_some() && picked.instance.is_some()).then(|| {
            view()
                .test_id("credential-generate")
                .child(action_button(
                    ButtonKind::Primary,
                    tr(store, |c| c.generate_credential),
                    move || store.console().open_credential_form(),
                ))
                .into_element()
        })
    })
}

/// The machine choice.
fn machine_picker(store: Store, id: TargetId, machines: Vec<(String, String)>) -> View {
    let console = store.console();
    let mut options = vec![SelectOption::new(
        "",
        store.strings().console.credentials_choose_machine,
    )];
    options.extend(
        machines
            .into_iter()
            .map(|(machine, host)| SelectOption::new(machine, host)),
    );
    select(
        tr(store, |c| c.credentials_machine),
        options,
        move || {
            SharedString::from(
                console
                    .credentials(id)
                    .machine
                    .map(|machine| machine.to_string())
                    .unwrap_or_default(),
            )
        },
        move |value| {
            if let Ok(machine) = Uuid::parse_str(value) {
                console.pick_credential_machine(id, machine);
            }
        },
    )
    .inline()
    .test_id("credentials-machine")
    .build()
}

/// The network choice among the machine's running networks.
fn network_picker(store: Store, id: TargetId, networks: Vec<(String, String)>) -> View {
    let console = store.console();
    let mut options = vec![SelectOption::new(
        "",
        store.strings().console.credentials_choose_network,
    )];
    options.extend(
        networks
            .into_iter()
            .map(|(instance, name)| SelectOption::new(instance, name)),
    );
    select(
        tr(store, |c| c.credentials_network),
        options,
        move || {
            SharedString::from(
                console
                    .credentials(id)
                    .instance
                    .map(|instance| instance.to_string())
                    .unwrap_or_default(),
            )
        },
        move |value| {
            if let Ok(instance) = Uuid::parse_str(value) {
                console.pick_credential_network(id, instance);
            }
        },
    )
    .inline()
    .test_id("credentials-network")
    .build()
}

/// The credentials of the picked network as a table.
fn list(store: Store, id: TargetId) -> AnyElement {
    let console = store.console();
    let kind = computed(move || {
        let feed = console.credentials(id).list;
        match feed.value {
            None => match feed.error {
                Some(error) => Kind::Failed(error),
                None => Kind::Loading,
            },
            Some(list) if list.is_empty() => Kind::Empty,
            Some(_) => Kind::Rows,
        }
    });
    let stale = computed(move || {
        let feed = console.credentials(id).list;
        feed.value.and(feed.error)
    });
    let cols = columns(vec![
        column(tr(store, |c| c.col_credential)).flex().mono(),
        column(tr(store, |c| c.col_groups)).width(176.0),
        column(tr(store, |c| c.col_options))
            .width(184.0)
            .hide_below(1000.0),
        column(tr(store, |c| c.credential_expires))
            .width(176.0)
            .mono()
            .right()
            .hide_below(900.0),
        column("").width(96.0).actions(),
    ]);
    view()
        .gap_4()
        .child(dynamic(move || {
            stale.get().map(|_| {
                callout(CalloutKind::Warning, tr(store, |c| c.refresh_failed))
                    .action(store.tr(|s| s.common.retry), move || {
                        console.refresh_credentials(id)
                    })
                    .into_element()
            })
        }))
        .child(dynamic(move || match kind.get() {
            Kind::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
            Kind::Failed(error) => {
                untracked(|| load_failed(store, &error, move || console.refresh_credentials(id)))
            }
            Kind::Empty => panel(empty_panel(
                Icon::Key,
                tr(store, |c| c.credentials_empty_title),
                tr(store, |c| c.credentials_empty_body),
                Some(
                    action_button(
                        ButtonKind::Primary,
                        tr(store, |c| c.generate_credential),
                        move || console.open_credential_form(),
                    )
                    .into_element(),
                ),
            )),
            Kind::Rows => {
                let rows_columns = cols.clone();
                untracked(|| {
                    data_table(&cols)
                        .body(each(
                            move || console.credentials(id).list.value.unwrap_or_default(),
                            |credential: &Credential| credential.id.clone(),
                            move |credential, _| {
                                credential_row(store, id, &rows_columns, credential)
                            },
                        ))
                        .test_id("credentials-table")
                        .into_element()
                })
            }
        }))
        .into_element()
}

/// One credential: its id, its groups, what it allows and when it ends.
fn credential_row(
    store: Store,
    id: TargetId,
    cols: &crate::components::Columns,
    credential: ReadSignal<Credential>,
) -> View {
    let first = credential.get_untracked();
    let credential_id = first.id.clone();
    let copy_id = credential_id.clone();
    let groups = view()
        .flex_row()
        .items_center()
        .gap_1()
        .min_w_0()
        .overflow_hidden()
        .child(dynamic(move || {
            let groups = credential.with(|credential| credential.groups.clone());
            if groups.is_empty() {
                return vec![
                    meta_text(tr(store, |c| c.credential_no_groups))
                        .number_of_lines(1)
                        .into_element(),
                ];
            }
            groups
                .into_iter()
                .map(|group| neutral_chip(SharedString::from(group)).into_element())
                .collect()
        }));
    let flags = view()
        .flex_row()
        .items_center()
        .gap_1()
        .min_w_0()
        .overflow_hidden()
        .child(dynamic(move || {
            let (relay, reusable) = credential
                .with(|credential| (credential.allow_relay, credential.reusable == Some(true)));
            vec![
                relay.then(|| neutral_chip(tr(store, |c| c.credential_relay)).into_element()),
                reusable.then(|| neutral_chip(tr(store, |c| c.credential_reusable)).into_element()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        }));
    let revoke_id = credential_id.clone();
    table_row(cols)
        .cell(copy_value(
            &format!("credential-{credential_id}"),
            move || copy_id.clone(),
            CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied)),
        ))
        .cell(groups)
        .cell(flags)
        .text(move || {
            credential
                .with(|credential| credential.expires_at_ms)
                .map(format_utc)
                .unwrap_or_default()
        })
        .cell(action_button(
            ButtonKind::Ghost,
            tr(store, |c| c.revoke),
            move || confirm_revoke(store, id, revoke_id.clone()),
        ))
        .detail(dynamic(move || {
            let subnets = credential.with(|credential| credential.allowed_proxy_cidrs.clone());
            (!subnets.is_empty()).then(|| {
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .pb_3()
                    .child(meta_text(tr(store, |c| c.credential_subnets)))
                    .child(
                        machine_text(SharedString::from(subnets.join(", ")))
                            .text_color(move || theme().muted_foreground),
                    )
                    .into_element()
            })
        }))
        .build()
}

/// Asks, naming the credential, then revokes it.
fn confirm_revoke(store: Store, id: TargetId, credential_id: String) {
    let strings = store.strings();
    let asked = crate::components::confirm(
        fill(
            strings.console.revoke_title,
            &[("id", credential_id.as_str())],
        ),
        strings.console.revoke_body,
        strings.console.revoke,
        strings.common.cancel,
        true,
    );
    let console = store.console();
    console.when_confirmed(asked, move || console.revoke_credential(id, credential_id));
}

/// The "Generate credential" form, in a drawer.
pub(super) fn generate_drawer(store: Store) -> impl IntoElement {
    let console = store.console();
    let fields = console.credential_fields();
    drawer(
        move || fields.open.get(),
        tr(store, |c| c.credential_form_title),
        store.tr(|s| s.common.close),
        move || console.close_credential_form(),
    )
    .kind(tr(store, |c| c.kind_credential))
    .body(move || {
        let options = Lifetime::ALL
            .into_iter()
            .map(|lifetime| {
                SelectOption::new(
                    lifetime.id(),
                    match lifetime {
                        Lifetime::OneHour => store.strings().console.lifetime_1h,
                        Lifetime::OneDay => store.strings().console.lifetime_1d,
                        Lifetime::SevenDays => store.strings().console.lifetime_7d,
                        Lifetime::ThirtyDays => store.strings().console.lifetime_30d,
                    },
                )
            })
            .collect::<Vec<_>>();
        view()
            .gap_4()
            .child(
                select(
                    tr(store, |c| c.credential_lifetime),
                    options,
                    move || SharedString::from(fields.lifetime.get().id()),
                    move |id| {
                        if let Some(lifetime) = Lifetime::from_id(id) {
                            fields.lifetime.set(lifetime);
                        }
                    },
                )
                .test_id("credential-lifetime"),
            )
            .child(
                field(
                    tr(store, |c| c.credential_groups),
                    move || fields.groups.get(),
                    move |text| fields.groups.set(text.to_owned()),
                )
                .helper(tr(store, |c| c.credential_groups_help))
                .test_id("credential-groups"),
            )
            .child(
                switch_row(
                    tr(store, |c| c.credential_allow_relay),
                    move || fields.relay.get(),
                    move |on| fields.relay.set(on),
                )
                .description(tr(store, |c| c.credential_allow_relay_help))
                .exposure(tr(store, |c| c.credential_relay_exposed))
                .test_id("credential-relay"),
            )
            .child(
                switch_row(
                    tr(store, |c| c.credential_reusable_label),
                    move || fields.reusable.get(),
                    move |on| fields.reusable.set(on),
                )
                .description(tr(store, |c| c.credential_reusable_help))
                .exposure(tr(store, |c| c.credential_reusable_exposed))
                .test_id("credential-reusable"),
            )
            .child(dynamic(move || {
                let error = fields.error.get()?;
                Some(
                    callout(
                        CalloutKind::Danger,
                        SharedString::from(remote_error_text(store.strings(), &error)),
                    )
                    .into_element(),
                )
            }))
    })
    .footer(move || {
        dialog_footer()
            .child(action_button(
                ButtonKind::Ghost,
                store.tr(|s| s.common.cancel),
                move || console.close_credential_form(),
            ))
            .child(busy_button(
                ButtonKind::Primary,
                tr(store, |c| c.credential_generate),
                move || fields.busy.get(),
                move || console.generate_credential(store.current_target_id()),
            ))
    })
    .test_id("credential-form")
}
