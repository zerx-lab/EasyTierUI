//! Settings: the UI's language and the app's version.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    components::{choice_row, machine_text, section_label},
    i18n::{Language, LanguageChoice},
    store::Store,
};

/// The settings page.
pub fn settings(store: Store) -> View {
    view()
        .gap_6()
        .child(section(store.tr(|s| s.settings_language), languages(store)))
        .child(section(store.tr(|s| s.settings_about), about(store)))
}

/// A section: its label above a grouped list.
fn section(label: impl Fn() -> &'static str + 'static, body: View) -> View {
    view().gap_2().child(section_label(label)).child(body)
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

/// A grouped list: panel fill, hairline, container radius (DESIGN.md "Flat Resting State Rule").
fn group() -> View {
    view()
        .overflow_hidden()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
}
