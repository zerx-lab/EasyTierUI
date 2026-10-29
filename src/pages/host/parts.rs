//! Pieces the host pages share: panels, problem and release notices, copy labels, file sizes.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    components::{
        CalloutKind, CopyLabels, SecretLabels, callout, group, machine_text, section_label,
    },
    i18n::Pick,
    store::{Page, Store, host::HostProblem, versions::ActiveRelease},
};

/// A panel under construction: the panel fill and hairline of a grouped list with a header row (the label at the
/// start, `actions` at the end, a hairline under it). Add the body as children.
pub(super) fn panel(label: impl IntoReactive<SharedString>, actions: Option<AnyElement>) -> View {
    group().child(
        view()
            .flex_row()
            .items_center()
            .gap_2()
            .px_4()
            .py_2()
            .min_h_9()
            .border_b_1()
            .border_color(move || theme().border)
            .child(section_label(label).flex_1())
            .child(actions),
    )
}

/// The words of every Copy button on these pages.
pub(super) fn copy_labels(store: Store) -> CopyLabels {
    CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied))
}

/// The words of a secret field's Show, Hide and Copy buttons.
pub(super) fn secret_labels(store: Store) -> SecretLabels {
    SecretLabels::new(
        store.tr(|s| s.common.show),
        store.tr(|s| s.common.hide),
        copy_labels(store),
    )
}

/// A file's size for a table cell: bytes below 1 KiB, else KiB with one decimal (machine data, not translated).
pub(super) fn size_text(bytes: usize) -> String {
    const KIB: usize = 1024;
    if bytes < KIB {
        return format!("{bytes} B");
    }
    // Tenths of a KiB, rounded down.
    let tenths = bytes.saturating_mul(10) / KIB;
    format!("{}.{} KiB", tenths / 10, tenths % 10)
}

/// A failure in words, with its machine data (a port, an OS message) apart from them. When the way out is
/// Settings → EasyTier, the notice offers it.
pub(super) fn problem_notice(store: Store, problem: &HostProblem) -> View {
    let message = store.tr(problem.message());
    let mut notice = callout(CalloutKind::Danger, message).test_id("host-problem");
    if problem.needs_release() {
        notice = notice.action(store.tr(|s| s.host.open_settings), move || {
            store.open(Page::Settings);
        });
    }
    view()
        .gap_1()
        .child(notice)
        .child(problem.detail().map(|detail| {
            machine_text(SharedString::from(detail))
                .selectable(true)
                .text_color(move || theme().muted_foreground)
        }))
}

/// A failure of the owner-account step in words, with its machine data apart from them.
pub(super) fn owner_notice(store: Store, message: Pick, detail: Option<String>) -> View {
    view()
        .gap_1()
        .child(callout(CalloutKind::Danger, store.tr(message)).test_id("owner-problem"))
        .child(detail.map(|detail| {
            machine_text(SharedString::from(detail))
                .selectable(true)
                .text_color(move || theme().muted_foreground)
        }))
}

/// The notice that hosting needs an EasyTier release, shown while none is usable.
pub(super) fn release_notice(store: Store) -> AnyElement {
    dynamic(move || {
        let message: Option<Pick> = match store.versions().active() {
            ActiveRelease::Missing => Some(|s| s.host.core_missing),
            ActiveRelease::Unsupported { .. } => Some(|s| s.host.core_unsupported),
            ActiveRelease::Checking
            | ActiveRelease::Ready { .. }
            | ActiveRelease::Failed(_)
            | ActiveRelease::Unmanaged => None,
        };
        message.map(|message| {
            callout(CalloutKind::Info, store.tr(message))
                .action(store.tr(|s| s.host.open_settings), move || {
                    store.open(Page::Settings);
                })
                .test_id("host-release-notice")
        })
    })
}
