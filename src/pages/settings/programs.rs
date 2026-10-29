//! Settings → EasyTier: which release runs, the releases on this device and on GitHub, a release found on disk.
//! On the desktops the app installs and switches the official programs; on iOS and Android the library is part of
//! the app; in the browser the server's programs change through its deployment files.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{adopt, installed, releases, section};
use crate::{
    components::{
        ButtonKind, CalloutKind, ICON_SIZE, Icon, action_button, app_color, callout, group,
        group_row, icon_colored, machine_text, meta_text, summary_cell, summary_strip,
    },
    core::platform::{Availability, Backend},
    i18n::{Pick, fill},
    store::{
        Page, Store,
        versions::{ActiveRelease, Standing, VersionProblem},
    },
};

/// The EasyTier section for this platform.
pub(super) fn programs(store: Store) -> View {
    match store.capabilities().node {
        Availability::Local(Backend::Sidecar) => desktop(store),
        Availability::Local(Backend::Embedded) => embedded(store),
        Availability::Local(_) | Availability::Elsewhere(_) => server(store),
    }
}

/// A row of a grouped list: a label, then a value in the monospace.
pub(super) fn value_row(
    separated: bool,
    label: impl IntoReactive<SharedString>,
    value: impl IntoReactive<SharedString>,
) -> View {
    group_row(separated)
        .flex_wrap()
        .child(
            text(label)
                .text_sm()
                .flex_1()
                .text_color(move || theme().card_foreground),
        )
        .child(machine_text(value).text_color(move || theme().muted_foreground))
}

/// The label of a button that is one of many, for screen readers and tests: `template` with `{version}`.
pub(super) fn named(
    store: Store,
    pick: Pick,
    version: String,
) -> impl Fn() -> Option<SharedString> + 'static {
    move || {
        Some(SharedString::from(fill(
            pick(store.strings()),
            &[("version", &version)],
        )))
    }
}

/// Something is installing or switching: nothing else may start.
pub(super) fn working(store: Store) -> impl Fn() -> bool + Copy {
    let versions = store.versions();
    move || versions.switching() || versions.progress().is_some()
}

/// A problem in words, its machine data apart, and a Close action.
pub(super) fn problem_view(
    store: Store,
    problem: &VersionProblem,
    retry: Option<(Pick, Box<dyn Fn()>)>,
) -> View {
    let mut notice = callout(CalloutKind::Danger, store.tr(problem.message()));
    if let Some((label, on_press)) = retry {
        notice = notice.action(store.tr(label), on_press);
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

// --- iOS and Android -------------------------------------------------------------------------------------------

/// The embedded library: its version, and that it updates with the app.
fn embedded(store: Store) -> View {
    let version = store.versions().embedded_version().unwrap_or_default();
    section(
        Icon::Server,
        store.tr(|s| s.versions.section),
        view()
            .gap_2()
            .child(group().child(value_row(
                false,
                store.tr(|s| s.versions.embedded_label),
                SharedString::from(version),
            )))
            .child(meta_text(store.tr(|s| s.versions.embedded_note))),
    )
}

// --- The web build ---------------------------------------------------------------------------------------------

/// The server's programs change through deployment files, not from here.
fn server(store: Store) -> View {
    section(
        Icon::Server,
        store.tr(|s| s.versions.section),
        group().child(
            group_row(false)
                .flex_wrap()
                .gap_3()
                .child(icon_colored(Icon::Info, ICON_SIZE, move || {
                    app_color("primary")
                }))
                .child(
                    text(store.tr(|s| s.versions.web_note))
                        .text_sm()
                        .flex_1()
                        .text_color(move || theme().card_foreground),
                )
                .child(action_button(
                    ButtonKind::Secondary,
                    store.tr(|s| s.versions.web_action),
                    move || store.open(Page::Deployment),
                )),
        ),
    )
}

// --- The desktops ----------------------------------------------------------------------------------------------

/// The desktops' section: install, switch, roll back, adopt.
fn desktop(store: Store) -> View {
    let versions = store.versions();
    versions.opened();
    section(
        Icon::Server,
        store.tr(|s| s.versions.section),
        view()
            .gap_6()
            .child(status(store))
            .child(fresh(store))
            .child(installed::installed(store))
            .child(releases::releases(store))
            .child(adopt::adopt(store)),
    )
}

/// The last failure of an action, the facts about the active release, and what to do when there is none.
fn status(store: Store) -> View {
    let versions = store.versions();
    view()
        .gap_3()
        .child(dynamic(move || {
            versions.problem().map(|problem| {
                problem_view(
                    store,
                    &problem,
                    Some((
                        |s| s.common.close,
                        Box::new(move || versions.dismiss_problem()),
                    )),
                )
            })
        }))
        .child(summary(store))
        .child(when(
            move || matches!(versions.active(), ActiveRelease::Missing),
            move || {
                callout(CalloutKind::Info, store.tr(|s| s.versions.missing_body))
                    .test_id("easytier-missing")
            },
        ))
        .child(when(
            move || matches!(versions.active(), ActiveRelease::Unsupported { .. }),
            move || {
                callout(
                    CalloutKind::Warning,
                    store.tr(|s| s.versions.unsupported_body),
                )
            },
        ))
        .child(dynamic(move || match versions.active() {
            ActiveRelease::Failed(problem) => Some(problem_view(store, &problem, None)),
            ActiveRelease::Checking
            | ActiveRelease::Ready { .. }
            | ActiveRelease::Missing
            | ActiveRelease::Unsupported { .. }
            | ActiveRelease::Unmanaged => None,
        }))
}

/// The summary strip: the active release, how many are installed, the newest one GitHub lists and the oldest the
/// app supports. A cell with nothing to report is left out.
fn summary(store: Store) -> AnyElement {
    let versions = store.versions();
    dynamic(move || {
        let strings = store.strings();
        let active = versions.active();
        let installed = versions.installed().len();
        let latest = versions.latest();

        let (value, caption, has_version) = match active {
            ActiveRelease::Ready { version } => {
                (version, Some(strings.versions.status_ready), true)
            }
            ActiveRelease::Unsupported { version } => {
                (version, Some(strings.versions.status_unsupported), true)
            }
            ActiveRelease::Checking => (strings.common.loading.to_owned(), None, false),
            ActiveRelease::Missing | ActiveRelease::Unmanaged => {
                (strings.versions.status_missing.to_owned(), None, false)
            }
            ActiveRelease::Failed(_) => {
                (strings.versions.status_unreadable.to_owned(), None, false)
            }
        };
        let mut active_cell =
            summary_cell(strings.versions.active_label, SharedString::from(value));
        if let Some(caption) = caption {
            active_cell = active_cell.caption(caption);
        }
        if has_version {
            active_cell = active_cell.test_id("easytier-active-version");
        }

        let mut cells = vec![active_cell];
        if installed > 0 {
            cells.push(
                summary_cell(
                    strings.versions.installed_summary,
                    SharedString::from(installed.to_string()),
                )
                .unit(strings.versions.unit_releases),
            );
        }
        if let Some(latest) = latest {
            let mut cell = summary_cell(
                strings.versions.latest_label,
                SharedString::from(latest.release.version),
            );
            match latest.standing {
                Standing::Update => cell = cell.caption(strings.versions.update_available),
                Standing::UpToDate => cell = cell.caption(strings.versions.up_to_date),
                Standing::Unrelated => {}
            }
            cells.push(cell);
        }
        cells.push(summary_cell(
            strings.versions.oldest_label,
            SharedString::from(versions.oldest_supported()),
        ));
        Some(summary_strip(cells).test_id("easytier-active"))
    })
    .into_element()
}

/// A release that was just installed and waits to be activated.
fn fresh(store: Store) -> AnyElement {
    let versions = store.versions();
    dynamic(move || {
        let number = versions.fresh()?;
        let target = number.clone();
        let busy = working(store)();
        Some(
            group().test_id("easytier-fresh").child(
                group_row(false)
                    .flex_wrap()
                    .child(icon_colored(Icon::Check, ICON_SIZE, move || {
                        app_color("success")
                    }))
                    .child(
                        machine_text(SharedString::from(number.clone()))
                            .text_color(move || theme().foreground),
                    )
                    .child(
                        text(store.tr(|s| s.versions.installed_ready))
                            .text_sm()
                            .flex_1()
                            .text_color(move || theme().card_foreground),
                    )
                    .child(
                        action_button(
                            ButtonKind::Primary,
                            store.tr(|s| s.versions.activate),
                            move || {
                                versions.activate(&target);
                            },
                        )
                        .accessibility_label(named(store, |s| s.versions.activate_named, number))
                        .disabled(move || busy),
                    ),
            ),
        )
    })
    .into_element()
}
