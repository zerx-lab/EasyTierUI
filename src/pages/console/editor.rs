//! The network editor: one drawer for creating a network on a machine, editing a saved config and importing a
//! TOML config. Everything the config will do is in the form, grouped in sections, with the exposures it widens named
//! beside them.

use nsh_base::theme;
use nsh_core::prelude::*;
use uuid::Uuid;

use super::{format::remote_error_text, networks::load_failed, tr};
use crate::{
    components::{
        ButtonKind, CalloutKind, CopyLabels, HintLevel, SecretLabels, SelectOption, action_button,
        busy_button, callout, dialog_footer, drawer, field, meta_text, secret_field, section_label,
        segmented, select, switch_row,
    },
    core::{
        remote::RemoteError,
        secret::{Secret, Strength},
    },
    i18n::fill,
    store::{
        Store,
        console::{EditorError, EditorMode, FormProblem, NetworkingChoice},
    },
};

/// Which part of the dialog shows.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// A saved config is being read.
    Loading,
    /// A saved config could not be read.
    LoadFailed(RemoteError),
    /// Importing: the TOML is pasted here.
    Paste,
    /// The form.
    Form,
}

fn step_of(store: Store) -> Memo<Step> {
    let console = store.console();
    computed(move || {
        let editor = console.editor();
        match (editor.mode, editor.reviewing) {
            (EditorMode::Edit { .. }, false) => match (editor.loading, editor.error) {
                (false, Some(EditorError::Remote(error))) => Step::LoadFailed(error),
                _ => Step::Loading,
            },
            (EditorMode::Import, false) => Step::Paste,
            _ => Step::Form,
        }
    })
}

/// The words for what is wrong with the form or the request.
fn error_text(store: Store, error: &EditorError) -> String {
    let c = store.strings().console;
    match error {
        EditorError::NoRandomSource => c.problem_no_random.to_owned(),
        EditorError::Remote(error) => remote_error_text(store.strings(), error),
        EditorError::Problem(problem) => match problem {
            FormProblem::Machine => c.problem_machine.to_owned(),
            FormProblem::Name => c.problem_name.to_owned(),
            FormProblem::Secret => c.problem_secret.to_owned(),
            FormProblem::StaticIp => c.problem_static_ip.to_owned(),
            FormProblem::PublicUrl => c.problem_public_url.to_owned(),
            FormProblem::Peer(line) => fill(c.problem_peer, &[("line", line.as_str())]),
            FormProblem::Listener(line) => fill(c.problem_listener, &[("line", line.as_str())]),
            FormProblem::Proxy(line) => fill(c.problem_proxy, &[("line", line.as_str())]),
            FormProblem::Socks5Port => c.problem_socks5_port.to_owned(),
            FormProblem::NothingToImport => c.problem_nothing_to_import.to_owned(),
        },
    }
}

/// The drawer.
pub(super) fn drawer_view(store: Store) -> impl IntoElement {
    let console = store.console();
    drawer(
        move || console.editor().open,
        move || {
            let editor = console.editor();
            let c = store.strings().console;
            let host = editor
                .machine
                .and_then(|machine| {
                    console
                        .machines(editor.target)
                        .value
                        .and_then(|machines| machines.into_iter().find(|m| m.id == machine))
                })
                .map(|machine| machine.hostname)
                .unwrap_or_default();
            SharedString::from(match editor.mode {
                EditorMode::Create => fill(c.editor_title_create, &[("host", host.as_str())]),
                EditorMode::Edit { .. } => c.editor_title_edit.to_owned(),
                EditorMode::Import => c.editor_title_import.to_owned(),
            })
        },
        store.tr(|s| s.common.close),
        move || console.close_editor(),
    )
    .kind(store.tr(|s| s.kind_network))
    .test_id("network-editor")
    .body(move || body(store))
    .footer(move || footer(store))
}

/// A section of the form: its label (with a hairline above all but the first) over its content.
fn section(label: impl IntoReactive<SharedString>, first: bool) -> View {
    view()
        .when(!first, |head| {
            head.pt_6()
                .border_t_1()
                .border_color(move || theme().border)
        })
        .child(section_label(label))
}

/// The dialog's content: the step, and below it why the last attempt failed.
fn body(store: Store) -> View {
    let console = store.console();
    let step = step_of(store);
    view()
        .gap_4()
        .child(dynamic(move || match step.get() {
            Step::Loading => meta_text(store.tr(|s| s.common.loading)).into_element(),
            Step::LoadFailed(error) => untracked(|| {
                load_failed(store, &error, move || {
                    let editor = console.editor();
                    if let (Some(machine), EditorMode::Edit { instance }) =
                        (editor.machine, editor.mode)
                    {
                        console.load_config(editor.target, machine, instance);
                    }
                })
            }),
            Step::Paste => untracked(|| paste(store).into_element()),
            Step::Form => untracked(|| form(store).into_element()),
        }))
        .child(dynamic(move || {
            if matches!(step.get(), Step::Loading | Step::LoadFailed(_)) {
                return None;
            }
            let error = console.editor().error?;
            Some(
                callout(
                    CalloutKind::Danger,
                    SharedString::from(error_text(store, &error)),
                )
                .test_id("editor-error")
                .into_element(),
            )
        }))
}

/// The buttons of the step.
fn footer(store: Store) -> View {
    let console = store.console();
    let step = step_of(store);
    let busy = move || console.editor().busy;
    dialog_footer()
        .child(action_button(
            ButtonKind::Ghost,
            store.tr(|s| s.common.cancel),
            move || console.close_editor(),
        ))
        .child(dynamic(move || match step.get() {
            Step::Loading | Step::LoadFailed(_) => None,
            Step::Paste => Some(
                busy_button(
                    ButtonKind::Primary,
                    tr(store, |c| c.editor_read),
                    busy,
                    move || {
                        console.read_import();
                    },
                )
                .into_element(),
            ),
            Step::Form => {
                let mode = console.editor().mode;
                let label: fn(&'static crate::i18n::ConsoleStrings) -> &'static str = match mode {
                    EditorMode::Create => |c| c.editor_create,
                    EditorMode::Edit { .. } => |c| c.editor_save,
                    EditorMode::Import => |c| c.editor_run,
                };
                Some(
                    view()
                        .flex_row()
                        .gap_2()
                        .child((mode == EditorMode::Import).then(|| {
                            action_button(
                                ButtonKind::Ghost,
                                tr(store, |c| c.editor_back),
                                move || {
                                    console.back_to_import();
                                },
                            )
                        }))
                        .child(busy_button(
                            ButtonKind::Primary,
                            tr(store, label),
                            busy,
                            move || {
                                console.submit_editor();
                            },
                        ))
                        .into_element(),
                )
            }
        }))
}

/// Importing, first step: the machine and the pasted TOML.
fn paste(store: Store) -> View {
    let console = store.console();
    let fields = console.editor_fields();
    let target = console.editor().target;
    let mut machines = console.machines(target).value.unwrap_or_default();
    machines.sort_by(|a, b| a.hostname.cmp(&b.hostname));
    let options = machines
        .into_iter()
        .map(|machine| SelectOption::new(machine.id.to_string(), machine.hostname))
        .collect::<Vec<_>>();
    view()
        .gap_4()
        .child(
            select(
                tr(store, |c| c.editor_import_machine),
                options,
                move || {
                    SharedString::from(
                        console
                            .editor()
                            .machine
                            .map(|machine| machine.to_string())
                            .unwrap_or_default(),
                    )
                },
                move |value| {
                    if let Ok(machine) = Uuid::parse_str(value) {
                        console.set_editor_machine(machine);
                    }
                },
            )
            .test_id("editor-import-machine"),
        )
        .child(
            field(
                tr(store, |c| c.editor_import_toml),
                move || fields.toml.get(),
                move |text| fields.toml.set(text.to_owned()),
            )
            .multiline(10)
            .mono()
            .helper(tr(store, |c| c.editor_import_help))
            .test_id("editor-import-toml"),
        )
}

/// The strength of what is typed in the secret field.
fn strength(text: String) -> Strength {
    Secret::new(text).strength()
}

/// The form.
fn form(store: Store) -> View {
    let console = store.console();
    let fields = console.editor_fields();
    let importing = console.editor().mode == EditorMode::Import;
    let labels = SecretLabels::new(
        store.tr(|s| s.common.show),
        store.tr(|s| s.common.hide),
        CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied)),
    );
    let secret = secret_field(
        tr(store, |c| c.editor_secret),
        move || fields.secret.get(),
        move |text| fields.secret.set(text.to_owned()),
        labels,
    )
    .hint(
        move || {
            let value = fields.secret.get();
            if value.is_empty() {
                return SharedString::default();
            }
            let common = store.strings().common;
            SharedString::from(match strength(value) {
                Strength::Weak => common.strength_weak,
                Strength::Fair => common.strength_fair,
                Strength::Strong => common.strength_strong,
            })
        },
        move || match strength(fields.secret.get()) {
            Strength::Weak => HintLevel::Weak,
            Strength::Fair => HintLevel::Fair,
            Strength::Strong => HintLevel::Strong,
        },
    )
    .test_id("editor-secret");
    let secret = if console.can_generate_secret() {
        secret.generate(store.tr(|s| s.common.generate), move || {
            console.generate_secret()
        })
    } else {
        secret
    };
    let address = view()
        .gap_2()
        .child(segmented(
            tr(store, |c| c.editor_address),
            vec![
                SelectOption::new("dhcp", store.strings().console.editor_dhcp),
                SelectOption::new("static", store.strings().console.editor_static),
            ],
            move || SharedString::from(if fields.dhcp.get() { "dhcp" } else { "static" }),
            move |id| fields.dhcp.set(id == "dhcp"),
            "editor-address",
        ))
        .child(when(
            move || !fields.dhcp.get(),
            move || {
                field(
                    tr(store, |c| c.editor_static_ip),
                    move || fields.static_ip.get(),
                    move |text| fields.static_ip.set(text.to_owned()),
                )
                .placeholder("10.144.144.1/24")
                .mono()
                .test_id("editor-static-ip")
            },
        ));
    let networking = view()
        .gap_2()
        .child(segmented(
            tr(store, |c| c.editor_networking),
            vec![
                SelectOption::new(
                    NetworkingChoice::PublicServer.id(),
                    store.strings().console.editor_net_public,
                ),
                SelectOption::new(
                    NetworkingChoice::Manual.id(),
                    store.strings().console.editor_net_manual,
                ),
                SelectOption::new(
                    NetworkingChoice::Standalone.id(),
                    store.strings().console.editor_net_standalone,
                ),
            ],
            move || SharedString::from(fields.networking.get().id()),
            move |id| {
                if let Some(choice) = NetworkingChoice::from_id(id) {
                    fields.networking.set(choice);
                }
            },
            "editor-networking",
        ))
        .child(dynamic(move || match fields.networking.get() {
            NetworkingChoice::PublicServer => field(
                tr(store, |c| c.editor_public_url),
                move || fields.public_url.get(),
                move |text| fields.public_url.set(text.to_owned()),
            )
            .placeholder("tcp://shared.example.com:11010")
            .helper(tr(store, |c| c.editor_public_url_help))
            .mono()
            .test_id("editor-public-url")
            .into_element(),
            NetworkingChoice::Manual => field(
                tr(store, |c| c.editor_peers),
                move || fields.peers.get(),
                move |text| fields.peers.set(text.to_owned()),
            )
            .multiline(3)
            .placeholder("tcp://203.0.113.5:11010")
            .helper(tr(store, |c| c.editor_peers_help))
            .mono()
            .test_id("editor-peers")
            .into_element(),
            NetworkingChoice::Standalone => {
                meta_text(tr(store, |c| c.editor_standalone_note)).into_element()
            }
        }));
    let encryption_options = vec![
        SelectOption::new("default", store.strings().console.enc_default),
        SelectOption::new("aes-gcm", store.strings().console.enc_aes_gcm),
        SelectOption::new("aes-256-gcm", store.strings().console.enc_aes_256_gcm),
        SelectOption::new("chacha20", store.strings().console.enc_chacha20),
        SelectOption::new("xor", store.strings().console.enc_xor),
        SelectOption::new("off", store.strings().console.enc_off),
    ];
    view()
        .gap_4()
        .child(
            importing.then(|| callout(CalloutKind::Warning, tr(store, |c| c.editor_import_review))),
        )
        .child(section(tr(store, |c| c.editor_section_identity), true))
        .child(
            field(
                tr(store, |c| c.editor_name),
                move || fields.name.get(),
                move |text| fields.name.set(text.to_owned()),
            )
            .test_id("editor-name"),
        )
        .child(secret)
        .child(
            field(
                tr(store, |c| c.editor_hostname),
                move || fields.hostname.get(),
                move |text| fields.hostname.set(text.to_owned()),
            )
            .helper(tr(store, |c| c.editor_hostname_help))
            .test_id("editor-hostname"),
        )
        .child(section(tr(store, |c| c.editor_section_connect), false))
        .child(address)
        .child(networking)
        .child(
            field(
                tr(store, |c| c.editor_listeners),
                move || fields.listeners.get(),
                move |text| fields.listeners.set(text.to_owned()),
            )
            .multiline(3)
            .placeholder("tcp://0.0.0.0:11010")
            .helper(tr(store, |c| c.editor_listeners_help))
            .mono()
            .test_id("editor-listeners"),
        )
        .child(when(
            move || console.listeners_expose_all(),
            move || {
                callout(
                    CalloutKind::Warning,
                    tr(store, |c| c.editor_listeners_exposed),
                )
            },
        ))
        .child(section(tr(store, |c| c.editor_section_share), false))
        .child(
            field(
                tr(store, |c| c.editor_proxy),
                move || fields.proxy_cidrs.get(),
                move |text| fields.proxy_cidrs.set(text.to_owned()),
            )
            .multiline(3)
            .placeholder("192.168.1.0/24")
            .helper(tr(store, |c| c.editor_proxy_help))
            .mono()
            .test_id("editor-proxy"),
        )
        .child(when(
            move || !fields.proxy_cidrs.get().trim().is_empty(),
            move || callout(CalloutKind::Warning, tr(store, |c| c.editor_proxy_exposed)),
        ))
        .child(
            switch_row(
                tr(store, |c| c.editor_exit_node),
                move || fields.exit_node.get(),
                move |on| fields.exit_node.set(on),
            )
            .description(tr(store, |c| c.editor_exit_node_help))
            .exposure(tr(store, |c| c.editor_exit_exposed))
            .test_id("editor-exit-node"),
        )
        .child(
            switch_row(
                tr(store, |c| c.editor_socks5),
                move || fields.socks5.get(),
                move |on| fields.socks5.set(on),
            )
            .description(tr(store, |c| c.editor_socks5_help))
            .exposure(tr(store, |c| c.editor_socks5_exposed))
            .test_id("editor-socks5"),
        )
        .child(when(
            move || fields.socks5.get(),
            move || {
                field(
                    tr(store, |c| c.editor_socks5_port),
                    move || fields.socks5_port.get(),
                    move |text| fields.socks5_port.set(text.to_owned()),
                )
                .placeholder("1080")
                .mono()
                .test_id("editor-socks5-port")
            },
        ))
        .child(section(tr(store, |c| c.editor_section_behavior), false))
        .child(
            switch_row(
                tr(store, |c| c.editor_no_tun),
                move || fields.no_tun.get(),
                move |on| fields.no_tun.set(on),
            )
            .description(tr(store, |c| c.editor_no_tun_help))
            .test_id("editor-no-tun"),
        )
        .child(
            switch_row(
                tr(store, |c| c.editor_latency_first),
                move || fields.latency_first.get(),
                move |on| fields.latency_first.set(on),
            )
            .description(tr(store, |c| c.editor_latency_first_help))
            .test_id("editor-latency-first"),
        )
        .child(
            switch_row(
                tr(store, |c| c.editor_disable_p2p),
                move || fields.disable_p2p.get(),
                move |on| fields.disable_p2p.set(on),
            )
            .description(tr(store, |c| c.editor_disable_p2p_help))
            .test_id("editor-disable-p2p"),
        )
        .child(
            select(
                tr(store, |c| c.editor_encryption),
                encryption_options,
                move || SharedString::from(fields.encryption.get()),
                move |id| fields.encryption.set(id.to_owned()),
            )
            .test_id("editor-encryption"),
        )
        .child(meta_text(tr(store, |c| c.editor_encryption_help)))
        .child(when(
            move || matches!(fields.encryption.get().as_str(), "xor" | "off"),
            move || {
                callout(
                    CalloutKind::Warning,
                    tr(store, |c| c.editor_encryption_weak),
                )
            },
        ))
}
