//! The import dialog: paste a config or link, or pick a QR image, then preview and save.

use std::net::SocketAddr;

use nsh_core::prelude::*;

use super::common::{
    error_of, exposure_item, icon_label_button, import_failure_view, save_failure_view,
    secret_labels, secrets_notice, tag,
};
use crate::{
    components::{
        ButtonKind, CalloutKind, HintLevel, Icon, action_button, body_text, busy_button, callout,
        dialog_footer, dialog_frame, field, group, group_row, machine_text, meta_text,
        secret_field, section_label,
    },
    core::secret::{Secret, Strength},
    i18n::{fill, node::ImportStrings},
    profile::{Encryption, IpMode},
    store::{
        Store,
        node::{Field as FieldKind, ImportStage, NodeStore},
    },
};

/// Picks one string of [`ImportStrings`].
type Pick = fn(&'static ImportStrings) -> &'static str;

/// The dialog, on screen while `store.node().import()` holds a draft.
pub(super) fn dialog(store: Store) -> If {
    let node = store.node();
    dialog_frame(
        move || node.import().with(Option::is_some),
        store.tr(|s| s.node.import.title),
        move || node.close_import(),
    )
    .wide()
    .body(move || body(store))
    .footer(move || footer(store))
    .test_id("import-dialog")
    .build()
}

/// Whether the dialog shows the preview.
fn previewing(node: NodeStore) -> bool {
    node.import().with(|draft| {
        draft
            .as_ref()
            .is_some_and(|d| d.stage == ImportStage::Preview)
    })
}

/// Whether a picked image is being read.
fn reading(node: NodeStore) -> bool {
    node.import()
        .with(|draft| draft.as_ref().is_some_and(|d| d.reading))
}

/// Whether the profile is being saved.
fn saving(node: NodeStore) -> bool {
    node.import()
        .with(|draft| draft.as_ref().is_some_and(|d| d.saving))
}

/// The dialog's content for the stage it is at.
fn body(store: Store) -> View {
    let node = store.node();
    view().gap_4().child(
        when(move || previewing(node), move || preview(store)).otherwise(move || input(store)),
    )
}

/// The action row for the stage it is at.
fn footer(store: Store) -> View {
    let node = store.node();
    view().child(
        when(
            move || previewing(node),
            move || {
                dialog_footer()
                    .child(tag(
                        action_button(
                            ButtonKind::Ghost,
                            store.tr(|s| s.node.import.action_back),
                            move || node.import_back(),
                        ),
                        "import-back",
                    ))
                    .child(tag(
                        busy_button(
                            ButtonKind::Primary,
                            store.tr(|s| s.node.import.action_save),
                            move || saving(node),
                            move || node.import_save(),
                        ),
                        "import-save",
                    ))
            },
        )
        .otherwise(move || {
            dialog_footer()
                .child(tag(
                    action_button(
                        ButtonKind::Ghost,
                        store.tr(|s| s.common.cancel),
                        move || node.close_import(),
                    ),
                    "import-cancel",
                ))
                .child(tag(
                    icon_label_button(
                        ButtonKind::Primary,
                        Icon::Eye,
                        store.tr(|s| s.node.import.action_preview),
                        move || node.import_parse(),
                    ),
                    "import-preview",
                ))
        }),
    )
}

/// The input stage: the pasted text, the image picker and why the last try was refused.
fn input(store: Store) -> View {
    let node = store.node();
    view()
        .gap_3()
        .child(
            field(
                store.tr(|s| s.node.import.input_label),
                move || {
                    node.import()
                        .with(|draft| draft.as_ref().map(|d| d.input.expose().to_owned()))
                        .unwrap_or_default()
                },
                move |value| {
                    node.edit_import(|d| {
                        d.input = Secret::new(value);
                        d.failure = None;
                    });
                },
            )
            .mono()
            .multiline(8)
            .placeholder(move || store.strings().node.import.input_placeholder.to_owned())
            .helper(store.tr(|s| s.node.import.input_help))
            .test_id("import-text"),
        )
        .child(view().flex_row().child(tag(
            busy_button(
                ButtonKind::Secondary,
                move || {
                    if reading(node) {
                        store.strings().common.loading
                    } else {
                        store.strings().node.import.pick_image
                    }
                },
                move || reading(node),
                move || node.import_pick_image(),
            ),
            "import-pick-image",
        )))
        .child(
            dynamic(move || {
                node.import()
                    .with(|draft| draft.as_ref().and_then(|d| d.failure.clone()))
                    .map(|failure| import_failure_view(store, failure).test_id("import-failure"))
            })
            .into_element(),
        )
        .child(secrets_notice(store))
        .child(meta_text(store.tr(|s| s.node.import.nothing_applied)))
}

/// A body-type line that follows the language.
fn word(store: Store, pick: Pick) -> Text {
    body_text(move || pick(store.strings().node.import))
}

/// A meta-type line that follows the language.
fn note(store: Store, pick: Pick) -> Text {
    meta_text(move || pick(store.strings().node.import))
}

/// `values`, or the word for "none" when there are no values.
fn or_none(store: Store, values: Vec<AnyElement>) -> Vec<AnyElement> {
    if values.is_empty() {
        vec![word(store, |w| w.none).into_element()]
    } else {
        values
    }
}

/// A row of the summary: its label and the values under it.
fn row(store: Store, label: Pick, values: Vec<AnyElement>, separated: bool) -> View {
    group_row(separated)
        .items_start()
        .child(view().w_28().child(note(store, label)))
        .child(view().flex_1().gap_1().child(values))
}

/// A machine value with `extra` elements after it, on one wrapping line.
fn tagged_value(value: String, extra: Vec<AnyElement>) -> AnyElement {
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(machine_text(value))
        .child(extra)
        .into_element()
}

/// How the preview words the encryption.
fn encryption_values(store: Store, encryption: Encryption) -> Vec<AnyElement> {
    let Some(algorithm) = encryption.algorithm() else {
        return vec![word(store, |w| w.encryption_off).into_element()];
    };
    let name = machine_text(algorithm).into_element();
    if encryption.is_encryption() {
        vec![name]
    } else {
        vec![
            word(store, |w| w.encryption_obfuscation).into_element(),
            name,
        ]
    }
}

/// The preview stage: what the import reports, with the exposures first.
fn preview(store: Store) -> View {
    let node = store.node();
    let imported = node
        .import()
        .get_untracked()
        .and_then(|draft| draft.imported);
    let Some(imported) = imported else {
        return view();
    };
    let plan = &imported.preview;

    let exposures: Vec<View> = plan
        .exposures
        .iter()
        .map(|exposure| exposure_item(store, exposure))
        .collect();

    let address = match plan.ip {
        IpMode::Dhcp => vec![word(store, |w| w.address_dhcp).into_element()],
        IpMode::Static(net) => vec![
            word(store, |w| w.address_static).into_element(),
            machine_text(net.to_string()).into_element(),
        ],
        IpMode::None => vec![word(store, |w| w.address_none).into_element()],
    };
    let hostname = match &plan.hostname {
        Some(name) => vec![machine_text(name.clone()).into_element()],
        None => vec![word(store, |w| w.hostname_default).into_element()],
    };
    let peers = plan
        .peers
        .iter()
        .map(|peer| machine_text(peer.to_string()).into_element())
        .collect();
    let listeners = plan
        .listeners
        .iter()
        .map(|listener| {
            let mut extra = vec![
                note(store, |w| w.listener_port).into_element(),
                machine_text(listener.port.to_string()).into_element(),
            ];
            if listener.all_interfaces {
                extra.push(note(store, |w| w.listener_all_interfaces).into_element());
            }
            tagged_value(listener.url.to_string(), extra)
        })
        .collect();
    let exit_nodes = plan
        .exit_nodes
        .iter()
        .map(|address| machine_text(address.to_string()).into_element())
        .collect();
    let subnets = plan
        .proxied_subnets
        .iter()
        .map(|subnet| machine_text(subnet.to_string()).into_element())
        .collect();
    let routes = match &plan.manual_routes {
        Some(routes) if routes.is_empty() => vec![word(store, |w| w.routes_empty).into_element()],
        Some(routes) => routes
            .iter()
            .map(|route| machine_text(route.to_string()).into_element())
            .collect(),
        None => Vec::new(),
    };
    let mapped_listeners = plan
        .mapped_listeners
        .iter()
        .map(|listener| machine_text(listener.to_string()).into_element())
        .collect();
    let vpn_portal = plan
        .vpn_portal
        .iter()
        .map(|portal| {
            tagged_value(
                format!("{} · UDP {}", portal.client_cidr, portal.listen_port),
                vec![note(store, |w| w.listener_all_interfaces).into_element()],
            )
        })
        .collect();
    let recv_limit = plan
        .instance_recv_bps_limit
        .iter()
        .map(|limit| {
            let limit = limit.to_string();
            let strings = store.strings().node.import;
            body_text(fill(strings.recv_limit_value, &[("limit", &limit)])).into_element()
        })
        .collect();
    let acl = plan
        .acl
        .iter()
        .flat_map(|acl| {
            let strings = store.strings().node.import;
            let (chains, rules) = (acl.chains.to_string(), acl.rules.to_string());
            let mut lines = vec![
                body_text(fill(
                    strings.acl_summary,
                    &[("chains", &chains), ("rules", &rules)],
                ))
                .into_element(),
            ];
            if !acl.declared_groups.is_empty() {
                lines.push(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            acl.declared_groups
                                .iter()
                                .map(|name| machine_text(name.clone()))
                                .collect::<Vec<_>>(),
                        )
                        .into_element(),
                );
            }
            if acl.groups_without_secret > 0 {
                let count = acl.groups_without_secret.to_string();
                lines.push(
                    meta_text(fill(strings.acl_groups_missing, &[("count", &count)]))
                        .into_element(),
                );
            }
            lines
        })
        .collect();
    let forwards = plan
        .port_forwards
        .iter()
        .map(|item| {
            let forward = item.forward;
            let value = format!(
                "{} {} → {}",
                forward.proto.name(),
                forward.bind,
                forward.dst
            );
            let extra = if item.public {
                vec![note(store, |w| w.forward_public).into_element()]
            } else {
                Vec::new()
            };
            tagged_value(value, extra)
        })
        .collect();
    let socks5 = plan
        .socks5
        .iter()
        .map(|item| {
            let value = SocketAddr::new(item.proxy.host, item.proxy.port).to_string();
            let extra = if item.public {
                vec![note(store, |w| w.forward_public).into_element()]
            } else {
                Vec::new()
            };
            tagged_value(value, extra)
        })
        .collect();
    let on_off: Pick = if plan.private_mode {
        |w| w.yes
    } else {
        |w| w.no
    };
    let tun: Pick = if !plan.tun {
        |w| w.tun_no
    } else if node.tun_supported() {
        |w| w.tun_yes
    } else {
        |w| w.tun_unsupported
    };

    let summary = group()
        .child(row(
            store,
            |w| w.row_network,
            vec![machine_text(plan.network_name.clone()).into_element()],
            false,
        ))
        .child(row(store, |w| w.row_address, address, true))
        .child(row(store, |w| w.row_hostname, hostname, true))
        .child(row(store, |w| w.row_peers, or_none(store, peers), true))
        .child(row(
            store,
            |w| w.row_listeners,
            or_none(store, listeners),
            true,
        ))
        .child(row(
            store,
            |w| w.row_exit_nodes,
            or_none(store, exit_nodes),
            true,
        ))
        .child(row(store, |w| w.row_subnets, or_none(store, subnets), true))
        .child(row(store, |w| w.row_routes, or_none(store, routes), true))
        .child(row(
            store,
            |w| w.row_mapped_listeners,
            or_none(store, mapped_listeners),
            true,
        ))
        .child(row(
            store,
            |w| w.row_vpn_portal,
            or_none(store, vpn_portal),
            true,
        ))
        .child(row(
            store,
            |w| w.row_recv_limit,
            or_none(store, recv_limit),
            true,
        ))
        .child(row(store, |w| w.row_acl, or_none(store, acl), true))
        .child(row(
            store,
            |w| w.row_forwards,
            or_none(store, forwards),
            true,
        ))
        .child(row(store, |w| w.row_socks5, or_none(store, socks5), true))
        .child(row(
            store,
            |w| w.row_encryption,
            encryption_values(store, plan.encryption),
            true,
        ))
        .child(row(
            store,
            |w| w.row_private_mode,
            vec![word(store, on_off).into_element()],
            true,
        ))
        .child(row(
            store,
            |w| w.row_tun,
            vec![word(store, tun).into_element()],
            true,
        ));

    let ignored = (!imported.ignored_keys.is_empty()).then(|| {
        view()
            .gap_2()
            .child(
                callout(
                    CalloutKind::Warning,
                    store.tr(|s| s.node.import.ignored_heading),
                )
                .test_id("import-ignored"),
            )
            .child(
                view().flex_row().flex_wrap().gap_2().px_4().child(
                    imported
                        .ignored_keys
                        .iter()
                        .map(|key| machine_text(key.clone()))
                        .collect::<Vec<_>>(),
                ),
            )
    });
    let dropped = plan.default_listeners_dropped.then(|| {
        callout(
            CalloutKind::Info,
            store.tr(|s| s.node.import.listeners_dropped),
        )
        .test_id("import-listeners-dropped")
    });

    let secret: AnyElement = if plan.has_secret {
        callout(CalloutKind::Info, store.tr(|s| s.node.import.secret_kept))
            .test_id("import-secret-kept")
            .into_element()
    } else {
        view()
            .gap_2()
            .child(callout(
                CalloutKind::Info,
                store.tr(|s| s.node.import.secret_missing),
            ))
            .child(
                secret_field(
                    store.tr(|s| s.node.shared.field_secret),
                    move || {
                        node.import()
                            .with(|draft| draft.as_ref().map(|d| d.secret.expose().to_owned()))
                            .unwrap_or_default()
                    },
                    move |value| {
                        node.edit_import(|d| {
                            d.secret = Secret::new(value);
                            d.save_failure = None;
                        });
                    },
                    secret_labels(store),
                )
                .hint(
                    move || match strength_of(node) {
                        None => "",
                        Some(Strength::Weak) => store.strings().common.strength_weak,
                        Some(Strength::Fair) => store.strings().common.strength_fair,
                        Some(Strength::Strong) => store.strings().common.strength_strong,
                    },
                    move || match strength_of(node) {
                        Some(Strength::Strong) => HintLevel::Strong,
                        Some(Strength::Fair) => HintLevel::Fair,
                        Some(Strength::Weak) | None => HintLevel::Weak,
                    },
                )
                .test_id("import-secret"),
            )
            .child(note(store, |w| w.secret_help))
            .into_element()
    };

    view()
        .gap_4()
        .child(
            view()
                .gap_1()
                .child(word(store, |w| w.preview_heading).font_weight(FontWeight::MEDIUM))
                .child(note(store, |w| w.preview_help)),
        )
        .child((!exposures.is_empty()).then(|| {
            view()
                .gap_2()
                .child(section_label(store.tr(|s| s.node.import.exposures_heading)))
                .child(exposures)
        }))
        .child(
            field(
                store.tr(|s| s.node.shared.field_name),
                move || {
                    node.import()
                        .with(|draft| draft.as_ref().map(|d| d.name.clone()))
                        .unwrap_or_default()
                },
                move |value| {
                    node.edit_import(|d| {
                        d.name = value.to_owned();
                        d.save_failure = None;
                    });
                },
            )
            .helper(store.tr(|s| s.node.import.name_help))
            .error(move || {
                node.import().with(|draft| {
                    draft
                        .as_ref()
                        .filter(|d| d.show_errors)
                        .and_then(|d| error_of(store.strings(), &d.problems(), FieldKind::Name))
                })
            })
            .test_id("import-name"),
        )
        .child(summary)
        .child(ignored)
        .child(dropped)
        .child(secret)
        .child(secrets_notice(store))
        .child(save_failure_view(store, move || {
            node.import()
                .with(|draft| draft.as_ref().and_then(|d| d.save_failure.clone()))
        }))
        .child(note(store, |w| w.never_starts))
}

/// How strong the typed secret is, or `None` while it is empty.
fn strength_of(node: NodeStore) -> Option<Strength> {
    node.import().with(|draft| {
        draft
            .as_ref()
            .filter(|d| !d.secret.is_empty())
            .map(|d| d.secret.strength())
    })
}
