//! Settings: the UI's language, the EasyTier programs, and the app's version, as grouped panels with an icon each.
//! Language and About sit side by side on a wide window and stack on a narrow one; the EasyTier panel spans the page.

mod adopt;
mod installed;
mod programs;
mod releases;

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    components::{
        ButtonKind, ICON_SIZE, Icon, action_button, choice_row, group, icon, machine_text,
    },
    i18n::{Language, LanguageChoice},
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
