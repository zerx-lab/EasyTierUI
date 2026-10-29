//! Icon buttons (DESIGN.md "Buttons"): a ghost button that holds one icon, 32 pt on the desktops and 44 pt on touch
//! screens, named for screen readers and, with a pointer, by a tooltip.

use nsh_base::{button, theme, tooltip};
use nsh_core::prelude::*;

use super::{
    data::read,
    icon::{ICON_SIZE, Icon, icon_colored},
    text::{control_radius, meta_text},
};

/// How an icon button is tinted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconTone {
    /// `muted-foreground` at rest, `foreground` under the pointer.
    #[default]
    Quiet,
    /// `destructive`, for a button that deletes (the confirmation still names the object).
    Danger,
    /// `primary`, for the state a toggle is in (a follow switch that is on).
    Active,
}

/// An icon button under construction: add a [`test_id`](IconButton::test_id), then place it as a child.
pub struct IconButton {
    icon: Icon,
    label: Reactive<SharedString>,
    on_press: Box<dyn Fn()>,
    tone: IconTone,
    disabled: bool,
    selected: bool,
    test_id: Option<SharedString>,
}

/// A ghost icon button showing `icon` and named `label` (its accessible name and its tooltip on the desktops);
/// pressing it runs `on_press`.
///
/// ```ignore
/// icon_button(Icon::Refresh, store.tr(|s| s.action_refresh), move || store.refresh())
///     .test_id("refresh")
/// ```
pub fn icon_button(
    icon: Icon,
    label: impl IntoReactive<SharedString>,
    on_press: impl Fn() + 'static,
) -> IconButton {
    IconButton {
        icon,
        label: label.into_reactive(),
        on_press: Box::new(on_press),
        tone: IconTone::Quiet,
        disabled: false,
        selected: false,
        test_id: None,
    }
}

impl IconButton {
    /// Tints the icon (default [`IconTone::Quiet`]).
    #[must_use]
    pub fn tone(mut self, tone: IconTone) -> Self {
        self.tone = tone;
        self
    }

    /// Does not respond while `disabled` (50 % opacity). Pass the value as it is when the view is built: a reactive
    /// closure bound into a button inside a branch that re-renders on the same flush is a known NSH panic.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks a toggle as on: the well fill stays and screen readers hear the selected state.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The hook tests find the button by (`id:<test_id>`).
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view: the button inside a wrapper that tracks the pointer (nsh-base's `Button` takes no hover
    /// events) and carries the tooltip and the test id.
    pub fn build(self) -> View {
        let Self {
            icon,
            label,
            on_press,
            tone,
            disabled,
            selected,
            test_id,
        } = self;
        let hot = signal(false);
        let ink = move || match tone {
            IconTone::Danger => theme().destructive,
            IconTone::Active => theme().primary,
            IconTone::Quiet => {
                if hot.get() {
                    theme().foreground
                } else {
                    theme().muted_foreground
                }
            }
        };
        let name = label.clone();
        let pressable = button()
            .accessibility_label(move || Some(read(&name)))
            .selected(selected)
            .disabled(disabled)
            .on_press(on_press)
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(control_radius)
            .bg(move || {
                if selected {
                    theme().muted
                } else {
                    AdaptiveColor::TRANSPARENT
                }
            })
            .hover(|s| s.bg(move || theme().accent))
            .active(|s| s.bg(move || theme().accent))
            .disabled_style(|s| s.opacity(0.5))
            .child(icon_colored(icon, ICON_SIZE, ink));
        let sized = if is_desktop() {
            pressable.size_8()
        } else {
            pressable.size_11()
        };
        let tip = label;
        let holder = view()
            .flex_shrink_0()
            .on_hover_in(move || hot.set(true))
            .on_hover_out(move || hot.set(false))
            .child(sized);
        let holder = match test_id {
            Some(id) => holder.test_id(&id),
            None => holder,
        };
        if is_desktop() {
            holder.tooltip(
                tooltip().child(
                    view()
                        .px_2()
                        .py_1()
                        .rounded(control_radius)
                        .border_1()
                        .border_color(move || theme().border)
                        .bg(move || theme().popover)
                        .shadow_md()
                        .child(
                            meta_text(move || read(&tip))
                                .text_color(move || theme().popover_foreground),
                        ),
                ),
            )
        } else {
            holder
        }
    }
}

impl IntoElement for IconButton {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}
