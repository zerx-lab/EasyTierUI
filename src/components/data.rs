//! Machine-data building blocks (DESIGN.md "Machine Voice Rule"): the headline number, a copyable value, a code
//! block and the "updated N s ago" line.

use std::{rc::Rc, time::Duration};

use nsh_base::theme;
use nsh_core::{prelude::*, time::unix_ms};

use super::{
    button::{ButtonKind, action_button},
    text::{machine_text, meta_text, tagged},
};

/// How long a copy button says "Copied" before it offers the copy again.
const COPIED_FOR: Duration = Duration::from_millis(1500);

/// The words of a copy button, which components cannot know: the label that offers the copy and the one that
/// confirms it.
#[derive(Clone)]
pub struct CopyLabels {
    copy: Reactive<SharedString>,
    copied: Reactive<SharedString>,
}

impl CopyLabels {
    /// The label `copy` ("Copy") and the confirmation `copied` ("Copied"), each text or a closure.
    ///
    /// ```ignore
    /// CopyLabels::new(store.tr(|s| s.action_copy), store.tr(|s| s.action_copied))
    /// ```
    pub fn new(
        copy: impl IntoReactive<SharedString>,
        copied: impl IntoReactive<SharedString>,
    ) -> Self {
        Self {
            copy: copy.into_reactive(),
            copied: copied.into_reactive(),
        }
    }
}

/// The current value of `reactive`, tracked when it is bound.
pub(super) fn read<T: Clone>(reactive: &Reactive<T>) -> T {
    match reactive {
        Reactive::Static(value) => value.clone(),
        Reactive::Bound(f) => f(),
    }
}

/// A ghost button that puts `value()` on the clipboard when pressed (and only then: nothing is copied while
/// rendering) and says "Copied" for a moment. `test_id` marks it.
pub(super) fn copy_button(
    value: Rc<dyn Fn() -> String>,
    labels: CopyLabels,
    test_id: Option<&str>,
) -> AnyElement {
    let copied = signal(false);
    let label = move || {
        if copied.get() {
            read(&labels.copied)
        } else {
            read(&labels.copy)
        }
    };
    let button = action_button(ButtonKind::Ghost, label, move || {
        set_clipboard(value());
        copied.set(true);
        let _timer = set_timeout(COPIED_FOR, move || {
            // The button may be gone by then: a disposed signal is simply not written.
            if !copied.try_set(false) {
                log::debug!("copy button was disposed before its confirmation timed out");
            }
        });
    });
    tagged(button, test_id)
}

/// The one headline number of a card (DESIGN.md "Stat"): monospace 24/32, medium, tabular figures.
///
/// ```ignore
/// stat(move || store.peers_online().to_string())
/// ```
pub fn stat(value: impl IntoReactive<SharedString>) -> Text {
    text(value)
        .text_2xl()
        .font_family("monospace")
        .tabular_nums()
        .font_weight(FontWeight::MEDIUM)
        .text_color(move || theme().foreground)
}

/// A [`stat`] with its Meta caption under it.
///
/// ```ignore
/// stat_captioned(move || store.peers_online().to_string(), store.tr(|s| s.stat_peers_online))
/// ```
pub fn stat_captioned(
    value: impl IntoReactive<SharedString>,
    caption: impl IntoReactive<SharedString>,
) -> View {
    view().child(stat(value)).child(meta_text(caption))
}

/// Machine text (an address, an id) that can be selected, with a copy button after it. `test_id` marks the text;
/// the button is `<test_id>-copy`.
///
/// ```ignore
/// copy_value("virtual-ip", move || store.virtual_ip(), CopyLabels::new(copy, copied))
/// ```
pub fn copy_value(
    test_id: &str,
    value: impl Fn() -> String + Clone + 'static,
    labels: CopyLabels,
) -> View {
    let shown = value.clone();
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            machine_text(move || SharedString::from(shown()))
                .test_id(test_id)
                .selectable(true)
                .text_color(move || theme().foreground),
        )
        .child(copy_button(
            Rc::new(value),
            labels,
            Some(&format!("{test_id}-copy")),
        ))
}

/// A block of monospace text for TOML, YAML and logs: well fill, container radius, selectable, scrolling past about
/// 320 pt, with a copy button above it. `test_id` marks the text; the button is `<test_id>-copy`.
///
/// ```ignore
/// code_block("config-toml", move || store.config_text(), CopyLabels::new(copy, copied))
/// ```
pub fn code_block(
    test_id: &str,
    content: impl Fn() -> String + Clone + 'static,
    labels: CopyLabels,
) -> View {
    let shown = content.clone();
    view()
        .gap_1()
        .p_2()
        .rounded(move || theme().radius)
        .bg(move || theme().muted)
        .child(view().flex_row().justify_end().child(copy_button(
            Rc::new(content),
            labels,
            Some(&format!("{test_id}-copy")),
        )))
        .child(
            scroll().max_h_80().child(
                machine_text(move || SharedString::from(shown()))
                    .test_id(test_id)
                    .selectable(true)
                    .px_2()
                    .pb_2()
                    .text_color(move || theme().foreground),
            ),
        )
}

/// "Updated 3 s ago" in Meta type, re-rendered every second so it never goes stale. `updated_ms` is the Unix time in
/// milliseconds of the last update (`nsh_core::time::unix_ms`), or `None` before the first one (the text is empty
/// then). `template` holds `{age}`, replaced by the whole seconds since (`"Updated {age} s ago"`); `just_now` is
/// shown for the first second.
///
/// ```ignore
/// age_text(move || store.last_update_ms(), store.tr(|s| s.updated_ago), store.tr(|s| s.updated_now))
/// ```
pub fn age_text(
    updated_ms: impl Fn() -> Option<f64> + 'static,
    template: impl IntoReactive<SharedString>,
    just_now: impl IntoReactive<SharedString>,
) -> Text {
    let template = template.into_reactive();
    let just_now = just_now.into_reactive();
    let now = signal(unix_ms());
    // The ticker ends with the component that owns this text; dropping its handle does not cancel it.
    let _ticker = set_interval(Duration::from_secs(1), move || {
        now.set(unix_ms());
    });
    meta_text(move || {
        let Some(updated) = updated_ms() else {
            return SharedString::default();
        };
        // The clock may step backwards; an age is never negative.
        let seconds = ((now.get() - updated) / 1000.0).floor().max(0.0);
        if seconds < 1.0 {
            read(&just_now)
        } else {
            let template = read(&template);
            SharedString::from(template.replace("{age}", &format!("{seconds:.0}")))
        }
    })
}
