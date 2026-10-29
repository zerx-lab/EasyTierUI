//! Callouts (DESIGN.md "Callouts"): a 10 % tint of the hue, a drawn hue icon (info, warning triangle, error circle)
//! and `foreground` text, for exposure warnings, core problems and tips.

use std::rc::Rc;

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    button::{ButtonKind, action_button},
    icon::{ICON_SIZE, Icon, icon_colored},
    text::app_color,
};

/// What a callout says about its subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalloutKind {
    /// A tip or a fact (`primary-muted`).
    Info,
    /// Attention: a setting that widens exposure, a core update (`warning-muted`).
    Warning,
    /// Something failed (`destructive-muted`).
    Danger,
}

impl CalloutKind {
    /// The hue of the glyph.
    fn hue(self) -> AdaptiveColor {
        match self {
            Self::Info => theme().primary,
            Self::Warning => app_color("warning"),
            Self::Danger => theme().destructive,
        }
    }

    /// The tint behind the text.
    fn tint(self) -> AdaptiveColor {
        match self {
            Self::Info => app_color("primary-muted"),
            Self::Warning => app_color("warning-muted"),
            Self::Danger => app_color("destructive-muted"),
        }
    }

    /// The icon that names the kind: it is drawn, so it needs no icon font and reads in both appearances.
    fn icon(self) -> Icon {
        match self {
            Self::Info => Icon::Info,
            Self::Warning => Icon::AlertTriangle,
            Self::Danger => Icon::AlertCircle,
        }
    }
}

/// An action button's label and handler.
type Action = (Reactive<SharedString>, Rc<dyn Fn()>);

/// A callout under construction: add an [`action`](Callout::action) and a [`test_id`](Callout::test_id), then place it
/// as a child.
pub struct Callout {
    kind: CalloutKind,
    message: Reactive<SharedString>,
    action: Option<Action>,
    test_id: Option<SharedString>,
}

/// A callout of `kind` saying `message`.
///
/// ```ignore
/// callout(CalloutKind::Warning, store.tr(|s| s.exposure_exit_node))
///     .action(store.tr(|s| s.action_learn_more), move || store.open_docs())
///     .test_id("exit-node-warning")
/// ```
pub fn callout(kind: CalloutKind, message: impl IntoReactive<SharedString>) -> Callout {
    Callout {
        kind,
        message: message.into_reactive(),
        action: None,
        test_id: None,
    }
}

impl Callout {
    /// A secondary button after the message, running `on_press`.
    #[must_use]
    pub fn action(
        mut self,
        label: impl IntoReactive<SharedString>,
        on_press: impl Fn() + 'static,
    ) -> Self {
        self.action = Some((label.into_reactive(), Rc::new(on_press)));
        self
    }

    /// The hook tests find the callout by (`id:<test_id>`).
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view.
    pub fn build(self) -> View {
        let Self {
            kind,
            message,
            action,
            test_id,
        } = self;
        // The icon is decoration: the message beside it says everything.
        let glyph = icon_colored(kind.icon(), ICON_SIZE, move || kind.hue());
        let body = text(message)
            .flex_1()
            .text_color(move || theme().foreground);
        let body = if is_desktop() {
            body.text_sm()
        } else {
            body.text_base()
        };
        let row = view()
            .role(match kind {
                CalloutKind::Info => AccessibilityRole::Summary,
                CalloutKind::Warning | CalloutKind::Danger => AccessibilityRole::Alert,
            })
            .flex_row()
            .items_start()
            .gap_3()
            .px_4()
            .py_3()
            .rounded(move || theme().radius)
            .bg(move || kind.tint())
            .child(view().pt_0p5().child(glyph))
            .child(body)
            .when_some(action, |row, (label, on_press)| {
                row.child(action_button(ButtonKind::Secondary, label, move || {
                    on_press();
                }))
            });
        match test_id {
            Some(id) => row.test_id(&id),
            None => row,
        }
    }
}

impl IntoElement for Callout {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}
