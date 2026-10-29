//! Server deployment files: a plan form on the left (sections), the rendered files on the right (a file list and the
//! chosen file's text); stacked on phones and narrow windows. Nothing is applied from here; the same page works on
//! every platform, the web build and the phones included.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::parts::{copy_labels, panel, size_text};
use crate::{
    components::{
        CalloutKind, Icon, Lamp, SelectOption, callout, code_block, column, columns, data_table,
        empty_panel, field, group, group_row, machine_text, meta_text, neutral_chip, segmented,
        status_lamp, switch_row, table_row, toolbar,
    },
    deploy::{
        Artifact, ConfigProtocol, DeployError, Field, Hardening, LinuxArch, ProxyKind, Reason,
    },
    i18n::{Pick, fill},
    store::{
        Store,
        host::{DeployOutcome, RuntimeKind},
    },
};

/// The window width from which the form and the files sit side by side, in points.
const TWO_PANES_FROM: f32 = 960.0;

/// The form pane's width when side by side, in points.
const FORM_WIDTH: f32 = 400.0;

/// The deployment page.
pub fn page(store: Store) -> View {
    let host = store.host();
    let two_panes = computed(move || is_desktop() && window().width >= Points(TWO_PANES_FROM));
    let status = move || match host.deploy_outcome() {
        DeployOutcome::Ready(_) => Lamp::Good,
        DeployOutcome::Empty => Lamp::Idle,
        DeployOutcome::Invalid(_) => Lamp::Degraded,
    };
    let word = move || {
        SharedString::from(match host.deploy_outcome() {
            DeployOutcome::Ready(files) => fill(
                store.strings().host.files_count,
                &[("count", &files.len().to_string())],
            ),
            DeployOutcome::Empty | DeployOutcome::Invalid(_) => {
                store.strings().deployment_empty_title.to_owned()
            }
        })
    };
    view()
        .gap_6()
        .child(toolbar().child(status_lamp(status, word)))
        .child(dynamic(move || {
            if two_panes.get() {
                view()
                    .flex_row()
                    .items_start()
                    .gap_6()
                    .child(plan(store).w(Points(FORM_WIDTH)).flex_shrink_0())
                    .child(files(store).flex_1().min_w_0())
            } else {
                view().gap_6().child(plan(store)).child(files(store))
            }
        }))
}

/// The plan form: the server, how it runs, ports, its own node, and how far hardening is.
fn plan(store: Store) -> View {
    view()
        .gap_6()
        .child(panel(store.tr(|s| s.host.section_server), None).child(server(store)))
        .child(panel(store.tr(|s| s.host.section_runtime), None).child(runtime(store)))
        .child(panel(store.tr(|s| s.host.section_ports), None).child(ports(store)))
        .child(panel(store.tr(|s| s.host.section_node), None).child(node(store)))
        .child(panel(store.tr(|s| s.host.section_hardening), None).child(hardening_progress(store)))
}

/// The words of a refused field, under that field.
fn error_text(store: Store, error: DeployError) -> Option<SharedString> {
    let strings = store.strings();
    let name = match error.field {
        Field::Domain => strings.host.error_field_domain,
        Field::AcmeEmail => strings.host.error_field_email,
        Field::ApiPort => strings.host.error_field_api_port,
        Field::ConfigPort => strings.host.error_field_config_port,
        Field::NodeOwner => strings.host.error_field_owner,
        Field::ImageTag
        | Field::ImageDigest
        | Field::Version
        | Field::AssetSha256
        | Field::DownloadUrl
        | Field::DataVolume
        | Field::StateDir
        | Field::WebDir => return None,
    };
    let reason: Pick = match error.reason {
        Reason::Empty => |s| s.host.reason_empty,
        Reason::TooLong => |s| s.host.reason_too_long,
        Reason::TooShort => |s| s.host.reason_too_short,
        Reason::BadCharacter => |s| s.host.reason_bad_character,
        Reason::BadFormat => |s| s.host.reason_bad_format,
        Reason::Reserved => |s| s.host.reason_reserved,
        Reason::Conflict => |s| s.host.reason_conflict,
        Reason::OutOfRange => |s| s.host.reason_out_of_range,
        Reason::Insecure => |s| s.host.reason_insecure,
        Reason::Mismatch => |s| s.host.reason_mismatch,
    };
    Some(SharedString::from(fill(
        reason(strings),
        &[("field", name)],
    )))
}

/// The error under `field`, if the form's last render was refused for it.
fn error_under(store: Store, field: Field) -> impl Fn() -> Option<SharedString> + 'static {
    move || match store.host().deploy_outcome() {
        DeployOutcome::Invalid(error) if error.field == field => error_text(store, error),
        DeployOutcome::Empty | DeployOutcome::Invalid(_) | DeployOutcome::Ready(_) => None,
    }
}

/// A selector over `options` (id, label) with one selected; language changes rebuild it.
fn choice(
    store: Store,
    label: Pick,
    options: &[(&'static str, Pick)],
    selected: impl Fn() -> &'static str + Copy + 'static,
    on_select: impl Fn(&str) + Copy + 'static,
    test_id: &'static str,
) -> AnyElement {
    let options = options.to_vec();
    dynamic(move || {
        let strings = store.strings();
        untracked(|| {
            segmented(
                store.tr(label),
                options
                    .iter()
                    .map(|(id, pick)| SelectOption::new(*id, pick(strings)))
                    .collect(),
                move || SharedString::from(selected()),
                on_select,
                test_id,
            )
        })
    })
}

/// The body of a form panel: 16 pt padding all round (so a focus ring is never clipped) and 16 pt between fields.
fn body() -> View {
    view().p_4().gap_4()
}

/// Domain and ACME email, and the reverse proxy.
fn server(store: Store) -> View {
    let host = store.host();
    body()
        .child(
            field(
                store.tr(|s| s.host.field_domain),
                move || host.deploy_form().domain,
                move |text| host.edit_deploy(|form| form.domain = text.to_owned()),
            )
            .mono()
            .placeholder(String::from("vpn.example.com"))
            .helper(store.tr(|s| s.host.help_domain))
            .error(error_under(store, Field::Domain))
            .test_id("deploy-domain"),
        )
        .child(
            field(
                store.tr(|s| s.host.field_email),
                move || host.deploy_form().email,
                move |text| host.edit_deploy(|form| form.email = text.to_owned()),
            )
            .mono()
            .placeholder(String::from("admin@example.com"))
            .helper(store.tr(|s| s.host.help_email))
            .error(error_under(store, Field::AcmeEmail))
            .test_id("deploy-email"),
        )
        .child(choice(
            store,
            |s| s.host.field_proxy,
            &[
                ("caddy", |s| s.host.proxy_caddy),
                ("nginx", |s| s.host.proxy_nginx),
            ],
            move || match host.deploy_form().proxy {
                ProxyKind::Caddy => "caddy",
                ProxyKind::Nginx => "nginx",
            },
            move |key| {
                let proxy = match key {
                    "caddy" => ProxyKind::Caddy,
                    "nginx" => ProxyKind::Nginx,
                    _ => return,
                };
                host.edit_deploy(|form| form.proxy = proxy);
            },
            "deploy-proxy",
        ))
        .child(meta_text(store.tr(|s| s.host.help_proxy)))
}

/// Docker Compose or systemd (and the CPU for systemd).
fn runtime(store: Store) -> View {
    let host = store.host();
    body()
        .child(choice(
            store,
            |s| s.host.field_runtime,
            &[
                ("compose", |s| s.host.runtime_compose),
                ("systemd", |s| s.host.runtime_systemd),
            ],
            move || match host.deploy_form().runtime {
                RuntimeKind::Compose => "compose",
                RuntimeKind::Systemd => "systemd",
            },
            move |key| {
                let runtime = match key {
                    "compose" => RuntimeKind::Compose,
                    "systemd" => RuntimeKind::Systemd,
                    _ => return,
                };
                host.edit_deploy(|form| form.runtime = runtime);
            },
            "deploy-runtime",
        ))
        .child(when(
            move || host.deploy_form().runtime == RuntimeKind::Compose,
            move || meta_text(store.tr(|s| s.host.help_runtime_compose)),
        ))
        .child(when(
            move || host.deploy_form().runtime == RuntimeKind::Systemd,
            move || {
                view()
                    .gap_4()
                    .child(meta_text(store.tr(|s| s.host.help_runtime_systemd)))
                    .child(choice(
                        store,
                        |s| s.host.field_arch,
                        &[
                            ("x86_64", |s| s.host.arch_x86_64),
                            ("aarch64", |s| s.host.arch_aarch64),
                        ],
                        move || match host.deploy_form().arch {
                            LinuxArch::X86_64 => "x86_64",
                            LinuxArch::Aarch64 => "aarch64",
                        },
                        move |key| {
                            let arch = match key {
                                "x86_64" => LinuxArch::X86_64,
                                "aarch64" => LinuxArch::Aarch64,
                                _ => return,
                            };
                            host.edit_deploy(|form| form.arch = arch);
                        },
                        "deploy-arch",
                    ))
            },
        ))
}

/// The two ports and the config server's protocol.
fn ports(store: Store) -> View {
    let host = store.host();
    body()
        .child(
            field(
                store.tr(|s| s.host.field_api_port),
                move || host.deploy_form().api_port,
                move |text| host.edit_deploy(|form| form.api_port = text.to_owned()),
            )
            .mono()
            .helper(store.tr(|s| s.host.help_api_port))
            .error(error_under(store, Field::ApiPort))
            .test_id("deploy-api-port"),
        )
        .child(
            field(
                store.tr(|s| s.host.field_config_port),
                move || host.deploy_form().config_port,
                move |text| host.edit_deploy(|form| form.config_port = text.to_owned()),
            )
            .mono()
            .helper(store.tr(|s| s.host.help_config_port))
            .error(error_under(store, Field::ConfigPort))
            .test_id("deploy-config-port"),
        )
        .child(choice(
            store,
            |s| s.host.field_protocol,
            &[
                ("udp", |s| s.host.protocol_udp),
                ("tcp", |s| s.host.protocol_tcp),
                ("ws", |s| s.host.protocol_ws),
            ],
            move || match host.deploy_form().protocol {
                ConfigProtocol::Udp => "udp",
                ConfigProtocol::Tcp => "tcp",
                ConfigProtocol::Ws => "ws",
            },
            move |key| {
                let protocol = match key {
                    "udp" => ConfigProtocol::Udp,
                    "tcp" => ConfigProtocol::Tcp,
                    "ws" => ConfigProtocol::Ws,
                    _ => return,
                };
                host.edit_deploy(|form| form.protocol = protocol);
            },
            "deploy-protocol",
        ))
}

/// Whether the server is also a node, and under which account it enrolls.
fn node(store: Store) -> View {
    let host = store.host();
    body()
        .child(
            switch_row(
                store.tr(|s| s.host.node_switch),
                move || host.deploy_form().node,
                move |on| host.edit_deploy(|form| form.node = on),
            )
            .description(store.tr(|s| s.host.node_description))
            .test_id("deploy-node"),
        )
        .child(when(
            move || host.deploy_form().node,
            move || {
                field(
                    store.tr(|s| s.host.field_owner),
                    move || host.deploy_form().owner,
                    move |text| host.edit_deploy(|form| form.owner = text.to_owned()),
                )
                .mono()
                .helper(store.tr(|s| s.host.help_owner))
                .error(error_under(store, Field::NodeOwner))
                .test_id("deploy-owner")
            },
        ))
}

/// How far hardening is, and what that changes at the proxy.
fn hardening_progress(store: Store) -> View {
    let host = store.host();
    let pending = move || host.deploy_form().hardening == Hardening::Pending;
    body()
        .child(choice(
            store,
            |s| s.host.field_hardening,
            &[
                ("pending", |s| s.host.hardening_pending),
                ("done", |s| s.host.hardening_done),
            ],
            move || {
                if pending() { "pending" } else { "done" }
            },
            move |key| {
                let hardening = match key {
                    "pending" => Hardening::Pending,
                    "done" => Hardening::Done,
                    _ => return,
                };
                host.edit_deploy(|form| form.hardening = hardening);
            },
            "deploy-hardening",
        ))
        .child(
            callout(CalloutKind::Info, store.tr(|s| s.host.hardening_explain))
                .test_id("deploy-hardening-note"),
        )
        .child(
            group().child(
                group_row(false)
                    .flex_wrap()
                    .child(
                        text(store.tr(|s| s.host.api_path_label))
                            .text_sm()
                            .flex_1()
                            .text_color(move || theme().card_foreground),
                    )
                    .child(machine_text("/api/").text_color(move || theme().muted_foreground))
                    .child(
                        text(move || {
                            let strings = store.strings();
                            if pending() {
                                strings.host.api_closed
                            } else {
                                strings.host.api_open
                            }
                        })
                        .text_sm()
                        .text_color(move || theme().card_foreground),
                    ),
            ),
        )
}

/// The rendered files, or why there are none yet.
fn files(store: Store) -> View {
    let host = store.host();
    let ready = move || matches!(host.deploy_outcome(), DeployOutcome::Ready(_));
    view()
        .gap_6()
        .child(when(ready, move || rendered(store)).otherwise(move || {
            view()
                .gap_4()
                .child(dynamic(move || match host.deploy_outcome() {
                    DeployOutcome::Invalid(error) if error_text(store, error).is_none() => Some(
                        callout(CalloutKind::Danger, store.tr(|s| s.host.error_pinned)),
                    ),
                    DeployOutcome::Empty | DeployOutcome::Invalid(_) | DeployOutcome::Ready(_) => {
                        None
                    }
                }))
                .child(group().child(empty_panel(
                    Icon::Rocket,
                    store.tr(|s| s.deployment_empty_title),
                    store.tr(|s| s.deployment_empty_body),
                    None,
                )))
        }))
}

/// The files, README first, as a list (name, permissions, size) and the chosen one's text below it.
fn rendered(store: Store) -> View {
    let host = store.host();
    let artifacts = move || match host.deploy_outcome() {
        DeployOutcome::Ready(artifacts) => artifacts,
        DeployOutcome::Empty | DeployOutcome::Invalid(_) => Vec::new(),
    };
    let cols = columns(vec![
        column(store.tr(|s| s.host.col_file)).flex().mono(),
        column(store.tr(|s| s.host.col_mode))
            .width(110.0)
            .mono()
            .right(),
        column(store.tr(|s| s.host.col_size))
            .width(90.0)
            .mono()
            .right(),
    ]);
    let list = data_table(&cols)
        .body(each(
            artifacts,
            |artifact: &Artifact| artifact.file_name.clone(),
            move |artifact, _index| file_row(store, &cols, artifact),
        ))
        .test_id("deploy-files");
    view()
        .gap_4()
        .child(meta_text(store.tr(|s| s.host.files_note)))
        .child(list)
        .child(dynamic(move || {
            host.selected_artifact()
                .map(|artifact| file_text(store, artifact))
        }))
}

/// A row of the file list: name, permissions and size; pressing it shows the file.
fn file_row(
    store: Store,
    cols: &crate::components::Columns,
    artifact: ReadSignal<Artifact>,
) -> View {
    let host = store.host();
    let name = artifact.with(|artifact| artifact.file_name.clone());
    let mode = artifact
        .with(|artifact| artifact.mode)
        .map(|mode| format!("{mode:04o}"))
        .unwrap_or_default();
    let size = artifact.with(|artifact| size_text(artifact.text.len()));
    let chosen = name.clone();
    let shown = name.clone();
    let press = name.clone();
    table_row(cols)
        .text(SharedString::from(name.clone()))
        .text(SharedString::from(mode))
        .text(SharedString::from(size))
        .selected(move || {
            host.selected_artifact()
                .is_some_and(|artifact| artifact.file_name == chosen)
        })
        .on_press(move || host.select_artifact(&press))
        .label(SharedString::from(shown))
        .test_id(format!("deploy-row-{name}"))
        .build()
}

/// The chosen file: its name, its permissions with what they mean, and its text with a Copy button.
fn file_text(store: Store, artifact: Artifact) -> View {
    let name = artifact.file_name.clone();
    let id = format!("deploy-file-{name}");
    let permissions = artifact.mode.map(|mode| {
        let note: Option<Pick> = match mode {
            0o600 => Some(|s| s.host.mode_owner_only),
            0o755 => Some(|s| s.host.mode_executable),
            _ => None,
        };
        view()
            .flex_row()
            .items_center()
            .gap_2()
            .child(meta_text(store.tr(|s| s.host.file_mode_label)))
            .child(
                machine_text(SharedString::from(format!("{mode:04o}")))
                    .test_id(&format!("{id}-mode")),
            )
            .child(note.map(|note| neutral_chip(store.tr(note))))
    });
    let text = artifact.text;
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(
                    machine_text(SharedString::from(name))
                        .flex_1()
                        .text_color(move || theme().foreground),
                )
                .child(permissions),
        )
        .child(code_block(&id, move || text.clone(), copy_labels(store)))
}
