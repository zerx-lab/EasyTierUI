//! Fields (DESIGN.md "Inputs / Fields"): a visible label above a text input, helper or error text below.

use std::rc::Rc;

use nsh_base::{Input, input, theme};
use nsh_core::prelude::*;

use super::text::{control_radius, meta_text, section_label};

/// The input's width of one text line in a multiline field, in points (Body line height 20 plus padding).
const LINE_HEIGHT: f32 = 20.0;

/// Vertical padding of an input, in points (spacing step 1.5 on both sides).
const INPUT_PADDING: f32 = 12.0;

/// A text input in the field style: panel fill, 1 pt `input` edge (`destructive` while `invalid()`), control radius,
/// 32 pt high (44 pt on touch screens); on focus the edge becomes `ring` and a 2 pt `ring` outline sits 2 pt outside
/// it. `mono` sets the machine voice (addresses, CIDRs, ports).
pub(super) fn styled_input(invalid: impl Fn() -> bool + 'static, mono: bool) -> Input {
    let ring = move || theme().ring;
    let base = input()
        .px_2p5()
        .py_1p5()
        .rounded(control_radius)
        .border_1()
        .border_color(move || {
            if invalid() {
                theme().destructive
            } else {
                theme().input
            }
        })
        .bg(move || theme().card)
        .text_color(move || theme().foreground)
        .placeholder_color(move || color(theme().muted_foreground))
        .selection_color(move || color(theme().primary))
        .focus(move |s| {
            s.border_color(ring)
                .outline(Points(2.0))
                .outline_color(ring)
                .outline_offset(Points(2.0))
        });
    let sized = if is_desktop() {
        base.min_h_8().text_sm()
    } else {
        base.min_h_11().text_base()
    };
    if mono {
        sized.font_family("monospace").tabular_nums()
    } else {
        sized
    }
}

/// A labelled field under construction: set what it needs, then place it as a child.
pub struct Field {
    label: Reactive<SharedString>,
    value: Rc<dyn Fn() -> String>,
    on_change: Rc<dyn Fn(&str)>,
    placeholder: Option<Reactive<String>>,
    helper: Option<Reactive<SharedString>>,
    error: Option<Rc<dyn Fn() -> Option<SharedString>>>,
    mono: bool,
    lines: Option<u8>,
    disabled: bool,
    test_id: Option<SharedString>,
}

/// A field named `label` showing `value()`; `on_change` receives every edit (the caller stores it and the field
/// shows what `value()` returns, so a rejected edit does not appear).
///
/// ```ignore
/// field(store.tr(|s| s.field_network_name), move || store.name(), move |t| store.set_name(t))
///     .helper(store.tr(|s| s.field_network_name_help))
///     .error(move || store.name_error())
///     .test_id("network-name")
/// ```
pub fn field(
    label: impl IntoReactive<SharedString>,
    value: impl Fn() -> String + 'static,
    on_change: impl Fn(&str) + 'static,
) -> Field {
    Field {
        label: label.into_reactive(),
        value: Rc::new(value),
        on_change: Rc::new(on_change),
        placeholder: None,
        helper: None,
        error: None,
        mono: false,
        lines: None,
        disabled: false,
        test_id: None,
    }
}

impl Field {
    /// Text shown while the field is empty. For an address field this is the effective default (`11010`), so the
    /// default is visible without being a value.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl IntoReactive<String>) -> Self {
        self.placeholder = Some(placeholder.into_reactive());
        self
    }

    /// Meta text under the input: what the field is for.
    #[must_use]
    pub fn helper(mut self, helper: impl IntoReactive<SharedString>) -> Self {
        self.helper = Some(helper.into_reactive());
        self
    }

    /// The error to show, or `None` while the value is fine. The edge turns `destructive` and the message replaces
    /// the helper; the field keeps its value. The message should name the field and the fix.
    #[must_use]
    pub fn error(mut self, error: impl Fn() -> Option<SharedString> + 'static) -> Self {
        self.error = Some(Rc::new(error));
        self
    }

    /// Machine voice: monospace with tabular figures, for addresses, CIDRs and ports.
    #[must_use]
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// A multiline field at least `lines` lines tall (pasting a TOML config); return adds a line.
    #[must_use]
    pub fn multiline(mut self, lines: u8) -> Self {
        self.lines = Some(lines.max(1));
        self
    }

    /// Not editable while `disabled`.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The hook tests find the input by (`id:<test_id>`); the error text is `<test_id>-error`.
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
            placeholder,
            helper,
            error,
            mono,
            lines,
            disabled,
            test_id,
        } = self;
        let error_for_edge = error.clone();
        let mut input = styled_input(
            move || error_for_edge.as_ref().is_some_and(|e| e().is_some()),
            mono,
        )
        .label(label.clone())
        .value(move || value())
        .readonly(disabled)
        .on_change(move |text: &str| on_change(text));
        if let Some(placeholder) = placeholder {
            input = input.placeholder(placeholder);
        }
        if let Some(lines) = lines {
            input = input
                .multiline(true)
                .min_h(Points(f32::from(lines) * LINE_HEIGHT + INPUT_PADDING));
        }
        if let Some(id) = test_id.clone() {
            input = input.test_id(&id);
        }
        let message_id = test_id.map(|id| format!("{id}-error"));
        // Under the input: the error when there is one, else the helper.
        let below = error.clone();
        let message = move || {
            below
                .as_ref()
                .and_then(|e| e())
                .map(|text| (text, true))
                .or_else(|| helper.as_ref().map(|h| (super::data::read(h), false)))
        };
        view()
            .gap_1()
            .child(section_label(label))
            .child(input)
            .child(dynamic(move || {
                message().map(|(text, is_error)| {
                    let line = meta_text(text);
                    let line = if is_error {
                        line.role(AccessibilityRole::Alert)
                            .text_color(move || theme().destructive)
                    } else {
                        line
                    };
                    match message_id.clone() {
                        Some(id) if is_error => line.test_id(&id),
                        _ => line,
                    }
                })
            }))
    }
}

impl IntoElement for Field {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}
