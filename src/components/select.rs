//! Choosing one of a few options: a [`select`] (a trigger row that opens a menu; nsh-base 0.3 has no Select) and a
//! [`segmented`] control (Tabs). Both are 32 pt high on the desktops and 44 pt on touch screens, with control radius.

use std::rc::Rc;

use nsh_base::{menu, menu_item, tab, tabs, theme};
use nsh_core::prelude::*;

use super::{
    icon::{ICON_SIZE, Icon, icon, icon_colored},
    text::{body_text, control_radius, section_label, tagged},
};

/// One option of a [`select`] or [`segmented`] control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    /// What the caller recognizes the option by, passed back to the handler.
    pub id: SharedString,
    /// What the person reads.
    pub label: SharedString,
}

impl SelectOption {
    /// An option `id` shown as `label`.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// The width of an [`inline`](Select::inline) select's trigger in points.
const INLINE_WIDTH: f32 = 160.0;

/// A select under construction.
pub struct Select {
    label: Reactive<SharedString>,
    options: Rc<Vec<SelectOption>>,
    selected: Rc<dyn Fn() -> SharedString>,
    on_select: Rc<dyn Fn(&str)>,
    disabled: bool,
    inline: bool,
    test_id: Option<SharedString>,
}

/// A select named `label` offering `options`; `selected()` is the id of the chosen one and `on_select` receives the
/// id of the one picked. The open menu marks the chosen option with a check.
///
/// ```ignore
/// select(store.tr(|s| s.field_protocol), options, move || store.protocol_id(), move |id| store.set_protocol(id))
///     .test_id("protocol")
/// ```
pub fn select(
    label: impl IntoReactive<SharedString>,
    options: Vec<SelectOption>,
    selected: impl Fn() -> SharedString + 'static,
    on_select: impl Fn(&str) + 'static,
) -> Select {
    Select {
        label: label.into_reactive(),
        options: Rc::new(options),
        selected: Rc::new(selected),
        on_select: Rc::new(on_select),
        disabled: false,
        inline: false,
        test_id: None,
    }
}

impl Select {
    /// Does not open while `disabled`.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// For a toolbar: hides the visible label (it stays the trigger's accessible name) and fixes the trigger at
    /// about 160 pt.
    #[must_use]
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }

    /// The hook tests find the trigger by (`id:<test_id>`); each option is `<test_id>-<option id>`.
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view.
    pub fn build(self) -> View {
        let Self {
            label,
            options,
            selected,
            on_select,
            disabled,
            inline,
            test_id,
        } = self;
        let current = {
            let options = options.clone();
            let selected = selected.clone();
            move || {
                let id = selected();
                options
                    .iter()
                    .find(|option| option.id == id)
                    .map(|option| option.label.clone())
                    .unwrap_or_default()
            }
        };
        // The chevron is decoration: the trigger's accessible name is the label, its value the chosen text.
        let trigger = view()
            .role(AccessibilityRole::Combobox)
            .label(label.clone())
            .flex_row()
            .items_center()
            .gap_2()
            .px_2p5()
            .py_1p5()
            .rounded(control_radius)
            .border_1()
            .border_color(move || theme().input)
            .bg(move || theme().card)
            .hover(|s| s.bg(move || theme().accent))
            .when(disabled, |t| t.opacity(0.5))
            .child(body_text(current).flex_1())
            .child(icon(Icon::ChevronDown, ICON_SIZE));
        let trigger = if is_desktop() {
            trigger.min_h_8()
        } else {
            trigger.min_h_11()
        };
        let trigger = match test_id.clone() {
            Some(id) => trigger.test_id(&id),
            None => trigger,
        };
        let items = options.iter().map(|option| {
            let id = option.id.clone();
            let chosen = id.clone();
            let is_selected = selected.clone();
            let on_select = on_select.clone();
            let row = menu_item()
                .on_select(move || on_select(&chosen))
                .flex_row()
                .items_center()
                .gap_2()
                .px_3()
                .py_1p5()
                .min_h_8()
                .hover(|s| s.bg(move || theme().accent))
                .active(|s| s.bg(move || theme().accent))
                // The check is the selected option's mark; the others keep its room so labels line up.
                .child(icon_colored(Icon::Check, ICON_SIZE, move || {
                    if is_selected() == id {
                        theme().primary
                    } else {
                        AdaptiveColor::TRANSPARENT
                    }
                }))
                .child(body_text(option.label.clone()).flex_1());
            tagged(
                row,
                test_id
                    .as_ref()
                    .map(|test| format!("{test}-{}", option.id))
                    .as_deref(),
            )
        });
        // A disabled select is its trigger alone: nothing opens the menu.
        let control = if disabled {
            view().child(trigger)
        } else {
            view().child(
                menu()
                    .popup_style(|s| {
                        s.bg(move || theme().popover)
                            .border_1()
                            .border_color(move || theme().border)
                            .rounded(move || theme().radius)
                            .shadow_md()
                            .py_1()
                            .min_w_48()
                    })
                    .offset(Points(4.0))
                    .trigger(trigger)
                    .child(items.collect::<Vec<_>>()),
            )
        };
        if inline {
            view()
                .w(Points(INLINE_WIDTH))
                .flex_shrink_0()
                .child(control)
        } else {
            view().gap_1().child(section_label(label)).child(control)
        }
    }
}

impl IntoElement for Select {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}

/// A segmented control: a well-filled strip of tabs, the chosen one raised to the `card` layer. `selected()` is the
/// id of the chosen option and `on_select` receives the id pressed. `label` names the group for screen readers.
/// Options are tabs, so use this for views of one subject (a few, short labels), and [`select`] for longer lists.
///
/// ```ignore
/// segmented(store.tr(|s| s.field_role), options, move || store.role_id(), move |id| store.set_role(id), "role")
/// ```
pub fn segmented(
    label: impl IntoReactive<SharedString>,
    options: Vec<SelectOption>,
    selected: impl Fn() -> SharedString + Clone + 'static,
    on_select: impl Fn(&str) + 'static,
    test_id: &str,
) -> View {
    strip(label, options, selected, on_select, test_id, true)
}

/// [`segmented`] without the visible label, for toolbars: `label` stays the group's accessible name.
///
/// ```ignore
/// segmented_inline(store.tr(|s| s.filter_state), options, move || store.filter_id(), move |id| store.set_filter(id), "state")
/// ```
pub fn segmented_inline(
    label: impl IntoReactive<SharedString>,
    options: Vec<SelectOption>,
    selected: impl Fn() -> SharedString + Clone + 'static,
    on_select: impl Fn(&str) + 'static,
    test_id: &str,
) -> View {
    strip(label, options, selected, on_select, test_id, false)
}

/// The strip of tabs behind [`segmented`] and [`segmented_inline`].
fn strip(
    label: impl IntoReactive<SharedString>,
    options: Vec<SelectOption>,
    selected: impl Fn() -> SharedString + Clone + 'static,
    on_select: impl Fn(&str) + 'static,
    test_id: &str,
    show_label: bool,
) -> View {
    let on_select = Rc::new(on_select);
    let name = label.into_reactive();
    let tabs_list = options.into_iter().fold(
        tabs()
            .flex_row()
            .gap_0p5()
            .p_0p5()
            .rounded(control_radius)
            .bg(move || theme().muted),
        |list, option| {
            let id = option.id.clone();
            let pressed = id.clone();
            let is_selected = selected.clone();
            let on_select = on_select.clone();
            let own = id.clone();
            let chosen = {
                let is_selected = is_selected.clone();
                move || is_selected() == own
            };
            let chosen_fill = chosen.clone();
            let tab = tab()
                .selected(chosen)
                .on_press(move || on_select(&pressed))
                .items_center()
                .justify_center()
                .px_3()
                .py_1()
                .w_full()
                .rounded(control_radius)
                .bg(move || {
                    if chosen_fill() {
                        theme().card
                    } else {
                        AdaptiveColor::TRANSPARENT
                    }
                })
                .child(
                    body_text(option.label)
                        .font_weight(FontWeight::MEDIUM)
                        .number_of_lines(1),
                );
            let tab = if is_desktop() {
                tab.min_h_7()
            } else {
                tab.min_h_11()
            };
            // The tab takes no test id of its own, so a view that shares the strip's width carries it.
            list.child(
                view()
                    .test_id(&format!("{test_id}-{id}"))
                    // Grow from the label's own width (not a zero basis), so every label shows whole.
                    .flex_grow(1.0)
                    .flex_shrink_0()
                    .child(tab),
            )
        },
    );
    if show_label {
        view().gap_1().child(section_label(name)).child(tabs_list)
    } else {
        // No visible label: the group keeps it as its accessible name.
        view().label(name).flex_shrink_0().child(tabs_list)
    }
}
