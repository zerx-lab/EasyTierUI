//! First-run hardening of the hosted control plane: a toolbar with the checks' tally, the checklist as a table, the new
//! passwords, the owner account (captcha, then the enrollment address), the result, and the passwords kept in the
//! keychain (shown in a drawer, only on request). "Harden now" is the page header's.

use nsh_core::prelude::*;

use super::parts::{
    copy_labels, owner_notice, panel, problem_notice, release_notice, secret_labels,
};
use crate::{
    components::{
        ButtonKind, CalloutKind, Icon, Lamp, action_button, busy_button, callout, column, columns,
        data_table, dialog_footer, drawer, empty_panel, fact_mono, field, group, kv_grid,
        meta_text, secret_field, status_lamp, table_row, toolbar,
    },
    i18n::{Pick, fill},
    store::{
        Page, Store,
        host::{Busy, Check, HardeningOutcome, HardeningState, HostRun, HostStore, OwnerStep},
    },
};

/// Whether nothing is hosted and nothing is known: there is nothing to check yet, only a way to the Status page.
fn idle(host: HostStore) -> bool {
    !host.is_hosting()
        && !host.hardening_known()
        && host.hardening() == HardeningState::Idle
        && host.run() == HostRun::Stopped
}

/// The hardening page.
pub fn page(store: Store) -> View {
    let host = store.host();
    host.hardening_opened();
    // Leaving the page gives up a registration that waits for its captcha, closing its window.
    cleanup(move || host.cancel_owner());
    view()
        .gap_6()
        .child(toolbar().child(tally(store)))
        .child(release_notice(store))
        .child(
            when(move || idle(host), move || nothing_yet(store)).otherwise(move || checking(store)),
        )
}

/// The page header's actions: Harden now, and the way on to the console once every check passed.
pub fn actions(store: Store) -> AnyElement {
    let host = store.host();
    dynamic(move || {
        if idle(host) {
            return None;
        }
        // Read once per render and passed as constants, never bound into the buttons.
        let hardening = host.busy() == Busy::Hardening;
        let blocked = host.busy() != Busy::Idle;
        let console = host.checks().all_pass() && host.is_hosting();
        Some(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(console.then(|| {
                    action_button(
                        ButtonKind::Secondary,
                        store.tr(|s| s.host.manage_in_console),
                        move || host.manage_in_console(),
                    )
                }))
                .child(
                    busy_button(
                        ButtonKind::Primary,
                        store.tr(|s| s.host.harden_now),
                        move || hardening,
                        move || host.harden(),
                    )
                    .disabled(blocked),
                ),
        )
    })
}

/// Nothing is hosted: what this page will check, and the way to start.
fn nothing_yet(store: Store) -> View {
    group().child(empty_panel(
        Icon::Shield,
        store.tr(|s| s.hardening_idle_title),
        store.tr(|s| s.hardening_idle_body),
        Some(
            action_button(
                ButtonKind::Primary,
                store.tr(|s| s.host.open_status),
                move || store.open(Page::HostStatus),
            )
            .into_element(),
        ),
    ))
}

/// The page once a control plane exists.
fn checking(store: Store) -> View {
    let host = store.host();
    view()
        .gap_6()
        .child(callout(
            CalloutKind::Info,
            store.tr(|s| s.host.harden_intro),
        ))
        .child(checklist(store))
        .child(when(
            move || !store.secrets_persistent(),
            move || {
                callout(
                    CalloutKind::Warning,
                    store.tr(|s| s.common.secrets_not_kept),
                )
            },
        ))
        .child(passwords(store))
        .child(dynamic(move || {
            host.problem()
                .map(|problem| problem_notice(store, &problem))
        }))
        .child(dynamic(move || match host.hardening() {
            HardeningState::Done(outcome) => Some(result(store, &outcome)),
            HardeningState::Idle | HardeningState::Running => None,
        }))
        .child(owner(store))
        .child(reveal_drawer(store))
}

/// "3 of 4 checks done", with a lamp.
fn tally(store: Store) -> View {
    let host = store.host();
    let lamp = move || {
        let (done, total) = host.checks().passed();
        if idle(host) {
            Lamp::Idle
        } else if done == total {
            Lamp::Good
        } else {
            Lamp::Degraded
        }
    };
    status_lamp(lamp, move || {
        if idle(host) {
            return SharedString::from(store.strings().host.check_unknown);
        }
        let (done, total) = host.checks().passed();
        SharedString::from(fill(
            store.strings().host.checks_summary,
            &[("done", &done.to_string()), ("total", &total.to_string())],
        ))
    })
}

/// The lamp and word of a check that depends on what happened.
fn check_state(store: Store, check: impl Fn() -> Check + Copy + 'static) -> View {
    let lamp = move || match check() {
        Check::Pass => Lamp::Good,
        Check::Open => Lamp::Degraded,
        Check::Unknown => Lamp::Idle,
    };
    let word = move || {
        let strings = store.strings();
        match check() {
            Check::Pass => strings.host.check_done,
            Check::Open => strings.host.check_open,
            Check::Unknown => strings.host.check_unknown,
        }
    };
    status_lamp(lamp, word)
}

/// What explains a check, under its row.
fn explanation(store: Store, detail: Pick) -> View {
    view().px_4().pb_3().child(meta_text(store.tr(detail)))
}

/// A button in the actions column, found by `id`.
fn row_action(
    label: impl IntoReactive<SharedString>,
    id: &str,
    on_press: impl Fn() + 'static,
) -> View {
    view()
        .test_id(id)
        .child(action_button(ButtonKind::Secondary, label, on_press))
}

/// The six checks as a table: what is checked, how it stands (a lamp with a word), and the action that fixes it;
/// each row's explanation sits under it. Three depend on what happened, three are facts about how the control
/// plane is built.
fn checklist(store: Store) -> View {
    let host = store.host();
    let cols = columns(vec![
        column(store.tr(|s| s.host.col_check)).flex(),
        column(store.tr(|s| s.host.col_status)).width(200.0),
        column("").width(160.0).actions(),
    ]);
    // The owner row offers its action while the account is missing; read once per render, as constants.
    let register = dynamic(move || {
        let missing = host.checks().owner != Check::Pass;
        let blocked = host.busy() != Busy::Idle || host.owner_step() != OwnerStep::Idle;
        missing.then(|| {
            view().test_id("check-owner-action").child(
                action_button(
                    ButtonKind::Secondary,
                    store.tr(|s| s.host.action_register),
                    move || host.begin_owner(),
                )
                .disabled(blocked),
            )
        })
    });
    let settings = row_action(
        store.tr(|s| s.host.action_view_settings),
        "check-config-action",
        move || store.open(Page::HostStatus),
    );
    data_table(&cols)
        .row(
            table_row(&cols)
                .text(store.tr(|s| s.host.check_admin))
                .cell(check_state(store, move || host.checks().admin))
                .cell(view())
                .detail(explanation(store, |s| s.host.detail_admin))
                .test_id("check-admin"),
        )
        .row(
            table_row(&cols)
                .text(store.tr(|s| s.host.check_user))
                .cell(check_state(store, move || host.checks().user))
                .cell(view())
                .detail(explanation(store, |s| s.host.detail_user))
                .test_id("check-user"),
        )
        .row(
            table_row(&cols)
                .text(store.tr(|s| s.host.check_registration))
                .cell(check_state(store, move || host.checks().registration))
                .cell(view())
                .detail(explanation(store, |s| s.host.detail_registration))
                .test_id("check-registration"),
        )
        .row(
            table_row(&cols)
                .text(store.tr(|s| s.host.check_owner))
                .cell(check_state(store, move || host.checks().owner))
                .cell(register)
                .detail(explanation(store, |s| s.host.detail_owner))
                .test_id("check-owner"),
        )
        .row(
            table_row(&cols)
                .text(store.tr(|s| s.host.check_api))
                .cell(status_lamp(
                    || Lamp::Good,
                    store.tr(|s| s.host.check_api_ok),
                ))
                .cell(view())
                .detail(explanation(store, |s| s.host.detail_api))
                .test_id("check-api"),
        )
        .row(
            table_row(&cols)
                .text(store.tr(|s| s.host.check_config))
                .cell(status_lamp(
                    || Lamp::Degraded,
                    store.tr(|s| s.host.check_config_exposed),
                ))
                .cell(settings)
                .detail(explanation(store, |s| s.host.detail_config))
                .test_id("check-config"),
        )
        .test_id("hardening-checks")
        .build()
}

/// The two password fields (empty means "generate one") in a panel; the keychain's passwords are one press away.
fn passwords(store: Store) -> View {
    let host = store.host();
    let short =
        move |flag: bool| flag.then(|| SharedString::from(store.strings().host.password_too_short));
    let reveal = when(
        move || {
            let stored = host.stored_passwords();
            stored.admin || stored.user
        },
        move || {
            view().test_id("host-reveal").child(action_button(
                ButtonKind::Ghost,
                store.tr(|s| s.host.reveal),
                move || host.open_reveal(),
            ))
        },
    );
    panel(
        store.tr(|s| s.host.passwords_label),
        Some(reveal.into_element()),
    )
    .child(
        view()
            .p_4()
            .gap_4()
            .child(meta_text(store.tr(|s| s.host.passwords_help)))
            .child(
                secret_field(
                    store.tr(|s| s.host.field_admin_password),
                    move || host.admin_input().expose().to_owned(),
                    move |text| host.set_admin_input(text),
                    secret_labels(store),
                )
                .generate(store.tr(|s| s.common.generate), move || {
                    host.generate_admin()
                })
                .error(move || short(host.input_errors().admin_short))
                .test_id("harden-admin"),
            )
            .child(
                secret_field(
                    store.tr(|s| s.host.field_user_password),
                    move || host.user_input().expose().to_owned(),
                    move |text| host.set_user_input(text),
                    secret_labels(store),
                )
                .generate(store.tr(|s| s.common.generate), move || {
                    host.generate_user()
                })
                .error(move || short(host.input_errors().user_short))
                .test_id("harden-user"),
            ),
    )
}

/// The owner account: the captcha to read and type, then the enrollment address to give to machines.
fn owner(store: Store) -> View {
    let host = store.host();
    let step = dynamic(move || {
        let step = host.owner_step();
        match step {
            OwnerStep::Idle if host.owner_registered() => owner_done(store),
            OwnerStep::Idle | OwnerStep::Preparing => {
                owner_start(store, step == OwnerStep::Preparing)
            }
            OwnerStep::Waiting { captcha } => owner_captcha(store, captcha, false),
            OwnerStep::Submitting { captcha } => owner_captcha(store, captcha, true),
        }
    });
    panel(store.tr(|s| s.host.owner_label), None).child(view().p_4().gap_4().child(step).child(
        dynamic(move || {
            host.owner_problem()
                .map(|problem| owner_notice(store, problem.message(), problem.detail()))
        }),
    ))
}

/// What the owner account is, and the button that begins its registration.
fn owner_start(store: Store, preparing: bool) -> View {
    let host = store.host();
    let blocked = host.busy() != Busy::Idle;
    view()
        .gap_3()
        .child(meta_text(store.tr(|s| s.host.owner_intro)))
        .child(
            view().flex_row().items_center().gap_3().child(
                view().test_id("owner-register").child(
                    busy_button(
                        ButtonKind::Primary,
                        store.tr(|s| s.host.owner_register),
                        move || preparing,
                        move || host.begin_owner(),
                    )
                    .disabled(blocked),
                ),
            ),
        )
        .child(preparing.then(|| meta_text(store.tr(|s| s.host.owner_preparing))))
}

/// The captcha image, the field for its text and the buttons: `submitting` while the text is being sent.
fn owner_captcha(store: Store, captcha: String, submitting: bool) -> View {
    let host = store.host();
    let blocked = submitting || host.busy() != Busy::Idle;
    view()
        .gap_4()
        .child(
            image(captcha)
                .w(Points(200.0))
                .h(Points(72.0))
                .resize_mode(ResizeMode::Contain)
                .label(store.tr(|s| s.host.owner_captcha_image))
                .test_id("owner-captcha-image"),
        )
        .child(
            field(
                store.tr(|s| s.host.owner_captcha_label),
                move || host.captcha_input(),
                move |text| host.set_captcha_input(text),
            )
            .mono()
            .helper(store.tr(|s| s.host.owner_captcha_help))
            .test_id("owner-captcha"),
        )
        .child(
            dialog_footer()
                .child(
                    view().test_id("owner-cancel").child(
                        action_button(
                            ButtonKind::Ghost,
                            store.tr(|s| s.common.cancel),
                            move || host.cancel_owner(),
                        )
                        .disabled(blocked),
                    ),
                )
                .child(
                    view().test_id("owner-submit").child(
                        busy_button(
                            ButtonKind::Primary,
                            store.tr(|s| s.host.owner_submit),
                            move || submitting,
                            move || host.submit_captcha(),
                        )
                        .disabled(blocked),
                    ),
                ),
        )
}

/// The registered owner account and the enrollment address, a secret: masked, shown or copied on request.
fn owner_done(store: Store) -> View {
    let host = store.host();
    let needs_address = host
        .enrollment()
        .is_some_and(|enrollment| enrollment.needs_address);
    view()
        .gap_4()
        .child(callout(CalloutKind::Info, store.tr(|s| s.host.owner_done)).test_id("owner-done"))
        .child(
            secret_field(
                store.tr(|s| s.host.owner_url_label),
                move || {
                    host.enrollment()
                        .map(|enrollment| enrollment.url.expose().to_owned())
                        .unwrap_or_default()
                },
                // The address is read-only: an edit is dropped, the field shows what the keychain holds.
                |_| {},
                secret_labels(store),
            )
            .test_id("owner-url"),
        )
        .child(meta_text(store.tr(|s| s.host.owner_url_note)))
        .child(needs_address.then(|| {
            callout(
                CalloutKind::Warning,
                store.tr(|s| s.host.owner_url_placeholder),
            )
            .test_id("owner-url-placeholder")
        }))
}

/// What the run did: how it ended, and where the new passwords are.
fn result(store: Store, outcome: &HardeningOutcome) -> View {
    let kind = if outcome.complete {
        CalloutKind::Info
    } else {
        CalloutKind::Warning
    };
    let message: Pick = if outcome.complete && outcome.already_hardened {
        |s| s.host.result_already
    } else if outcome.complete {
        |s| s.host.result_complete
    } else {
        |s| s.host.result_stopped
    };
    let stored_admin: Pick = if outcome.admin_stored {
        |s| s.host.stored_admin
    } else {
        |s| s.host.not_stored_admin
    };
    let stored_user: Pick = if outcome.user_stored {
        |s| s.host.stored_user
    } else {
        |s| s.host.not_stored_user
    };
    view()
        .gap_2()
        .child(callout(kind, store.tr(message)).test_id("hardening-result"))
        .child(
            outcome
                .error
                .as_ref()
                .map(|problem| problem_notice(store, problem)),
        )
        .child(meta_text(store.tr(stored_admin)))
        .child(meta_text(store.tr(stored_user)))
}

/// The drawer that shows the passwords the keychain holds: opened only when the person asks, closed they are
/// forgotten.
fn reveal_drawer(store: Store) -> AnyElement {
    let host = store.host();
    drawer(
        move || host.reveal_open(),
        store.tr(|s| s.host.reveal_label),
        store.tr(|s| s.common.close),
        move || host.close_reveal(),
    )
    .kind(store.tr(|s| s.host.kind_control_plane))
    .body(move || {
        view()
            .gap_4()
            .child(meta_text(store.tr(|s| s.host.reveal_note)))
            .child(dynamic(move || {
                let Some(revealed) = host.revealed() else {
                    return meta_text(store.tr(|s| s.common.loading)).into_element();
                };
                let fact = |label: Pick, secret: Option<crate::core::secret::Secret>, id: &str| {
                    match secret {
                        Some(secret) => fact_mono(
                            store.tr(label),
                            SharedString::from(secret.expose().to_owned()),
                        )
                        .copyable(copy_labels(store))
                        .test_id(id.to_owned()),
                        None => fact_mono(store.tr(label), store.tr(|s| s.host.reveal_none)),
                    }
                };
                kv_grid(vec![
                    fact(|s| s.host.reveal_admin, revealed.admin, "reveal-admin"),
                    fact(|s| s.host.reveal_user, revealed.user, "reveal-user"),
                ])
                .into_element()
            }))
    })
    .footer(move || {
        dialog_footer().child(action_button(
            ButtonKind::Secondary,
            store.tr(|s| s.common.close),
            move || host.close_reveal(),
        ))
    })
    .test_id("host-reveal-drawer")
    .into_element()
}
