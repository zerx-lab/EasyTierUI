//! The drawer (DESIGN.md "The Drawer Rule"): the place for an entity's detail and editing. On the desktops and the
//! web a right-side sheet, `min(760 pt, 72 %)` wide, with the panel fill, a hairline leading edge, the Dialog lift and
//! a `foreground` backdrop at 24 %; on phones and narrow windows a full-height page sheet. Anatomy: a header (name in
//! Title, a neutral kind chip, a status lamp with its word, the header's actions, a close button; hairline below), an
//! optional key-value grid, optional tabs, a scrolling body with 24 pt padding on every side (so a 2 pt focus ring is
//! never clipped) and a footer with the actions (hairline above).

use std::rc::Rc;

use nsh_base::{SheetEvent, dialog, sheet, theme};
use nsh_core::{ModalPresentation, prelude::*};

use super::{
    chip::kind_chip,
    icon::Icon,
    icon_button::icon_button,
    lamp::{Lamp, status_lamp},
};

/// The widest the drawer gets on the desktops, in points.
const MAX_WIDTH: f32 = 760.0;

/// How much of the window the drawer takes at most (72 %).
const WIDTH_SHARE: f32 = 0.72;

/// Below this window width the drawer takes the whole window (the layout is compact here too).
const FULL_WIDTH_BELOW: f32 = 640.0;

/// The backdrop's strength: `foreground` at 24 % (lighter than a dialog's scrim: the page stays readable).
const BACKDROP_OPACITY: f32 = 0.24;

/// The drawer's width in points at the current window width (tracked).
fn drawer_width() -> Points {
    let window = window().width.0;
    if window < FULL_WIDTH_BELOW {
        Points(window)
    } else {
        Points(MAX_WIDTH.min(window * WIDTH_SHARE))
    }
}

/// Makes a part of the drawer when it opens.
type Part = Rc<dyn Fn() -> AnyElement>;

/// Wraps `make` as a [`Part`].
fn part<E: IntoElement>(make: impl Fn() -> E + 'static) -> Part {
    Rc::new(move || make().into_element())
}

/// A drawer under construction: give it parts, then place it as a child anywhere in the page. It is on screen while
/// `open()` holds; its parts are made again on each opening.
pub struct Drawer {
    open: Rc<dyn Fn() -> bool>,
    title: Reactive<SharedString>,
    close_label: Reactive<SharedString>,
    on_close: Rc<dyn Fn()>,
    kind: Option<Reactive<SharedString>>,
    status: Option<Part>,
    actions: Option<Part>,
    meta: Option<Part>,
    tabs: Option<Part>,
    body: Option<Part>,
    footer: Option<Part>,
    test_id: Option<SharedString>,
}

/// A drawer titled `title`, shown while `open()` holds. `close_label` names the close button (and its tooltip);
/// `on_close` runs when the person closes it (the close button, Escape, a press on the backdrop, back on a phone),
/// and the caller then makes `open()` false.
///
/// ```ignore
/// drawer(move || store.selected().is_some(), move || name(), store.tr(|s| s.action_close), move || store.close())
///     .kind(store.tr(|s| s.kind_network))
///     .status(move || lamp(), store.tr(|s| s.state_running))
///     .meta(move || kv_grid(facts()))
///     .tabs(move || segmented(..))
///     .body(move || tab_content())
///     .footer(move || dialog_footer().child(save_button()))
///     .test_id("network-drawer")
/// ```
pub fn drawer(
    open: impl Fn() -> bool + 'static,
    title: impl IntoReactive<SharedString>,
    close_label: impl IntoReactive<SharedString>,
    on_close: impl Fn() + 'static,
) -> Drawer {
    Drawer {
        open: Rc::new(open),
        title: title.into_reactive(),
        close_label: close_label.into_reactive(),
        on_close: Rc::new(on_close),
        kind: None,
        status: None,
        actions: None,
        meta: None,
        tabs: None,
        body: None,
        footer: None,
        test_id: None,
    }
}

impl Drawer {
    /// A neutral kind chip after the name ("Network", "Machine").
    #[must_use]
    pub fn kind(mut self, kind: impl IntoReactive<SharedString>) -> Self {
        self.kind = Some(kind.into_reactive());
        self
    }

    /// The entity's status: a lamp with its word, after the kind chip.
    #[must_use]
    pub fn status(
        mut self,
        lamp: impl Fn() -> Lamp + Copy + 'static,
        word: impl IntoReactive<SharedString>,
    ) -> Self {
        let word = word.into_reactive();
        self.status = Some(part(move || status_lamp(lamp, word.clone())));
        self
    }

    /// Actions at the header's trailing edge, before the close button (Start / Stop).
    #[must_use]
    pub fn header_actions<E: IntoElement>(mut self, actions: impl Fn() -> E + 'static) -> Self {
        self.actions = Some(part(actions));
        self
    }

    /// The facts under the header, usually a [`kv_grid`](super::kv_grid).
    #[must_use]
    pub fn meta<E: IntoElement>(mut self, meta: impl Fn() -> E + 'static) -> Self {
        self.meta = Some(part(meta));
        self
    }

    /// The tabs under the facts, usually a [`segmented`](super::segmented) control.
    #[must_use]
    pub fn tabs<E: IntoElement>(mut self, tabs: impl Fn() -> E + 'static) -> Self {
        self.tabs = Some(part(tabs));
        self
    }

    /// The scrolling content, padded 24 pt on every side.
    #[must_use]
    pub fn body<E: IntoElement>(mut self, body: impl Fn() -> E + 'static) -> Self {
        self.body = Some(part(body));
        self
    }

    /// The sticky action row at the bottom (see [`dialog_footer`]); the primary button last.
    #[must_use]
    pub fn footer<E: IntoElement>(mut self, footer: impl Fn() -> E + 'static) -> Self {
        self.footer = Some(part(footer));
        self
    }

    /// The hook tests find the panel by (`id:<test_id>`); the close button is `<test_id>-close`.
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished region (empty while closed).
    pub fn build(self) -> AnyElement {
        let Self {
            open,
            title,
            close_label,
            on_close,
            kind,
            status,
            actions,
            meta,
            tabs,
            body,
            footer,
            test_id,
        } = self;
        let close_in_panel = on_close.clone();
        let make_panel = move || {
            let close = close_in_panel.clone();
            let close_id = test_id.as_ref().map_or_else(
                || SharedString::from("drawer-close"),
                |id| format!("{id}-close").into(),
            );
            let header = view()
                .flex_row()
                .items_center()
                .gap_3()
                .px_6()
                .py_3()
                .min_h_14()
                .border_b_1()
                .border_color(move || theme().border)
                .child(
                    text(title.clone())
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .number_of_lines(1)
                        .min_w_0()
                        .flex_shrink(1.0)
                        .role(AccessibilityRole::Header)
                        .text_color(move || theme().foreground),
                )
                .child(kind.clone().map(kind_chip))
                .child(status.as_ref().map(|status| status()))
                .child(view().flex_1())
                .child(actions.as_ref().map(|actions| actions()))
                .child(
                    icon_button(Icon::Close, close_label.clone(), move || close())
                        .test_id(close_id),
                );
            let panel = view()
                .size_full()
                .bg(move || theme().card)
                .child(header)
                .child(meta.as_ref().map(|meta| view().px_6().pt_4().child(meta())))
                .child(tabs.as_ref().map(|tabs| view().px_6().pt_4().child(tabs())))
                .child(
                    scroll()
                        .flex_1()
                        .child(view().p_6().gap_6().child(body.as_ref().map(|body| body()))),
                )
                .child(footer.as_ref().map(|footer| footer_bar().child(footer())));
            match test_id.clone() {
                Some(id) => panel.test_id(&id),
                None => panel,
            }
        };
        if is_desktop() {
            let close_from_sheet = on_close;
            sheet()
                .open(move || Some(open()))
                .side(Placement::Right)
                .on_open_change(move |event: &SheetEvent| {
                    if let SheetEvent::OpenChange(false, _) = event {
                        close_from_sheet();
                    }
                })
                .backdrop_style(|s| s.bg(move || theme().foreground.opacity(BACKDROP_OPACITY)))
                .popup_style(|s| {
                    s.w(drawer_width)
                        .max_w(relative(1.0))
                        .bg(move || theme().card)
                        .border_l_1()
                        .border_color(move || theme().border)
                        .shadow_lg()
                })
                // The parts are made when the sheet comes in, once: reading a signal while making them must not
                // rebuild the panel under the person's hands.
                .child(dynamic(move || untracked(&make_panel)))
                .into_element()
        } else {
            // A phone: a full-height page sheet, closed by the back gesture.
            let close_from_modal = on_close;
            when(
                move || open(),
                move || {
                    let close = close_from_modal.clone();
                    dialog()
                        .close_on_escape(true)
                        .on_cancel(move || close())
                        .presentation(ModalPresentation::PageSheet)
                        .child(make_panel().bg(move || theme().background))
                },
            )
            .into_element()
        }
    }
}

/// The sticky action row: a hairline above, 24 pt gutters, clear of the home indicator on phones.
fn footer_bar() -> View {
    let bar = view()
        .px_6()
        .py_3()
        .border_t_1()
        .border_color(move || theme().border);
    if is_desktop() {
        bar
    } else {
        bar.pb_safe_area_plus(Points(12.0))
    }
}

impl IntoElement for Drawer {
    fn into_element(self) -> AnyElement {
        self.build()
    }
}
