//! Dialogs and confirmations (DESIGN.md "Dialogs and Sheets"). Desktops and the web: a centered panel over a scrim,
//! 24 pt padding, 480 pt wide (640 pt for config editors), Dialog lift; the layer fades in while the panel settles
//! into place, and fades out as it was. Phones: a full-height page sheet. Confirmations on every platform are the
//! platform's own alert.

use std::rc::Rc;

use nsh_base::{alert_dialog, dialog, theme};
use nsh_core::{ModalAnimation, ModalPresentation, prelude::*};

use super::motion::entrance;

/// The panel's width in points on the desktops (DESIGN.md).
const STANDARD_WIDTH: f32 = 480.0;

/// The panel's width in points for config editors.
const EDITOR_WIDTH: f32 = 640.0;

/// The scrim's strength: `foreground` at 40 % (DESIGN.md "Dialog lift").
const SCRIM_OPACITY: f32 = 0.4;

/// The room a focus ring needs outside a field in points: its 2 pt outline and 2 pt offset (DESIGN.md "Inputs /
/// Fields"), a physical size equal to spacing step 1.
const RING_ROOM: f32 = 4.0;

/// How small the panel starts as it comes in: it grows into place where it will stay, never slides across the window.
const ENTER_SCALE: f32 = 0.96;

/// How far below its place the panel starts, in points (spacing step 2).
const ENTER_RISE: f32 = 8.0;

/// A dialog under construction: give it a body and a footer, then place it as a child anywhere in the page. It is on
/// screen while `open()` holds.
pub struct DialogFrame {
    open: Rc<dyn Fn() -> bool>,
    title: Reactive<SharedString>,
    on_close: Rc<dyn Fn()>,
    width: f32,
    body: Option<Rc<dyn Fn() -> View>>,
    footer: Option<Rc<dyn Fn() -> View>>,
    test_id: Option<SharedString>,
}

/// A dialog titled `title`, shown while `open()` holds. `on_close` runs when the person tries to dismiss it (Escape,
/// a press on the scrim, the back gesture or a swipe down on a phone); the caller then makes `open()` false.
///
/// ```ignore
/// dialog_frame(move || store.editing(), store.tr(|s| s.dialog_edit_title), move || store.cancel_edit())
///     .body(move || view().child(field(label, value, on_change)))
///     .footer(move || dialog_footer()
///         .child(action_button(ButtonKind::Ghost, cancel, move || store.cancel_edit()))
///         .child(action_button(ButtonKind::Primary, save, move || store.save())))
///     .test_id("edit-dialog")
/// ```
pub fn dialog_frame(
    open: impl Fn() -> bool + 'static,
    title: impl IntoReactive<SharedString>,
    on_close: impl Fn() + 'static,
) -> DialogFrame {
    DialogFrame {
        open: Rc::new(open),
        title: title.into_reactive(),
        on_close: Rc::new(on_close),
        width: STANDARD_WIDTH,
        body: None,
        footer: None,
        test_id: None,
    }
}

/// The action row of a dialog footer: right-aligned and 8 pt apart on the desktops, full-width buttons stacked on
/// phones. Add the buttons as children, the primary one last.
pub fn dialog_footer() -> View {
    if is_desktop() {
        view().flex_row().justify_end().gap_2()
    } else {
        view().gap_2()
    }
}

impl DialogFrame {
    /// A 640 pt panel for config editors instead of 480 pt.
    #[must_use]
    pub fn wide(mut self) -> Self {
        self.width = EDITOR_WIDTH;
        self
    }

    /// The content; made again on each opening, scrolls when it is taller than the window.
    #[must_use]
    pub fn body(mut self, body: impl Fn() -> View + 'static) -> Self {
        self.body = Some(Rc::new(body));
        self
    }

    /// The action row (see [`dialog_footer`]); made again on each opening.
    #[must_use]
    pub fn footer(mut self, footer: impl Fn() -> View + 'static) -> Self {
        self.footer = Some(Rc::new(footer));
        self
    }

    /// The hook tests find the panel by (`id:<test_id>`).
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished region (empty while closed).
    pub fn build(self) -> If {
        let Self {
            open,
            title,
            on_close,
            width,
            body,
            footer,
            test_id,
        } = self;
        when(
            move || open(),
            move || {
                let close_cancel = on_close.clone();
                let title_text = text(title.clone())
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .role(AccessibilityRole::Header)
                    .text_color(move || theme().foreground);
                let content = body.as_ref().map(|body| body());
                let actions = footer.as_ref().map(|footer| footer());
                let modal = dialog()
                    .close_on_escape(true)
                    .on_cancel(move || close_cancel());
                if is_desktop() {
                    let close_scrim = on_close.clone();
                    // The scrim and the panel fade in and out together as one layer (the platform snapshots it on
                    // the way out, so a body that empties as its state closes never shows); the panel also grows and
                    // rises into place. Under Reduce Motion it all appears and goes at once.
                    let layer_motion = if reduce_motion() {
                        ModalAnimation::None
                    } else {
                        ModalAnimation::Fade
                    };
                    let settle = entrance();
                    let panel = view()
                        .w(Points(width))
                        .max_w(relative(1.0))
                        .max_h(relative(1.0))
                        .gap_4()
                        .p_6()
                        .rounded(move || theme().radius)
                        .border_1()
                        .border_color(move || theme().border)
                        .bg(move || theme().card)
                        .shadow_lg()
                        .bind_scale(&settle.interpolate(&[0.0, 1.0], &[ENTER_SCALE, 1.0]))
                        .bind_translate_y(&settle.interpolate(&[0.0, 1.0], &[ENTER_RISE, 0.0]))
                        .child(title_text)
                        .when_some(content, |panel, content| {
                            // A focused field draws a 2 pt ring 2 pt outside its edge; the scroll view clips at
                            // its own edge, so the body is padded by that room inside the scroll and the scroll is
                            // pulled out by the same amount, keeping the body aligned with the title.
                            panel.child(
                                scroll()
                                    .flex_shrink(1.0)
                                    .mx(Points(-RING_ROOM))
                                    .child(view().p_1().child(content)),
                            )
                        })
                        .when_some(actions, |panel, actions| panel.child(actions));
                    let panel = match test_id.clone() {
                        Some(id) => panel.test_id(&id),
                        None => panel,
                    };
                    modal.animation(layer_motion).child(
                        view()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .p_4()
                            .child(
                                view()
                                    .absolute()
                                    .inset_0()
                                    .bg(move || theme().foreground.opacity(SCRIM_OPACITY))
                                    .on_press(move || close_scrim()),
                            )
                            .child(panel),
                    )
                } else {
                    // A phone: the whole screen is the page, the footer pinned under the scrolling body.
                    let page = view()
                        .size_full()
                        .bg(move || theme().background)
                        .child(view().px_4().pt_4().pb_2().child(title_text))
                        .child(
                            // The padding is inside the scroll, so a focused field's ring is never cut at its edge.
                            scroll().flex_1().child(
                                view()
                                    .px_4()
                                    .py_1()
                                    .when_some(content, |pad, content| pad.child(content)),
                            ),
                        )
                        .when_some(actions, |page, actions| {
                            page.child(
                                view()
                                    .px_4()
                                    .pt_3()
                                    .pb_safe_area_plus(Points(16.0))
                                    .border_t_1()
                                    .border_color(move || theme().border)
                                    .child(actions),
                            )
                        });
                    let page = match test_id.clone() {
                        Some(id) => page.test_id(&id),
                        None => page,
                    };
                    modal.presentation(ModalPresentation::PageSheet).child(page)
                }
            },
        )
    }
}

impl IntoElement for DialogFrame {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}

/// Asks the person to confirm an action in the platform's own alert, resolving `true` only when they choose
/// `confirm_label`; Cancel, Escape and a dismissal resolve `false`. Name the object and the consequence in
/// `description` ("Delete profile “home”? Its config and saved secret are removed from this device."). With
/// `destructive` the confirm action is drawn as destructive (Fault Red on the platforms that do). The alert opens
/// when this is called, so call it where the press happens and await the answer in a task.
///
/// ```ignore
/// let asked = confirm(title, description, delete_label, cancel_label, true);
/// spawn(async move {
///     if asked.await {
///         store.delete_profile(id);
///     }
/// });
/// ```
pub fn confirm(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    confirm_label: impl Into<SharedString>,
    cancel_label: impl Into<SharedString>,
    destructive: bool,
) -> impl Future<Output = bool> + 'static {
    let confirm_label = confirm_label.into();
    let options = alert_dialog()
        .title(title)
        .description(description)
        .cancel(cancel_label);
    let options = if destructive {
        options.destructive(confirm_label.clone())
    } else {
        options.action(confirm_label.clone())
    };
    let answer = options.show();
    async move {
        match answer.await {
            Ok(Some(chosen)) => chosen == confirm_label,
            Ok(None) => false,
            Err(error) => {
                log::warn!("the confirmation could not be shown: {error}");
                false
            }
        }
    }
}
