//! Settings → EasyTier → On this device: the installed releases as a table (version, state chips, directory, actions)
//! and the way back to the previous release.

use nsh_core::prelude::*;

use super::programs::{named, working};
use crate::{
    components::{
        ButtonKind, Columns, Icon, Lamp, action_button, column, columns, data_table, empty_panel,
        meta_text, neutral_chip, section_label, status_chip, status_lamp, table_row,
    },
    store::{
        Store,
        versions::{ActiveRelease, InstalledRelease},
    },
};

/// The directory column leaves the table below this window width, in points.
const DIRECTORY_MIN_WINDOW: f32 = 900.0;

/// The releases on this device, with Activate and Remove, and the way back to the previous one.
pub(super) fn installed(store: Store) -> View {
    let versions = store.versions();
    let cols = columns(vec![
        column(store.tr(|s| s.settings_version)).width(150.0).mono(),
        column(store.tr(|s| s.versions.col_status)).width(260.0),
        column(store.tr(|s| s.versions.directory_label))
            .flex()
            .mono()
            .hide_below(DIRECTORY_MIN_WINDOW),
        column(SharedString::from("")).width(190.0).actions(),
    ]);
    let rows_cols = cols.clone();
    let rows = dynamic(move || {
        let busy = working(store)();
        view().child(
            versions
                .installed()
                .into_iter()
                .map(|release| installed_row(store, &rows_cols, release, busy))
                .collect::<Vec<_>>(),
        )
    });
    let table = data_table(&cols)
        .body(rows)
        .empty(
            move || {
                versions.installed().is_empty()
                    && !matches!(versions.active(), ActiveRelease::Checking)
            },
            move || {
                empty_panel(
                    Icon::Download,
                    store.tr(|s| s.versions.installed_empty_title),
                    store.tr(|s| s.versions.installed_empty),
                    None,
                )
                .into_element()
            },
        )
        .test_id("easytier-installed");
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(section_label(store.tr(|s| s.versions.installed_label)))
                .child(view().flex_1())
                .child(rollback(store)),
        )
        .child(table)
        .child(when(
            move || versions.switching(),
            move || status_lamp(|| Lamp::Busy, store.tr(|s| s.versions.switching)),
        ))
        .child(meta_text(store.tr(|s| s.versions.switch_note)))
}

/// One installed release.
fn installed_row(store: Store, cols: &Columns, release: InstalledRelease, busy: bool) -> View {
    let versions = store.versions();
    let number = release.number.clone();
    let for_remove = release.number.clone();
    let can_activate = !release.active && !release.custom && release.usable;
    let can_remove = !release.active && !release.custom;
    let chips = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_1()
        .child(
            release
                .active
                .then(|| status_chip(|| Lamp::Good, store.tr(|s| s.versions.badge_active))),
        )
        .child(
            release
                .previous
                .then(|| neutral_chip(store.tr(|s| s.versions.badge_previous))),
        )
        .child(
            release
                .custom
                .then(|| neutral_chip(store.tr(|s| s.versions.badge_custom))),
        )
        .child(
            (!release.usable)
                .then(|| status_chip(|| Lamp::Failed, store.tr(|s| s.versions.badge_damaged))),
        );
    let actions = view()
        .flex_row()
        .items_center()
        .gap_1()
        .child(can_activate.then(|| {
            action_button(
                ButtonKind::Secondary,
                store.tr(|s| s.versions.activate),
                move || {
                    versions.activate(&number);
                },
            )
            .accessibility_label(named(
                store,
                |s| s.versions.activate_named,
                release.version.clone(),
            ))
            .disabled(move || busy)
        }))
        .child(can_remove.then(|| {
            action_button(
                ButtonKind::Ghost,
                store.tr(|s| s.versions.remove),
                move || {
                    versions.remove(&for_remove);
                },
            )
            .accessibility_label(named(
                store,
                |s| s.versions.remove_named,
                release.version.clone(),
            ))
            .disabled(move || busy)
        }));
    table_row(cols)
        .text(SharedString::from(release.version.clone()))
        .cell(chips)
        .text(SharedString::from(release.dir.clone()))
        .cell(actions)
        .test_id(format!("installed-{}", release.number))
        .build()
}

/// Back to the release before the active one.
fn rollback(store: Store) -> AnyElement {
    let versions = store.versions();
    dynamic(move || {
        let previous = versions
            .installed()
            .into_iter()
            .find(|release| release.previous)?;
        let busy = working(store)();
        Some(
            action_button(
                ButtonKind::Secondary,
                store.tr(|s| s.versions.rollback),
                move || {
                    versions.rollback();
                },
            )
            .accessibility_label(named(store, |s| s.versions.rollback_to, previous.version))
            .disabled(move || busy),
        )
    })
    .into_element()
}
