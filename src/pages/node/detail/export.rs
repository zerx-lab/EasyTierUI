//! The Export tab: the saved profile as TOML, a share link and a QR code. The secret is part of it only when the
//! switch says so (off by default).

use nsh_core::prelude::*;

use super::super::common::copy_labels;
use crate::{
    components::{CalloutKind, callout, code_block, meta_text, qr_view, section_label, switch_row},
    store::{Store, node::ExportFailure},
};

/// What the export area shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Content {
    /// Being made (or not asked for yet).
    Loading,
    /// The keychain or the profile stopped it.
    Failed(ExportFailure),
    /// The config and the link are there.
    Ready,
}

/// The tab (its export is made when the tab opens).
pub(super) fn tab(store: Store) -> View {
    let node = store.node();
    let export = node.export();
    let content = computed(move || {
        export.with(|view| {
            if view.loading {
                return Content::Loading;
            }
            match view.failure {
                Some(failure @ (ExportFailure::Keychain | ExportFailure::Invalid)) => {
                    Content::Failed(failure)
                }
                Some(ExportFailure::QrTooLong) | None => {
                    if view.toml.is_some() && view.link.is_some() {
                        Content::Ready
                    } else {
                        Content::Loading
                    }
                }
            }
        })
    });
    view()
        .gap_4()
        .child(
            switch_row(
                store.tr(|s| s.node.detail.export_include_secret),
                move || export.with(|view| view.include_secret),
                move |on| node.set_include_secret(on),
            )
            .description(store.tr(|s| s.node.detail.export_include_secret_help))
            .exposure(store.tr(|s| s.node.detail.export_secret_exposure))
            .test_id("export-include-secret"),
        )
        .child(dynamic(move || match content.get() {
            Content::Loading => meta_text(store.tr(|s| s.common.loading))
                .test_id("export-loading")
                .into_element(),
            Content::Failed(failure) => callout(CalloutKind::Danger, move || {
                let words = store.strings().node.detail;
                match failure {
                    ExportFailure::Keychain => words.export_keychain,
                    ExportFailure::Invalid | ExportFailure::QrTooLong => words.export_invalid,
                }
            })
            .test_id("export-failure")
            .into_element(),
            Content::Ready => untracked(|| ready(store)).into_element(),
        }))
}

/// The export itself: the config, the link and the QR code.
fn ready(store: Store) -> View {
    let node = store.node();
    let export = node.export();
    let qr = export.with_untracked(|view| view.qr.clone());
    let qr_too_long = export.with_untracked(|view| view.failure == Some(ExportFailure::QrTooLong));
    let qr_block = match qr {
        Some((size, modules)) => Some(
            view()
                .gap_2()
                .child(section_label(store.tr(|s| s.node.detail.export_qr)))
                .child(
                    view().flex_row().child(
                        qr_view(size, modules, store.tr(|s| s.node.detail.export_qr_label))
                            .test_id("export-qr"),
                    ),
                ),
        ),
        None => qr_too_long.then(|| {
            view()
                .gap_2()
                .child(section_label(store.tr(|s| s.node.detail.export_qr)))
                .child(
                    meta_text(store.tr(|s| s.node.detail.export_qr_too_long))
                        .test_id("export-qr-too-long"),
                )
        }),
    };
    view()
        .gap_4()
        .child(when(
            move || export.with(|view| view.secret_missing),
            move || {
                callout(
                    CalloutKind::Info,
                    store.tr(|s| s.node.detail.export_secret_missing),
                )
                .test_id("export-secret-missing")
            },
        ))
        .child(when(
            move || export.with(|view| !view.include_secret),
            move || {
                meta_text(store.tr(|s| s.node.detail.export_secret_left_out))
                    .test_id("export-secret-left-out")
            },
        ))
        .child(
            view()
                .gap_2()
                .child(section_label(store.tr(|s| s.node.detail.export_toml)))
                .child(code_block(
                    "export-toml",
                    move || {
                        export.with(|view| {
                            view.toml
                                .as_ref()
                                .map(|toml| toml.expose().to_owned())
                                .unwrap_or_default()
                        })
                    },
                    copy_labels(store),
                )),
        )
        .child(
            view()
                .gap_2()
                .child(section_label(store.tr(|s| s.node.detail.export_link)))
                .child(code_block(
                    "export-link",
                    move || {
                        export.with(|view| {
                            view.link
                                .as_ref()
                                .map(|link| link.expose().to_owned())
                                .unwrap_or_default()
                        })
                    },
                    copy_labels(store),
                ))
                .child(meta_text(store.tr(|s| s.node.detail.export_link_note))),
        )
        .child(qr_block)
}
