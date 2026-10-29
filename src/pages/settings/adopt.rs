//! Settings → EasyTier → releases already on this device: one found on the PATH, or in a directory the person types.
//! Both are checked first and used only after a confirmation that names the version and the path.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::programs::{named, problem_view, working};
use crate::{
    components::{
        ButtonKind, Lamp, action_button, busy_button, field, group, group_row, machine_text,
        meta_text, section_label, status_chip, status_lamp,
    },
    store::{
        Store,
        versions::{Candidate, Detection, DirCheck},
    },
};

/// The narrowest a panel gets before it wraps under its neighbour, in points.
const PANEL_MIN_WIDTH: f32 = 320.0;

/// The PATH panel and the custom-directory panel, side by side while there is room.
pub(super) fn adopt(store: Store) -> View {
    view()
        .flex_row()
        .flex_wrap()
        .items_start()
        .gap_6()
        .child(
            view()
                .flex_1()
                .min_w(Points(PANEL_MIN_WIDTH))
                .gap_2()
                .child(section_label(store.tr(|s| s.versions.path_label)))
                .child(path(store)),
        )
        .child(
            view()
                .flex_1()
                .min_w(Points(PANEL_MIN_WIDTH))
                .gap_2()
                .child(section_label(store.tr(|s| s.versions.custom_label)))
                .child(custom(store)),
        )
}

/// A release found on disk as two rows of a panel: its version with the action that asks before using it (or the
/// word that it is too old), then its directory.
fn candidate_rows(store: Store, candidate: Candidate, test_id: &str, separated: bool) -> View {
    let versions = store.versions();
    let supported = candidate.supported;
    let busy = working(store)();
    let name = candidate.version.clone();
    let chosen = candidate.clone();
    let action: AnyElement = if supported {
        action_button(
            ButtonKind::Secondary,
            store.tr(|s| s.versions.adopt),
            move || {
                versions.adopt(&chosen);
            },
        )
        .accessibility_label(named(store, |s| s.versions.adopt_named, name))
        .disabled(move || busy)
        .into_element()
    } else {
        status_chip(|| Lamp::Failed, store.tr(|s| s.versions.adopt_too_old)).into_element()
    };
    view()
        .child(
            group_row(separated)
                .flex_wrap()
                .test_id(test_id)
                .child(
                    text(store.tr(|s| s.settings_version))
                        .text_sm()
                        .text_color(move || theme().card_foreground),
                )
                .child(
                    machine_text(SharedString::from(candidate.version))
                        .flex_1()
                        .text_color(move || theme().foreground),
                )
                .child(action),
        )
        .child(
            group_row(true)
                .child(
                    text(store.tr(|s| s.versions.directory_label))
                        .text_sm()
                        .text_color(move || theme().card_foreground),
                )
                .child(
                    machine_text(SharedString::from(candidate.dir))
                        .number_of_lines(1)
                        .min_w_0()
                        .flex_1()
                        .text_right()
                        .selectable(true)
                        .text_color(move || theme().muted_foreground),
                ),
        )
}

/// A release on the PATH, offered unless it is the one in use already.
fn path(store: Store) -> AnyElement {
    let versions = store.versions();
    dynamic(move || match versions.detection() {
        Detection::Unchecked | Detection::Searching => group()
            .child(group_row(false).child(status_lamp(
                || Lamp::Busy,
                store.tr(|s| s.versions.path_searching),
            )))
            .into_element(),
        Detection::NotFound => group()
            .child(group_row(false).child(meta_text(store.tr(|s| s.versions.path_none))))
            .into_element(),
        Detection::Failed(problem) => problem_view(store, &problem, None).into_element(),
        Detection::Found(candidate) => {
            let in_use = versions
                .installed()
                .iter()
                .any(|release| release.active && release.dir == candidate.dir);
            if in_use {
                group()
                    .child(group_row(false).child(status_chip(
                        || Lamp::Good,
                        store.tr(|s| s.versions.badge_active),
                    )))
                    .into_element()
            } else {
                group()
                    .child(candidate_rows(store, candidate, "easytier-path", false))
                    .into_element()
            }
        }
    })
    .into_element()
}

/// A directory the person typed: checked first, then offered like a release on the PATH.
fn custom(store: Store) -> View {
    let versions = store.versions();
    let form = view()
        .p_4()
        .gap_3()
        .child(
            field(
                store.tr(|s| s.versions.custom_field),
                move || versions.custom_dir(),
                move |text| versions.set_custom_dir(text),
            )
            .helper(store.tr(|s| s.versions.custom_help))
            .mono()
            .test_id("easytier-custom-dir"),
        )
        .child(view().flex_row().child(busy_button(
            ButtonKind::Secondary,
            store.tr(|s| s.versions.custom_check),
            move || matches!(versions.custom_check(), DirCheck::Checking),
            move || versions.check_custom_dir(),
        )));
    view()
        .gap_2()
        .child(
            group()
                .child(form)
                .child(dynamic(move || match versions.custom_check() {
                    DirCheck::Found(candidate) => {
                        Some(candidate_rows(store, candidate, "easytier-custom", true))
                    }
                    DirCheck::Idle | DirCheck::Checking | DirCheck::Failed(_) => None,
                })),
        )
        .child(dynamic(move || match versions.custom_check() {
            DirCheck::Failed(problem) => Some(problem_view(store, &problem, None)),
            DirCheck::Idle | DirCheck::Checking | DirCheck::Found(_) => None,
        }))
}
