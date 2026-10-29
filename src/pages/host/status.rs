//! The hosted control plane's status: a toolbar with the process's lamp, a summary strip, the settings as facts
//! (edited in a drawer), where it listens, and its output. Start, Restart and Stop live in the page header.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::parts::{copy_labels, panel, problem_notice, release_notice};
use crate::{
    components::{
        ButtonKind, CalloutKind, Icon, Lamp, SelectOption, action_button, age_text, busy_button,
        callout, code_block, dialog_footer, drawer, empty_panel, fact, fact_mono, field, group,
        icon_button, kv_grid, live_dot, machine_text, meta_text, neutral_chip, segmented,
        status_lamp, summary_cell, summary_strip, toolbar,
    },
    deploy::ConfigProtocol,
    store::{
        Page, Store,
        host::{Busy, FormError, HostRun, HostStore, OwnerStep},
    },
};

/// The status page.
pub fn page(store: Store) -> View {
    let host = store.host();
    view()
        .gap_6()
        .child(
            toolbar()
                .child(status_lamp(lamp(store), word(store)))
                .child(dynamic(move || {
                    release_of(host).map(|version| neutral_chip(SharedString::from(version)))
                }))
                .child(live(store)),
        )
        .child(release_notice(store))
        .child(problems(store))
        .child(next_step(store))
        .child(summary(store))
        .child(when(
            move || host.run() == HostRun::Stopped,
            move || {
                group().child(empty_panel(
                    Icon::Server,
                    store.tr(|s| s.host_stopped_title),
                    store.tr(|s| s.host_stopped_body),
                    None,
                ))
            },
        ))
        .child(settings_panel(store))
        .child(logs_panel(store))
        .child(settings_drawer(store))
}

/// The page header's actions: Start while the process is down, Restart and Stop while it is up.
pub fn actions(store: Store) -> AnyElement {
    let host = store.host();
    // The busy state changes in the same update as the run state this switches on: it is read once per render
    // (re-rendering on change) rather than bound inside buttons a re-render is about to replace.
    dynamic(move || {
        let active = matches!(host.run(), HostRun::Starting | HostRun::Running(_));
        let busy = host.busy();
        if active {
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(busy_button(
                    ButtonKind::Secondary,
                    store.tr(|s| s.host.restart),
                    move || busy == Busy::Restarting,
                    move || host.restart(),
                ))
                .child(busy_button(
                    ButtonKind::Secondary,
                    store.tr(|s| s.host.stop),
                    move || busy == Busy::Stopping,
                    move || host.stop(),
                ))
        } else {
            view().flex_row().items_center().child(busy_button(
                ButtonKind::Primary,
                store.tr(|s| s.host.start),
                move || busy == Busy::Starting,
                move || host.start(),
            ))
        }
    })
}

/// The lamp for the process and the busy action under way.
fn lamp(store: Store) -> impl Fn() -> Lamp + Copy {
    let host = store.host();
    move || match host.busy() {
        Busy::Stopping | Busy::Restarting | Busy::Starting => Lamp::Busy,
        Busy::Idle | Busy::Saving | Busy::Hardening | Busy::Owner => match host.run() {
            HostRun::Stopped => Lamp::Idle,
            HostRun::Starting => Lamp::Busy,
            HostRun::Running(_) => Lamp::Good,
            HostRun::Failed(_) => Lamp::Failed,
        },
    }
}

/// The word beside the lamp.
fn word(store: Store) -> impl Fn() -> &'static str + Copy {
    let host = store.host();
    move || {
        let strings = store.strings();
        match host.busy() {
            Busy::Stopping => strings.common.state_stopping,
            Busy::Restarting | Busy::Starting => strings.common.state_starting,
            Busy::Idle | Busy::Saving | Busy::Hardening | Busy::Owner => match host.run() {
                HostRun::Stopped => strings.common.state_stopped,
                HostRun::Starting => strings.common.state_starting,
                HostRun::Running(_) => strings.common.state_running,
                HostRun::Failed(_) => strings.common.state_failed,
            },
        }
    }
}

/// The release the running process is, if it runs.
fn release_of(host: HostStore) -> Option<String> {
    match host.run() {
        HostRun::Running(running) => Some(running.version),
        _ => None,
    }
}

/// The REST API's port: the running one, else the saved one.
fn api_port(host: HostStore) -> u16 {
    match host.run() {
        HostRun::Running(running) => running.api_port,
        _ => host.config().api_port,
    }
}

/// The config server's port: the running one, else the saved one.
fn config_port(host: HostStore) -> u16 {
    match host.run() {
        HostRun::Running(running) => running.config_port,
        _ => host.config().config_port,
    }
}

/// The config server's protocol: the running one, else the saved one.
fn config_protocol(host: HostStore) -> ConfigProtocol {
    match host.run() {
        HostRun::Running(running) => running.protocol,
        _ => host.config().protocol,
    }
}

/// A protocol's name.
fn protocol_name(store: Store, protocol: ConfigProtocol) -> &'static str {
    let strings = store.strings();
    match protocol {
        ConfigProtocol::Udp => strings.host.protocol_udp,
        ConfigProtocol::Tcp => strings.host.protocol_tcp,
        ConfigProtocol::Ws => strings.host.protocol_ws,
    }
}

/// The live dot and "Updated N s ago", while the process runs and is being read.
fn live(store: Store) -> AnyElement {
    let host = store.host();
    when(
        move || matches!(host.run(), HostRun::Running(_)),
        move || {
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(live_dot(|| true))
                .child(age_text(
                    move || host.updated_ms(),
                    store.tr(|s| s.common.updated_ago),
                    store.tr(|s| s.common.updated_now),
                ))
        },
    )
    .into_element()
}

/// State, release (while it runs) and the two ports, as the process or the saved settings have them.
fn summary(store: Store) -> AnyElement {
    let host = store.host();
    let running = computed(move || matches!(host.run(), HostRun::Running(_)));
    dynamic(move || {
        let mut cells = vec![
            summary_cell(store.tr(|s| s.host.state_label), word(store)).test_id("summary-state"),
        ];
        if running.get() {
            cells.push(
                summary_cell(store.tr(|s| s.host.version_label), move || {
                    SharedString::from(release_of(host).unwrap_or_default())
                })
                .test_id("summary-release"),
            );
        }
        cells.push(
            summary_cell(store.tr(|s| s.host.api_label), move || {
                SharedString::from(api_port(host).to_string())
            })
            .caption(store.tr(|s| s.host.api_scope))
            .test_id("summary-api"),
        );
        cells.push(
            summary_cell(store.tr(|s| s.host.config_label), move || {
                SharedString::from(config_port(host).to_string())
            })
            .unit(move || protocol_name(store, config_protocol(host)))
            .caption(store.tr(|s| s.host.config_scope))
            .test_id("summary-config"),
        );
        summary_strip(cells)
    })
}

/// What went wrong: the last action's failure, or why the process failed.
fn problems(store: Store) -> AnyElement {
    let host = store.host();
    dynamic(move || {
        let failed = match host.run() {
            HostRun::Failed(problem) => Some(problem),
            _ => None,
        };
        let problem = host.problem().or(failed)?;
        // The release notice already says it, with the same way out.
        let covered = problem.needs_release() && !store.versions().active().is_ready();
        (!covered).then(|| problem_notice(store, &problem))
    })
}

/// After the first start: the default accounts must be replaced and the owner account registered.
fn next_step(store: Store) -> AnyElement {
    let host = store.host();
    when(
        move || matches!(host.run(), HostRun::Running(_)) && !host.checks().all_pass(),
        move || {
            callout(CalloutKind::Info, store.tr(|s| s.host.next_harden))
                .action(store.tr(|s| s.host.open_hardening), move || {
                    store.open(Page::Hardening);
                })
                .test_id("host-next-harden")
        },
    )
    .into_element()
}

/// The settings as facts, where the process listens, and what the config server exposes. Editing is the drawer's.
fn settings_panel(store: Store) -> View {
    let host = store.host();
    let edit = view().test_id("host-edit-settings").child(action_button(
        ButtonKind::Secondary,
        store.tr(|s| s.host.edit_settings),
        move || host.open_settings_editor(),
    ));
    let saved = kv_grid(vec![
        fact_mono(store.tr(|s| s.host.field_api_port), move || {
            SharedString::from(api_port(host).to_string())
        }),
        fact_mono(store.tr(|s| s.host.field_config_port), move || {
            SharedString::from(config_port(host).to_string())
        }),
        fact_mono(store.tr(|s| s.host.field_protocol), move || {
            protocol_name(store, config_protocol(host))
        }),
        fact(store.tr(|s| s.host.registration_label), move || {
            let strings = store.strings();
            // Registration is off from every start; it is open only while the owner-registration window is.
            let window_open = matches!(
                host.owner_step(),
                OwnerStep::Waiting { .. } | OwnerStep::Submitting { .. }
            );
            if host.config().registration_disabled && !window_open {
                strings.host.registration_off
            } else {
                strings.host.registration_on
            }
        }),
    ]);
    let addresses = when(
        move || matches!(host.run(), HostRun::Running(_)),
        move || {
            kv_grid(vec![
                fact_mono(store.tr(|s| s.host.address_api), move || {
                    SharedString::from(match host.run() {
                        HostRun::Running(running) => format!("127.0.0.1:{}", running.api_port),
                        _ => String::new(),
                    })
                })
                .copyable(copy_labels(store))
                .test_id("host-api-address"),
                fact_mono(store.tr(|s| s.host.address_config), move || {
                    SharedString::from(match host.run() {
                        HostRun::Running(running) => format!(
                            "{}://0.0.0.0:{}",
                            running.protocol.scheme(),
                            running.config_port
                        ),
                        _ => String::new(),
                    })
                })
                .copyable(copy_labels(store))
                .test_id("host-config-address"),
            ])
        },
    );
    panel(
        store.tr(|s| s.host.section_settings),
        Some(edit.into_element()),
    )
    .child(
        view()
            .p_4()
            .gap_4()
            .child(saved)
            .child(addresses)
            .child(
                callout(CalloutKind::Warning, store.tr(|s| s.host.exposure_note))
                    .test_id("host-exposure"),
            )
            .child(
                machine_text(move || {
                    SharedString::from(format!(
                        "{} {}",
                        protocol_name(store, config_protocol(host)),
                        config_port(host)
                    ))
                })
                .test_id("host-listen")
                .text_color(move || theme().foreground),
            )
            .child(config_problem(store))
            .child(when(
                move || host.is_hosting(),
                move || meta_text(store.tr(|s| s.host.settings_applies)),
            )),
    )
}

/// The unreadable saved settings, which the defaults replaced on screen.
fn config_problem(store: Store) -> AnyElement {
    let host = store.host();
    when(
        move || host.config_problem().is_some(),
        move || callout(CalloutKind::Warning, store.tr(|s| s.host.settings_corrupt)),
    )
    .into_element()
}

/// The process output.
fn logs_panel(store: Store) -> View {
    let host = store.host();
    let refresh = icon_button(Icon::Refresh, store.tr(|s| s.common.refresh), move || {
        host.refresh()
    })
    .test_id("host-refresh");
    let body = when(
        move || host.logs().is_empty(),
        move || meta_text(store.tr(|s| s.host.logs_empty)),
    )
    .otherwise(move || {
        code_block(
            "host-logs",
            move || host.logs().join("\n"),
            copy_labels(store),
        )
    });
    panel(
        store.tr(|s| s.host.section_logs),
        Some(refresh.into_element()),
    )
    .child(view().p_4().child(body))
}

/// The drawer that edits the ports and the protocol; Cancel drops the edits, Save keeps them for the next start.
fn settings_drawer(store: Store) -> AnyElement {
    let host = store.host();
    drawer(
        move || host.settings_open(),
        store.tr(|s| s.host.settings_title),
        store.tr(|s| s.common.close),
        move || host.close_settings_editor(),
    )
    .kind(store.tr(|s| s.host.kind_control_plane))
    .body(move || settings_form(store))
    .footer(move || settings_footer(store))
    .test_id("host-settings-drawer")
    .into_element()
}

/// The drawer's fields: ports, protocol, and what the config server exposes.
fn settings_form(store: Store) -> View {
    let host = store.host();
    let port_error = move |wrong: FormError| {
        move || {
            (host.form_error() == Some(wrong)).then(|| {
                SharedString::from(match wrong {
                    FormError::ApiNotNumber | FormError::ConfigNotNumber => {
                        store.strings().host.port_not_number
                    }
                    FormError::Invalid => store.strings().host.ports_invalid,
                })
            })
        }
    };
    let invalid = move || {
        // Out-of-range or equal ports are about both fields: say it under the second.
        (host.form_error() == Some(FormError::Invalid))
            .then(|| SharedString::from(store.strings().host.ports_invalid))
    };
    let protocol = dynamic(move || {
        let strings = store.strings();
        untracked(|| {
            segmented(
                store.tr(|s| s.host.field_protocol),
                vec![
                    SelectOption::new("udp", strings.host.protocol_udp),
                    SelectOption::new("tcp", strings.host.protocol_tcp),
                    SelectOption::new("ws", strings.host.protocol_ws),
                ],
                move || SharedString::from(protocol_key(host.protocol())),
                move |key| {
                    if let Some(protocol) = protocol_of(key) {
                        host.set_protocol(protocol);
                    }
                },
                "host-protocol",
            )
        })
    });
    view()
        .gap_4()
        .child(dynamic(move || {
            host.problem()
                .map(|problem| problem_notice(store, &problem))
        }))
        .child(
            field(
                store.tr(|s| s.host.field_api_port),
                move || host.api_port_text(),
                move |text| host.set_api_port_text(text),
            )
            .mono()
            .helper(store.tr(|s| s.host.help_api_port))
            .error(port_error(FormError::ApiNotNumber))
            .test_id("host-api-port"),
        )
        .child(
            field(
                store.tr(|s| s.host.field_config_port),
                move || host.config_port_text(),
                move |text| host.set_config_port_text(text),
            )
            .mono()
            .helper(store.tr(|s| s.host.help_config_port))
            .error(move || port_error(FormError::ConfigNotNumber)().or_else(invalid))
            .test_id("host-config-port"),
        )
        .child(protocol)
        .child(callout(
            CalloutKind::Warning,
            store.tr(|s| s.host.exposure_note),
        ))
        .child(machine_text(move || {
            SharedString::from(format!(
                "{} {}",
                protocol_name(store, host.protocol()),
                host.config_port_text().trim()
            ))
        }))
        .child(when(
            move || host.is_hosting(),
            move || meta_text(store.tr(|s| s.host.settings_applies)),
        ))
}

/// The drawer's footer: Cancel, and Save once the form differs from what is saved.
fn settings_footer(store: Store) -> View {
    let host = store.host();
    // Read once per render and passed as constants, never bound into the button.
    let save = dynamic(move || {
        let saving = host.busy() == Busy::Saving;
        let blocked = host.busy() != Busy::Idle || !host.form_dirty();
        busy_button(
            ButtonKind::Primary,
            store.tr(|s| s.host.save_settings),
            move || saving,
            move || host.save_settings(),
        )
        .disabled(blocked)
    });
    dialog_footer()
        .child(action_button(
            ButtonKind::Ghost,
            store.tr(|s| s.common.cancel),
            move || host.close_settings_editor(),
        ))
        .child(save)
}

/// The key of a protocol in the selector.
fn protocol_key(protocol: ConfigProtocol) -> &'static str {
    match protocol {
        ConfigProtocol::Udp => "udp",
        ConfigProtocol::Tcp => "tcp",
        ConfigProtocol::Ws => "ws",
    }
}

/// The protocol a selector key stands for.
fn protocol_of(key: &str) -> Option<ConfigProtocol> {
    [ConfigProtocol::Udp, ConfigProtocol::Tcp, ConfigProtocol::Ws]
        .into_iter()
        .find(|protocol| protocol_key(*protocol) == key)
}
