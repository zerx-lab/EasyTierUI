//! The first run's one question (PRODUCT.md "A minute to connected"): is this a client or a control end? The answer
//! picks the roles the shell shows; Settings changes it later. The client is chosen until the user picks the other.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    components::{ButtonKind, ICON_SIZE, Icon, action_button, body_text, icon_colored, meta_text},
    core::role::WorkMode,
    i18n::Pick,
    store::Store,
};

/// The widest the question gets on a large window, in points.
const MAX_WIDTH: f32 = 560.0;

/// The work-mode question: two presets, one checked, and a button that takes it.
pub fn welcome(store: Store) -> View {
    let choice = signal(WorkMode::Client);
    let options: Vec<_> = WorkMode::PRESETS
        .into_iter()
        .map(|mode| option(store, mode, choice))
        .collect();
    view().flex_1().items_center().justify_center().p_6().child(
        view()
            .w_full()
            .max_w(Points(MAX_WIDTH))
            .gap_6()
            .child(
                view()
                    .gap_2()
                    .child(
                        text(store.tr(|s| s.mode_title))
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .role(AccessibilityRole::Header)
                            .text_color(move || theme().foreground),
                    )
                    .child(meta_text(store.tr(|s| s.mode_intro)).text_sm()),
            )
            .child(
                view()
                    .role(AccessibilityRole::Radiogroup)
                    .label(store.tr(|s| s.mode_title))
                    .gap_3()
                    .child(options),
            )
            .child(
                view()
                    .items_end()
                    .test_id("mode-continue")
                    .child(action_button(
                        ButtonKind::Primary,
                        store.tr(|s| s.mode_continue),
                        move || store.set_mode(choice.get_untracked()),
                    )),
            ),
    )
}

/// A preset as a pressable card: its icon, name and what it shows; Cobalt edge and ring while checked.
fn option(store: Store, mode: WorkMode, choice: Signal<WorkMode>) -> View {
    let (glyph, title, body, key): (Icon, Pick, Pick, &str) = match mode {
        WorkMode::Control => (
            Icon::Server,
            |s| s.mode_control,
            |s| s.mode_control_body,
            "control",
        ),
        WorkMode::Client | WorkMode::Custom(_) => (
            Icon::Network,
            |s| s.mode_client,
            |s| s.mode_client_body,
            "client",
        ),
    };
    let selected = move || choice.get() == mode;
    let tint = move || {
        if selected() {
            theme().primary
        } else {
            theme().border
        }
    };
    let row = view()
        .test_id(&format!("mode-{key}"))
        .role(AccessibilityRole::Radio)
        .accessibility_state(move || AccessibilityState {
            selected: selected(),
            ..AccessibilityState::default()
        })
        .on_press(move || choice.set(mode))
        .flex_row()
        .items_start()
        .gap_3()
        .p_4()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(tint)
        .bg(move || theme().card)
        .hover(|s| s.bg(move || theme().accent))
        .child(
            view()
                .pt_0p5()
                .child(icon_colored(glyph, ICON_SIZE, move || {
                    if selected() {
                        theme().primary
                    } else {
                        theme().muted_foreground
                    }
                })),
        )
        .child(
            view()
                .flex_1()
                .gap_1()
                .child(body_text(store.tr(title)).font_weight(FontWeight::SEMIBOLD))
                .child(meta_text(store.tr(body)).text_sm()),
        )
        .child(radio_dot(selected));
    if is_desktop() { row } else { row.min_h_11() }
}

/// The radio indicator of a card, as `choice_row` draws it.
fn radio_dot(selected: impl Fn() -> bool + Copy + 'static) -> View {
    view()
        .size_4()
        .mt_0p5()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(move || {
            if selected() {
                theme().primary
            } else {
                theme().input
            }
        })
        .child(view().size_2().rounded_full().bg(move || {
            if selected() {
                theme().primary
            } else {
                AdaptiveColor::TRANSPARENT
            }
        }))
}
