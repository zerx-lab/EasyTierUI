//! The more menu (DESIGN.md "Buttons"): a ghost "more" icon button that opens a menu of labelled actions, each with an
//! icon. A destructive action is drawn in `destructive` and asks its own confirmation.

use std::rc::Rc;

use nsh_base::{menu, menu_item, menu_separator, theme};
use nsh_core::prelude::*;

use super::{
    data::read,
    icon::{ICON_SIZE, Icon, icon_colored},
    text::{control_radius, tagged},
};

/// One action of a [`more_menu`].
pub struct MenuAction {
    icon: Icon,
    label: Reactive<SharedString>,
    on_select: Rc<dyn Fn()>,
    destructive: bool,
    separated: bool,
    test_id: Option<SharedString>,
}

/// An action with `icon` and `label`; choosing it runs `on_select` and closes the menu.
///
/// ```ignore
/// menu_action(Icon::Download, store.tr(|s| s.action_export), move || store.export(id))
/// ```
pub fn menu_action(
    icon: Icon,
    label: impl IntoReactive<SharedString>,
    on_select: impl Fn() + 'static,
) -> MenuAction {
    MenuAction {
        icon,
        label: label.into_reactive(),
        on_select: Rc::new(on_select),
        destructive: false,
        separated: false,
        test_id: None,
    }
}

impl MenuAction {
    /// Draws the action in `destructive` (delete): the action must still ask for confirmation naming the object.
    #[must_use]
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Puts a hairline above the action: a group of its own.
    #[must_use]
    pub fn separated(mut self) -> Self {
        self.separated = true;
        self
    }

    /// The hook tests find the action by (`id:<test_id>`).
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }
}

/// A more menu under construction.
pub struct MoreMenu {
    label: Reactive<SharedString>,
    actions: Vec<MenuAction>,
    test_id: Option<SharedString>,
}

/// A "more" button named `label` (its accessible name) that opens `actions`.
///
/// ```ignore
/// more_menu(store.tr(|s| s.action_more), vec![
///     menu_action(Icon::Settings, store.tr(|s| s.action_settings), move || store.open_settings(id)),
///     menu_action(Icon::Trash, store.tr(|s| s.action_delete), move || store.ask_delete(id)).destructive().separated(),
/// ])
/// .test_id("network-more")
/// ```
pub fn more_menu(label: impl IntoReactive<SharedString>, actions: Vec<MenuAction>) -> MoreMenu {
    MoreMenu {
        label: label.into_reactive(),
        actions,
        test_id: None,
    }
}

impl MoreMenu {
    /// The hook tests find the trigger by (`id:<test_id>`).
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view.
    pub fn build(self) -> View {
        let Self {
            label,
            actions,
            test_id,
        } = self;
        let trigger = view()
            .role(AccessibilityRole::Button)
            .label(label)
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(control_radius)
            .hover(|s| s.bg(move || theme().accent))
            .active(|s| s.bg(move || theme().accent))
            .child(icon_colored(Icon::More, ICON_SIZE, move || {
                theme().muted_foreground
            }));
        let trigger = if is_desktop() {
            trigger.size_8()
        } else {
            trigger.size_11()
        };
        let trigger = match test_id.clone() {
            Some(id) => trigger.test_id(&id),
            None => trigger,
        };
        let items: Vec<AnyElement> = actions.into_iter().map(action_item).collect();
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
                .child(items),
        )
    }
}

impl IntoElement for MoreMenu {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}

/// An action as a menu row: icon and label, with a hairline above when it starts a group.
fn action_item(action: MenuAction) -> AnyElement {
    let MenuAction {
        icon,
        label,
        on_select,
        destructive,
        separated,
        test_id,
    } = action;
    let tone = move || {
        if destructive {
            theme().destructive
        } else {
            theme().popover_foreground
        }
    };
    let row = menu_item()
        .on_select(move || on_select())
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .py_1p5()
        .min_h_8()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().accent))
        .child(icon_colored(icon, ICON_SIZE, move || {
            if destructive {
                theme().destructive
            } else {
                theme().muted_foreground
            }
        }))
        .child(
            text(move || read(&label))
                .text_sm()
                .number_of_lines(1)
                .flex_1()
                .text_color(tone),
        );
    let item = tagged(row, test_id.as_deref());
    if separated {
        view()
            .child(
                menu_separator()
                    .my_1()
                    .border_t_1()
                    .border_color(move || theme().border),
            )
            .child(item)
            .into_element()
    } else {
        item
    }
}
