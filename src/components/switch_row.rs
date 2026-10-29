//! Switch rows (DESIGN.md "Callouts", "Pill"): a label with its description and a switch; a setting that widens
//! exposure says what it does right where it is turned on.

use std::rc::Rc;

use nsh_base::switch;
use nsh_core::prelude::*;

use super::{
    callout::{CalloutKind, callout},
    data::read,
    text::{body_text, meta_text, tagged},
};

/// A switch row under construction.
pub struct SwitchRow {
    label: Reactive<SharedString>,
    description: Option<Reactive<SharedString>>,
    checked: Rc<dyn Fn() -> bool>,
    on_change: Rc<dyn Fn(bool)>,
    exposure: Option<Reactive<SharedString>>,
    disabled: Option<Rc<dyn Fn() -> bool>>,
    test_id: Option<SharedString>,
}

/// A row with the switch `label` that shows `checked()` and reports the next value to `on_change` (the caller stores
/// it; the switch shows what `checked()` returns).
///
/// ```ignore
/// switch_row(store.tr(|s| s.opt_exit_node), move || store.exit_node(), move |on| store.set_exit_node(on))
///     .description(store.tr(|s| s.opt_exit_node_help))
///     .exposure(store.tr(|s| s.opt_exit_node_warning))
///     .test_id("exit-node")
/// ```
pub fn switch_row(
    label: impl IntoReactive<SharedString>,
    checked: impl Fn() -> bool + 'static,
    on_change: impl Fn(bool) + 'static,
) -> SwitchRow {
    SwitchRow {
        label: label.into_reactive(),
        description: None,
        checked: Rc::new(checked),
        on_change: Rc::new(on_change),
        exposure: None,
        disabled: None,
        test_id: None,
    }
}

impl SwitchRow {
    /// Meta text under the label: what the setting does.
    #[must_use]
    pub fn description(mut self, description: impl IntoReactive<SharedString>) -> Self {
        self.description = Some(description.into_reactive());
        self
    }

    /// Marks the setting as one that widens exposure (exit node, subnet proxy, encryption off): while it is on, a
    /// warning callout with `warning` sits under the row saying what it now does.
    #[must_use]
    pub fn exposure(mut self, warning: impl IntoReactive<SharedString>) -> Self {
        self.exposure = Some(warning.into_reactive());
        self
    }

    /// Dims the row and ignores presses while `disabled()` holds; say why nearby.
    #[must_use]
    pub fn disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
        self.disabled = Some(Rc::new(disabled));
        self
    }

    /// The hook tests find the switch by (`id:<test_id>`); the warning callout is `<test_id>-warning`.
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view.
    pub fn build(self) -> View {
        let Self {
            label,
            description,
            checked,
            on_change,
            exposure,
            disabled,
            test_id,
        } = self;
        let is_checked = checked.clone();
        let is_disabled = disabled.clone();
        let name = label.clone();
        // The switch itself is the press target; on touch screens its frame is 44 pt tall and wide.
        let toggle = switch()
            .checked(move || is_checked())
            .disabled(move || is_disabled.as_ref().is_some_and(|d| d()))
            .accessibility_label(move || Some(read(&name)))
            .track_class(
                "flex-row items-center w-10 h-6 p-0.5 rounded-full bg-input checked:bg-primary",
            )
            .thumb_class(
                "w-5 h-5 rounded-full bg-background checked:bg-primary-foreground \
                 checked:translate-x-4 transition-transform duration-150",
            )
            .on_change(move |next: &bool| on_change(*next))
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .disabled_style(|s| s.opacity(0.5));
        let toggle = if is_desktop() {
            toggle
        } else {
            toggle.min_h_11().min_w_11()
        };
        let toggle = tagged(toggle, test_id.as_deref());
        let texts = view()
            .flex_1()
            .child(body_text(label))
            .when_some(description, |col, description| {
                col.child(meta_text(description))
            });
        let warning_id = test_id.map(|id| format!("{id}-warning"));
        let warning = exposure.map(|warning| {
            let shown = checked.clone();
            when(
                move || shown(),
                move || {
                    let callout = callout(CalloutKind::Warning, warning.clone());
                    match warning_id.clone() {
                        Some(id) => callout.test_id(id),
                        None => callout,
                    }
                },
            )
        });
        view()
            .gap_2()
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(texts)
                    .child(toggle),
            )
            .when_some(warning, |column, warning| column.child(warning))
    }
}

impl IntoElement for SwitchRow {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}
