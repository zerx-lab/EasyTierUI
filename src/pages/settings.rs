//! Settings: the work mode, the UI's language, the desktop app's own behavior (tray, login, quiet start), the
//! EasyTier programs, and the app's version, as grouped panels with an icon each. The top row's panels sit side by
//! side on a wide window and stack on a narrow one; the EasyTier panel spans the page.

mod adopt;
mod desktop;
mod installed;
mod programs;
mod releases;

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    components::{
        ButtonKind, ICON_SIZE, Icon, action_button, card, choice_row, group, icon, machine_text,
        meta_text, switch_row,
    },
    core::role::{Role, WorkMode},
    i18n::{Language, LanguageChoice, Pick},
    store::{Page, Store},
};

/// The narrowest a panel of the top row gets before it wraps under its neighbour, in points.
const PANEL_MIN_WIDTH: f32 = 280.0;

/// The actions the settings page puts in the shell's page header: where the EasyTier programs are installed by the
/// app, looking again for releases (here, on the PATH and on GitHub). Elsewhere the page has none.
pub fn header_actions(store: Store, _page: Page) -> Option<AnyElement> {
    let versions = store.versions();
    if !versions.manages_programs() {
        return None;
    }
    Some(
        view()
            .test_id("settings-check-updates")
            .child(action_button(
                ButtonKind::Secondary,
                store.tr(|s| s.versions.check_updates),
                move || {
                    versions.refresh();
                    versions.detect_path();
                    versions.load_releases();
                },
            ))
            .into_element(),
    )
}

/// The settings page.
pub fn settings(store: Store) -> View {
    view()
        .gap_6()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_start()
                .gap_6()
                .child(
                    view()
                        .flex_1()
                        .min_w(Points(PANEL_MIN_WIDTH))
                        .child(section(
                            Icon::Settings,
                            store.tr(|s| s.settings_mode),
                            work_mode(store),
                        )),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w(Points(PANEL_MIN_WIDTH))
                        .child(section(
                            Icon::Globe,
                            store.tr(|s| s.settings_language),
                            languages(store),
                        )),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w(Points(PANEL_MIN_WIDTH))
                        .child(section(
                            Icon::Monitor,
                            store.tr(|s| s.desktop.section),
                            desktop::desktop(store),
                        )),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w(Points(PANEL_MIN_WIDTH))
                        .child(section(
                            Icon::Info,
                            store.tr(|s| s.settings_about),
                            about(store),
                        )),
                ),
        )
        .child(programs::programs(store))
}

/// A panel's heading: its icon, then its name in Title weight.
pub(super) fn heading(glyph: Icon, label: impl Fn() -> &'static str + 'static) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(icon(glyph, ICON_SIZE))
        .child(
            text(label)
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .role(AccessibilityRole::Header)
                .text_color(move || theme().foreground),
        )
}

/// A section: its heading above a body.
pub(super) fn section(
    glyph: Icon,
    label: impl Fn() -> &'static str + 'static,
    body: impl IntoElement,
) -> View {
    view().gap_3().child(heading(glyph, label)).child(body)
}

/// The work mode: the two presets or the user's own roles, what the choice shows, and that hiding stops nothing.
fn work_mode(store: Store) -> View {
    let custom = move || matches!(store.mode(), Some(WorkMode::Custom(_)));
    let rows: Vec<_> = WorkMode::PRESETS
        .into_iter()
        .enumerate()
        .map(|(index, mode)| {
            let label: Pick = match mode {
                WorkMode::Control => |s| s.mode_control,
                WorkMode::Client | WorkMode::Custom(_) => |s| s.mode_client,
            };
            choice_row(
                store.tr(label),
                move || store.mode() == Some(mode),
                index > 0,
                move || store.set_mode(mode),
            )
        })
        .chain(std::iter::once(choice_row(
            store.tr(|s| s.mode_custom),
            custom,
            true,
            move || {
                if !custom() {
                    store.set_mode(WorkMode::Custom(store.roles()));
                }
            },
        )))
        .collect();
    let summary = move || {
        let strings = store.strings();
        match store.mode() {
            Some(WorkMode::Control) => strings.mode_control_body,
            Some(WorkMode::Custom(_)) => strings.mode_custom_body,
            Some(WorkMode::Client) | None => strings.mode_client_body,
        }
    };
    view()
        .gap_3()
        .child(
            group()
                .role(AccessibilityRole::Radiogroup)
                .label(store.tr(|s| s.settings_mode))
                .child(rows),
        )
        .child(meta_text(summary))
        .child(when(custom, move || custom_roles(store)))
        .child(meta_text(store.tr(|s| s.mode_hidden_note)))
}

/// The custom mode's roles, a switch each; the last one on cannot be turned off.
fn custom_roles(store: Store) -> View {
    let rows: Vec<_> = Role::ALL
        .into_iter()
        .map(|role| {
            let (label, body, key): (Pick, Pick, &str) = match role {
                Role::Node => (|s| s.role_node, |s| s.role_node_body, "node"),
                Role::Console => (|s| s.role_console, |s| s.role_console_body, "console"),
                Role::Host => (|s| s.role_host, |s| s.role_host_body, "host"),
            };
            let on = move || store.roles().contains(role);
            switch_row(store.tr(label), on, move |next| {
                if let Some(roles) = store.roles().with(role, next) {
                    store.set_mode(WorkMode::Custom(roles));
                }
            })
            .description(store.tr(body))
            .disabled(move || on() && store.roles().with(role, false).is_none())
            .test_id(format!("mode-role-{key}"))
        })
        .collect();
    card().child(rows)
}
/// The language choices, one checked.
fn languages(store: Store) -> View {
    let rows: Vec<_> = LanguageChoice::ALL
        .into_iter()
        .enumerate()
        .map(|(index, choice)| {
            let label = move || {
                let strings = store.strings();
                match choice {
                    LanguageChoice::System => strings.language_system,
                    LanguageChoice::Fixed(Language::ZhCn) => strings.language_zh_cn,
                    LanguageChoice::Fixed(Language::En) => strings.language_en,
                }
            };
            choice_row(
                label,
                move || store.language() == choice,
                index > 0,
                move || store.set_language(choice),
            )
        })
        .collect();
    group()
        .role(AccessibilityRole::Radiogroup)
        .label(store.tr(|s| s.settings_language))
        .child(rows)
}

/// The app's version.
fn about(store: Store) -> View {
    group().child(
        view()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_2()
            .min_h_9()
            .child(
                text(store.tr(|s| s.settings_version))
                    .text_sm()
                    .flex_1()
                    .text_color(move || theme().card_foreground),
            )
            .child(
                machine_text(env!("CARGO_PKG_VERSION"))
                    .text_color(move || theme().muted_foreground),
            ),
    )
}
