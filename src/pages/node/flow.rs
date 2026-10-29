//! The create and join dialogs: one decision per screen (DESIGN.md "comfortable on first run"), then a network saved
//! and, on request, started.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::common::{error_of, save_failure_view, secret_labels, secrets_notice, tag};
use crate::{
    components::{
        ButtonKind, CalloutKind, HintLevel, action_button, body_text, busy_button, callout,
        dialog_footer, dialog_frame, field, group, group_row, machine_text, meta_text,
        secret_field,
    },
    core::secret::Strength,
    i18n::{Pick, fill},
    store::{
        NodeStore, Store,
        node::{Field, FlowDraft, FlowKind, FlowStep},
    },
};

/// The dialog, on screen while a flow is open.
pub(super) fn dialog(store: Store) -> If {
    let node = store.node();
    let flow = node.flow();
    dialog_frame(
        move || flow.with(|draft| draft.is_some()),
        move || {
            let words = store.strings().node.flow;
            match flow.with(|draft| draft.as_ref().map(|draft| draft.kind)) {
                Some(FlowKind::Join) => words.join_title,
                Some(FlowKind::Create) | None => words.create_title,
            }
        },
        move || node.close_flow(),
    )
    .body(move || body(store))
    .footer(move || footer(store))
    .test_id("flow-dialog")
    .build()
}

/// A field of the draft, or `default` once the flow is closed.
fn read<T>(node: NodeStore, default: T, f: impl Fn(&FlowDraft) -> T) -> T {
    node.flow().with(|draft| draft.as_ref().map_or(default, f))
}

/// The error line of `field` on the screen shown, if any.
fn visible_error(store: Store, field: Field) -> Option<SharedString> {
    let problems = read(store.node(), Vec::new(), FlowDraft::visible_problems);
    error_of(store.strings(), &problems, field)
}

/// A screen's question, in Title type.
fn prompt(text_of: impl Fn() -> &'static str + 'static) -> Text {
    text(text_of)
        .text_base()
        .font_weight(FontWeight::SEMIBOLD)
        .role(AccessibilityRole::Header)
        .text_color(move || theme().foreground)
}

/// The progress line, the screen, and the reason a save failed.
fn body(store: Store) -> View {
    let node = store.node();
    let flow = node.flow();
    let step = computed(move || flow.with(|draft| draft.as_ref().map(|draft| draft.step)));
    let progress = move || {
        let (at, total) = read(node, (1, 1), FlowDraft::position);
        fill(
            store.strings().node.flow.step_counter,
            &[("step", &at.to_string()), ("total", &total.to_string())],
        )
    };
    view()
        .gap_4()
        .child(meta_text(progress).test_id("flow-progress"))
        .child(dynamic(move || match step.get() {
            Some(FlowStep::Name) => name_step(store).into_element(),
            Some(FlowStep::Network) => network_step(store).into_element(),
            Some(FlowStep::Peer) => peer_step(store).into_element(),
            Some(FlowStep::Summary) => summary_step(store).into_element(),
            None => view().into_element(),
        }))
        .child(save_failure_view(store, move || {
            flow.with(|draft| draft.as_ref().and_then(|draft| draft.failure.clone()))
        }))
}

/// Create: what to call the network here.
fn name_step(store: Store) -> View {
    let node = store.node();
    view()
        .gap_3()
        .child(prompt(store.tr(|s| s.node.flow.name_prompt)))
        .child(
            field(
                store.tr(|s| s.node.shared.field_name),
                move || read(node, String::new(), |draft| draft.name.clone()),
                move |text| node.edit_flow(|draft| draft.name = text.to_owned()),
            )
            .placeholder(store.tr(|s| s.node.flow.name_placeholder))
            .helper(store.tr(|s| s.node.flow.name_helper))
            .error(move || visible_error(store, Field::Name))
            .test_id("flow-name"),
        )
}

/// The EasyTier network name and its secret.
fn network_step(store: Store) -> View {
    let node = store.node();
    let creating = read(node, true, |draft| draft.kind == FlowKind::Create);
    let strength = move || read(node, Strength::Weak, |draft| draft.secret.strength());
    let hint_text = move || {
        if !creating {
            return "";
        }
        let words = store.strings().common;
        match strength() {
            Strength::Weak => words.strength_weak,
            Strength::Fair => words.strength_fair,
            Strength::Strong => words.strength_strong,
        }
    };
    let hint_level = move || match strength() {
        Strength::Weak => HintLevel::Weak,
        Strength::Fair => HintLevel::Fair,
        Strength::Strong => HintLevel::Strong,
    };
    let mut secret = secret_field(
        store.tr(|s| s.node.shared.field_secret),
        move || {
            read(node, String::new(), |draft| {
                draft.secret.expose().to_owned()
            })
        },
        move |text| node.edit_flow(|draft| draft.secret = crate::core::secret::Secret::new(text)),
        secret_labels(store),
    )
    .hint(hint_text, hint_level)
    .error(move || visible_error(store, Field::Secret))
    .test_id("flow-secret");
    if creating {
        secret = secret.generate(store.tr(|s| s.common.generate), move || {
            node.flow_generate_secret();
        });
    }
    view()
        .gap_3()
        .child(prompt(move || {
            let words = store.strings().node.flow;
            if creating {
                words.network_prompt_create
            } else {
                words.network_prompt_join
            }
        }))
        .child(
            field(
                store.tr(|s| s.node.shared.field_network_name),
                move || read(node, String::new(), |draft| draft.network_name.clone()),
                move |text| node.edit_flow(|draft| draft.network_name = text.to_owned()),
            )
            .placeholder(store.tr(|s| s.node.flow.network_name_placeholder))
            .helper(store.tr(|s| s.node.flow.network_name_helper))
            .error(move || visible_error(store, Field::NetworkName))
            .test_id("flow-network-name"),
        )
        .child(secret)
        .child(
            callout(CalloutKind::Info, store.tr(|s| s.node.flow.secret_warning))
                .test_id("flow-secret-warning"),
        )
        .when(creating, |column| {
            column.child(meta_text(store.tr(|s| s.node.flow.secret_generated_note)))
        })
        .child(secrets_notice(store))
}

/// Join: the shared node or peer that leads in.
fn peer_step(store: Store) -> View {
    let node = store.node();
    let connects_to = dynamic(move || {
        let url = read(node, None, |draft| draft.peer_url().ok())?;
        Some(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(meta_text(store.tr(|s| s.node.flow.peer_connects_to)))
                .child(machine_text(url.as_str().to_owned()).test_id("flow-peer-url")),
        )
    });
    view()
        .gap_3()
        .child(prompt(store.tr(|s| s.node.flow.peer_prompt)))
        .child(
            field(
                store.tr(|s| s.node.flow.peer_label),
                move || read(node, String::new(), |draft| draft.peer.clone()),
                move |text| node.edit_flow(|draft| draft.peer = text.to_owned()),
            )
            .mono()
            .placeholder(store.tr(|s| s.node.flow.peer_placeholder))
            .helper(store.tr(|s| s.node.flow.peer_helper))
            .error(move || visible_error(store, Field::Peers))
            .test_id("flow-peer"),
        )
        .child(connects_to)
        .child(no_tun_note(store))
}

/// Phones: why the network runs without a virtual interface.
fn no_tun_note(store: Store) -> AnyElement {
    if store.node().tun_supported() {
        return view().into_element();
    }
    callout(CalloutKind::Info, store.tr(|s| s.node.flow.no_tun_note))
        .test_id("flow-no-tun")
        .into_element()
}

/// Create: what will be saved.
fn summary_step(store: Store) -> View {
    let node = store.node();
    let secret_kept = move || {
        if store.secrets_persistent() {
            store.strings().node.flow.summary_secret_kept
        } else {
            store.strings().common.secrets_not_kept
        }
    };
    let row = move |label: Pick, value: AnyElement, separated: bool| {
        group_row(separated)
            .child(meta_text(store.tr(label)).flex_shrink_0())
            .child(view().flex_1().items_end().child(value))
    };
    view()
        .gap_3()
        .child(prompt(store.tr(|s| s.node.flow.summary_prompt)))
        .child(
            group()
                .child(row(
                    |s| s.node.shared.field_name,
                    body_text(move || read(node, String::new(), FlowDraft::display_name))
                        .into_element(),
                    false,
                ))
                .child(row(
                    |s| s.node.shared.field_network_name,
                    machine_text(move || {
                        read(node, String::new(), |draft| {
                            draft.network_name.trim().to_owned()
                        })
                    })
                    .into_element(),
                    true,
                ))
                .child(row(
                    |s| s.node.flow.summary_secret,
                    body_text(secret_kept).into_element(),
                    true,
                ))
                .child(row(
                    |s| s.node.flow.summary_ip,
                    body_text(store.tr(|s| s.node.flow.summary_ip_value)).into_element(),
                    true,
                ))
                .child(row(
                    |s| s.node.flow.summary_access,
                    body_text(store.tr(|s| s.node.flow.summary_access_value)).into_element(),
                    true,
                )),
        )
        .child(if node.tun_supported() {
            meta_text(store.tr(|s| s.node.flow.summary_admin)).into_element()
        } else {
            no_tun_note(store)
        })
        .child(secrets_notice(store))
}

/// Back (or Cancel on the first screen), then Next, or the two ways to save on the last.
fn footer(store: Store) -> View {
    let node = store.node();
    let flow = node.flow();
    let last = computed(move || flow.with(|draft| draft.as_ref().is_some_and(FlowDraft::is_last)));
    let first = computed(move || {
        flow.with(|draft| {
            draft
                .as_ref()
                .is_none_or(|draft| draft.previous_step().is_none())
        })
    });
    let saving = computed(move || flow.with(|draft| draft.as_ref().is_some_and(|d| d.saving)));
    let back = tag(
        action_button(
            ButtonKind::Ghost,
            move || {
                if first.get() {
                    store.strings().common.cancel
                } else {
                    store.strings().back
                }
            },
            move || node.flow_back(),
        ),
        "flow-back",
    );
    dialog_footer().child(back).child(dynamic(move || {
        if last.get() {
            view()
                .flex_row()
                .gap_2()
                .child(tag(
                    action_button(
                        ButtonKind::Secondary,
                        store.tr(|s| s.node.flow.action_save_only),
                        move || {
                            if !saving.get() {
                                node.flow_submit(false);
                            }
                        },
                    ),
                    "flow-save-only",
                ))
                .child(tag(
                    busy_button(
                        ButtonKind::Primary,
                        store.tr(|s| s.node.flow.action_save_start),
                        move || saving.get(),
                        move || node.flow_submit(true),
                    ),
                    "flow-save-start",
                ))
        } else {
            view().child(tag(
                action_button(
                    ButtonKind::Primary,
                    store.tr(|s| s.common.next),
                    move || node.flow_next(),
                ),
                "flow-next",
            ))
        }
    }))
}
