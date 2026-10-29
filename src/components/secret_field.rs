//! The Secret Field (DESIGN.md signature): masked by default, with Show / Hide, Copy and Generate actions and a
//! strength hint when the secret is typed by hand.

use std::rc::Rc;

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    button::{ButtonKind, action_button},
    data::{CopyLabels, copy_button, read},
    field::styled_input,
    text::{app_color, meta_text, section_label, tagged},
};

/// How strong the typed secret is; the hint text beside it says the same in words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintLevel {
    /// Too short or guessable (Relay Amber: attention, not failure).
    Weak,
    /// Acceptable (`muted-foreground`).
    Fair,
    /// Strong (Lamp Green).
    Strong,
}

/// The words on the secret field's buttons, which components cannot know.
#[derive(Clone)]
pub struct SecretLabels {
    show: Reactive<SharedString>,
    hide: Reactive<SharedString>,
    copy: CopyLabels,
}

impl SecretLabels {
    /// The "Show" and "Hide" labels and the copy button's words.
    ///
    /// ```ignore
    /// SecretLabels::new(show, hide, CopyLabels::new(copy, copied))
    /// ```
    pub fn new(
        show: impl IntoReactive<SharedString>,
        hide: impl IntoReactive<SharedString>,
        copy: CopyLabels,
    ) -> Self {
        Self {
            show: show.into_reactive(),
            hide: hide.into_reactive(),
            copy,
        }
    }
}

/// The Generate button's label and handler.
type GenerateAction = (Reactive<SharedString>, Rc<dyn Fn()>);
/// The strength hint's text and level.
type Hint = (Reactive<SharedString>, Rc<dyn Fn() -> HintLevel>);

/// A secret field under construction.
pub struct SecretField {
    label: Reactive<SharedString>,
    value: Rc<dyn Fn() -> String>,
    on_change: Rc<dyn Fn(&str)>,
    labels: SecretLabels,
    generate: Option<GenerateAction>,
    hint: Option<Hint>,
    error: Option<Rc<dyn Fn() -> Option<SharedString>>>,
    test_id: Option<SharedString>,
}

/// A secret field named `label` showing `value()` masked; `on_change` receives every edit. Copy puts the secret on
/// the clipboard only when pressed.
///
/// ```ignore
/// secret_field(store.tr(|s| s.field_secret), move || store.secret_text(), move |t| store.set_secret(t), labels)
///     .generate(store.tr(|s| s.action_generate), move || store.generate_secret())
///     .hint(move || store.secret_hint_text(), move || store.secret_hint_level())
///     .test_id("network-secret")
/// ```
pub fn secret_field(
    label: impl IntoReactive<SharedString>,
    value: impl Fn() -> String + 'static,
    on_change: impl Fn(&str) + 'static,
    labels: SecretLabels,
) -> SecretField {
    SecretField {
        label: label.into_reactive(),
        value: Rc::new(value),
        on_change: Rc::new(on_change),
        labels,
        generate: None,
        hint: None,
        error: None,
        test_id: None,
    }
}

impl SecretField {
    /// Adds a Generate button running `on_press` (the caller stores a fresh secret). Without it there is none.
    #[must_use]
    pub fn generate(
        mut self,
        label: impl IntoReactive<SharedString>,
        on_press: impl Fn() + 'static,
    ) -> Self {
        self.generate = Some((label.into_reactive(), Rc::new(on_press)));
        self
    }

    /// The strength hint line: `text` (empty hides the line) and its `level`.
    #[must_use]
    pub fn hint(
        mut self,
        text: impl IntoReactive<SharedString>,
        level: impl Fn() -> HintLevel + 'static,
    ) -> Self {
        self.hint = Some((text.into_reactive(), Rc::new(level)));
        self
    }

    /// The error to show, or `None` (see [`super::Field::error`]).
    #[must_use]
    pub fn error(mut self, error: impl Fn() -> Option<SharedString> + 'static) -> Self {
        self.error = Some(Rc::new(error));
        self
    }

    /// The hook tests find the input by (`id:<test_id>`); the buttons are `<test_id>-toggle`, `<test_id>-copy`
    /// and `<test_id>-generate`, the error text `<test_id>-error`.
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view.
    pub fn build(self) -> View {
        let Self {
            label,
            value,
            on_change,
            labels,
            generate,
            hint,
            error,
            test_id,
        } = self;
        let revealed = signal(false);
        let error_for_edge = error.clone();
        let copy_source = value.clone();
        let mut field = styled_input(
            move || error_for_edge.as_ref().is_some_and(|e| e().is_some()),
            true,
        )
        .label(label.clone())
        .masked(move || !revealed.get())
        .value(move || value())
        .on_change(move |text: &str| on_change(text))
        .auto_correct(false)
        .auto_capitalize(AutoCapitalize::None)
        .flex_1();
        if let Some(id) = test_id.clone() {
            field = field.test_id(&id);
        }
        let toggle_label = move || {
            if revealed.get() {
                read(&labels.hide)
            } else {
                read(&labels.show)
            }
        };
        let id_of = |suffix: &str| test_id.as_ref().map(|id| format!("{id}-{suffix}"));
        let toggle = tagged(
            action_button(ButtonKind::Secondary, toggle_label, move || {
                revealed.update(|shown| *shown = !*shown);
            }),
            id_of("toggle").as_deref(),
        );
        let copy = copy_button(copy_source, labels.copy.clone(), id_of("copy").as_deref());
        let generate = generate.map(|(label, on_press)| {
            tagged(
                action_button(ButtonKind::Secondary, label, move || on_press()),
                id_of("generate").as_deref(),
            )
        });
        let actions = view()
            .flex_row()
            .flex_wrap()
            .gap_2()
            .child(toggle)
            .child(copy)
            .when_some(generate, |row, button| row.child(button));
        // Wide screens keep the input and its actions on one line; phones give the input the full width.
        let line = if is_desktop() {
            view().flex_row().items_start().gap_2().child(field)
        } else {
            view().gap_2().child(field)
        }
        .child(actions);
        let below = error.clone();
        let message_id = id_of("error");
        view()
            .gap_1()
            .child(section_label(label))
            .child(line)
            .child(dynamic(move || {
                let message = below.as_ref().and_then(|e| e());
                if let Some(message) = message {
                    let line = meta_text(message)
                        .role(AccessibilityRole::Alert)
                        .text_color(move || theme().destructive);
                    return Some(match message_id.as_deref() {
                        Some(id) => line.test_id(id),
                        None => line,
                    });
                }
                let (text, level) = hint.as_ref()?;
                let text = read(text);
                if text.is_empty() {
                    return None;
                }
                let level = level();
                Some(meta_text(text).text_color(move || match level {
                    HintLevel::Weak => app_color("warning"),
                    HintLevel::Fair => theme().muted_foreground,
                    HintLevel::Strong => app_color("success"),
                }))
            }))
    }
}

impl IntoElement for SecretField {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}
