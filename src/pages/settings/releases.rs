//! Settings → EasyTier → Available releases: what GitHub lists as a table (version, date, size, digest check, Install)
//! with the phase of the install that runs.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::programs::{named, problem_view, working};
use crate::{
    components::{
        ButtonKind, Columns, Lamp, action_button, column, columns, data_table, group, group_row,
        machine_text, meta_text, neutral_chip, section_label, status_chip, status_lamp, table_row,
    },
    i18n::Pick,
    store::{
        Store,
        versions::{
            AvailableRelease, InstallProgress, InstallStep, ReleaseBlock, ReleaseList, Standing,
            format_size,
        },
    },
};

/// The date column leaves the table below this window width, in points.
const DATE_MIN_WINDOW: f32 = 900.0;

/// The steps of an install, in order: the progress strip fills one segment per step.
const STEPS: [InstallStep; 4] = [
    InstallStep::Starting,
    InstallStep::Downloading,
    InstallStep::Verifying,
    InstallStep::Unpacking,
];

/// The releases GitHub lists, with Install.
pub(super) fn releases(store: Store) -> View {
    let versions = store.versions();
    let body = dynamic(move || match versions.releases() {
        ReleaseList::NotLoaded | ReleaseList::Loading => {
            state_panel(status_lamp(|| Lamp::Busy, store.tr(|s| s.common.loading))).into_element()
        }
        ReleaseList::Failed(problem) => problem_view(
            store,
            &problem,
            Some((
                |s| s.common.retry,
                Box::new(move || versions.load_releases()),
            )),
        )
        .into_element(),
        ReleaseList::Ready(list) if list.is_empty() => {
            state_panel(meta_text(store.tr(|s| s.versions.releases_empty))).into_element()
        }
        ReleaseList::Ready(list) => table(store, list).into_element(),
    });
    view()
        .gap_2()
        .child(section_label(store.tr(|s| s.versions.releases_label)))
        .child(meta_text(store.tr(|s| s.versions.releases_intro)))
        .child(body)
}

/// A panel holding one line that says what the list is doing.
fn state_panel(content: impl IntoElement) -> View {
    group().child(group_row(false).child(content))
}

/// The table of `list`.
fn table(store: Store, list: Vec<AvailableRelease>) -> View {
    let versions = store.versions();
    let cols = columns(vec![
        column(store.tr(|s| s.settings_version)).flex(),
        column(store.tr(|s| s.versions.col_published))
            .width(96.0)
            .mono()
            .hide_below(DATE_MIN_WINDOW),
        column(store.tr(|s| s.versions.col_size))
            .width(80.0)
            .mono()
            .right(),
        column(store.tr(|s| s.versions.col_verification)).width(210.0),
        column(SharedString::from("")).width(190.0).actions(),
    ]);
    let installed: Vec<String> = versions
        .installed()
        .into_iter()
        .filter(|release| !release.custom)
        .map(|release| release.number)
        .collect();
    let update = versions
        .latest()
        .filter(|latest| latest.standing == Standing::Update)
        .map(|latest| latest.release.tag);
    let progress = versions.progress();
    let busy = working(store)();
    let rows: Vec<View> = list
        .into_iter()
        .map(|release| {
            let facts = Facts {
                installed: installed.contains(&release.number),
                progress: progress
                    .clone()
                    .filter(|progress| progress.tag == release.tag),
                update: update.as_deref() == Some(release.tag.as_str()),
                busy,
            };
            release_row(store, &cols, release, facts)
        })
        .collect();
    data_table(&cols)
        .body(view().child(rows))
        .test_id("easytier-releases")
        .build()
}

/// What a row of the table needs to know besides its release.
struct Facts {
    /// The release is on this device.
    installed: bool,
    /// The install of this release that runs.
    progress: Option<InstallProgress>,
    /// Installing it is the update the page offers.
    update: bool,
    /// Something installs or switches: nothing else may start.
    busy: bool,
}

/// The word for a step of an install.
fn step_word(step: InstallStep) -> Pick {
    match step {
        InstallStep::Starting => |s| s.versions.phase_starting,
        InstallStep::Downloading => |s| s.versions.phase_downloading,
        InstallStep::Verifying => |s| s.versions.phase_verifying,
        InstallStep::Unpacking => |s| s.versions.phase_unpacking,
    }
}

/// One listed release: its version, date, size and how it is verified; then Install, or why it cannot be.
fn release_row(store: Store, cols: &Columns, release: AvailableRelease, facts: Facts) -> View {
    let versions = store.versions();
    let tag = release.tag.clone();
    let version = view()
        .flex_row()
        .items_center()
        .gap_2()
        .min_w_0()
        .child(
            machine_text(SharedString::from(release.version.clone()))
                .number_of_lines(1)
                .text_color(move || theme().foreground),
        )
        .child(
            release
                .prerelease
                .then(|| neutral_chip(store.tr(|s| s.versions.prerelease))),
        );
    let verification = view().flex_row().items_center().child(
        release
            .verified
            .then(|| status_chip(|| Lamp::Good, store.tr(|s| s.versions.verified))),
    );
    let mut detail = None;
    let action: AnyElement = if facts.installed {
        neutral_chip(store.tr(|s| s.versions.installed_badge)).into_element()
    } else if let Some(progress) = facts.progress {
        detail = Some(progress_strip(progress.step));
        status_lamp(|| Lamp::Busy, store.tr(step_word(progress.step))).into_element()
    } else if let Some(block) = release.block {
        let why: Pick = match block {
            ReleaseBlock::TooOld => |s| s.versions.blocker_too_old,
            ReleaseBlock::NoDownload => |s| s.versions.blocker_no_download,
            ReleaseBlock::NoDigest => |s| s.versions.blocker_no_digest,
        };
        detail = Some(
            view()
                .px_4()
                .pb_2()
                .child(meta_text(store.tr(why)))
                .into_element(),
        );
        view().into_element()
    } else {
        let kind = if facts.update {
            ButtonKind::Primary
        } else {
            ButtonKind::Secondary
        };
        let busy = facts.busy;
        action_button(kind, store.tr(|s| s.versions.install), move || {
            versions.install(&tag);
        })
        .accessibility_label(named(
            store,
            |s| s.versions.install_named,
            release.version.clone(),
        ))
        .disabled(move || busy)
        .into_element()
    };
    let row = table_row(cols)
        .cell(version)
        .text(SharedString::from(release.published.clone()))
        .text(SharedString::from(
            release.size.map(format_size).unwrap_or_default(),
        ))
        .cell(verification)
        .cell(view().flex_row().items_center().child(action))
        .test_id(format!("release-{}", release.number));
    match detail {
        Some(detail) => row.detail(detail).build(),
        None => row.build(),
    }
}

/// The install's four steps as a strip under the row: one segment per step, filled up to the one that runs.
fn progress_strip(step: InstallStep) -> AnyElement {
    let reached = STEPS
        .iter()
        .position(|candidate| *candidate == step)
        .map_or(0, |index| index.saturating_add(1));
    let segments: Vec<View> = STEPS
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let filled = index < reached;
            view().flex_1().h(Points(2.0)).rounded_full().bg(move || {
                if filled {
                    theme().primary
                } else {
                    theme().muted
                }
            })
        })
        .collect();
    view()
        .flex_row()
        .gap_1()
        .px_4()
        .pb_2()
        .accessible(false)
        .child(segments)
        .into_element()
}
