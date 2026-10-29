//! The VPN portal of the Forwarding tab: its kind, the clients connected right now and, only when asked for, the
//! ready-made client config. That config holds the client's WireGuard private key, so it is read on the press of a
//! button, shown with a warning and forgotten when hidden or when the tab is left.

use nsh_core::prelude::*;

use super::{loaded, tr};
use crate::{
    components::{
        ButtonKind, CalloutKind, CopyLabels, action_button, busy_button, callout, code_block,
        fact_mono, kv_grid, machine_text, meta_text,
    },
    store::{
        Store,
        console::{Scope, VpnSummary},
    },
};

/// The section: nothing to show for a network without a portal.
pub(super) fn section(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let body = loaded(
        store,
        move || console.vpn_portal(scope),
        move || console.refresh_vpn_portal(scope),
        move || portal(store, scope),
    );
    super::section(
        store,
        |c| c.vpn_title,
        Some(|c| c.vpn_hint),
        "vpn-section",
        vec![body],
    )
}

/// The portal when the network runs one, else a line saying it does not.
fn portal(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let enabled = computed(move || {
        console
            .vpn_portal(scope)
            .value
            .is_some_and(|portal| portal.is_enabled())
    });
    dynamic(move || {
        if enabled.get() {
            untracked(|| enabled_portal(store, scope))
        } else {
            meta_text(tr(store, |c| c.vpn_none)).into_element()
        }
    })
}

/// The portal's kind, its clients and the client config.
fn enabled_portal(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let summary = move || console.vpn_portal(scope).value.unwrap_or_default();
    view()
        .gap_3()
        .child(kv_grid(vec![
            fact_mono(tr(store, |c| c.vpn_type), move || summary().vpn_type).test_id("vpn-type"),
        ]))
        .child(clients(store, summary))
        .child(client_config(store, scope))
        .into_element()
}

/// Who is connected right now.
fn clients(store: Store, summary: impl Fn() -> VpnSummary + Copy + 'static) -> AnyElement {
    let list = computed(move || summary().connected_clients);
    view()
        .gap_1()
        .test_id("vpn-clients")
        .child(meta_text(move || {
            let count = list.with(|clients| clients.len());
            if count == 0 {
                store.strings().console_runtime.vpn_no_clients.to_owned()
            } else {
                super::filled(
                    store,
                    |c| c.vpn_clients,
                    &[("count", count.to_string().as_str())],
                )
            }
        }))
        .child(dynamic(move || {
            list.get()
                .into_iter()
                .map(|client| machine_text(SharedString::from(client)).into_element())
                .collect::<Vec<_>>()
        }))
        .into_element()
}

/// The client config: hidden behind a warning and a button until asked for.
fn client_config(store: Store, scope: Scope) -> AnyElement {
    let console = store.console();
    let shown = computed(move || console.revealed_vpn_config(scope).is_some());
    dynamic(move || {
        if shown.get() {
            let labels =
                CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied));
            view()
                .gap_2()
                .child(
                    callout(CalloutKind::Warning, tr(store, |c| c.vpn_config_shown))
                        .test_id("vpn-config-warning")
                        .build(),
                )
                .child(code_block(
                    "vpn-client-config",
                    move || {
                        console
                            .revealed_vpn_config(scope)
                            .map(|config| config.expose().to_owned())
                            .unwrap_or_default()
                    },
                    labels,
                ))
                .child(
                    view()
                        .flex_row()
                        .justify_end()
                        .test_id("vpn-config-hide")
                        .child(action_button(
                            ButtonKind::Ghost,
                            tr(store, |c| c.vpn_hide_config),
                            move || {
                                console.hide_vpn_config();
                            },
                        )),
                )
                .into_element()
        } else {
            view()
                .gap_2()
                .child(
                    callout(CalloutKind::Warning, tr(store, |c| c.vpn_config_secret))
                        .test_id("vpn-config-secret")
                        .build(),
                )
                .child(
                    view()
                        .flex_row()
                        .test_id("vpn-config-reveal")
                        .child(busy_button(
                            ButtonKind::Secondary,
                            tr(store, |c| c.vpn_show_config),
                            move || console.is_revealing_vpn_config(),
                            move || console.reveal_vpn_config(scope),
                        )),
                )
                .into_element()
        }
    })
}
