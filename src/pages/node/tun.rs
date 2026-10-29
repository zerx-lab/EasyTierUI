//! The confirmation before a network's first start with a virtual interface (TUN): the administrator prompt, the
//! service that keeps running after the app quits, and the alternative without TUN.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::common::tag;
use crate::{
    components::{ButtonKind, action_button, body_text, dialog_footer, dialog_frame, meta_text},
    store::Store,
};

/// The dialog, on screen while a start waits for the person's agreement.
pub(super) fn dialog(store: Store) -> If {
    let node = store.node();
    dialog_frame(
        move || node.tun_confirm().is_some(),
        store.tr(|s| s.node.flow.tun_title),
        move || node.cancel_tun(),
    )
    .body(move || {
        let name = move || {
            node.tun_confirm()
                .and_then(|id| node.profile(&id))
                .map(|profile| profile.name)
                .unwrap_or_default()
        };
        view()
            .gap_3()
            .child(
                text(name)
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(move || theme().foreground),
            )
            .child(body_text(store.tr(|s| s.node.flow.tun_admin)))
            .child(body_text(store.tr(|s| s.node.flow.tun_service)))
            .child(meta_text(store.tr(|s| s.node.flow.tun_alternative)))
    })
    .footer(move || {
        dialog_footer()
            .child(tag(
                action_button(
                    ButtonKind::Ghost,
                    store.tr(|s| s.common.cancel),
                    move || node.cancel_tun(),
                ),
                "tun-cancel",
            ))
            .child(tag(
                action_button(
                    ButtonKind::Secondary,
                    store.tr(|s| s.node.flow.tun_no_tun),
                    move || {
                        if let Some(id) = node.tun_confirm() {
                            node.start_without_tun(id);
                        }
                    },
                ),
                "tun-no-tun",
            ))
            .child(tag(
                action_button(
                    ButtonKind::Primary,
                    store.tr(|s| s.node.flow.tun_continue),
                    move || {
                        if let Some(id) = node.tun_confirm() {
                            node.confirm_tun(id);
                        }
                    },
                ),
                "tun-continue",
            ))
    })
    .test_id("tun-dialog")
    .build()
}
