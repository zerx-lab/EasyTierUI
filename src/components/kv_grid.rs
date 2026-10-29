//! The key-value grid (DESIGN.md "Key-value grid"): facts as a Label over a value, in columns of at least 180 pt that
//! fit the width, so a drawer never shows a near-empty second row.

use std::rc::Rc;

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    data::{CopyLabels, copy_button, read},
    text::{body_text, machine_text, section_label},
};

/// The narrowest a column gets, in points.
const COLUMN_MIN: &str = "repeat(auto-fit, minmax(180px, 1fr))";

/// One fact of a [`kv_grid`].
pub struct Fact {
    label: Reactive<SharedString>,
    value: Reactive<SharedString>,
    mono: bool,
    copy: Option<CopyLabels>,
    test_id: Option<SharedString>,
}

/// A fact named `label` with the value `value`, in the UI font (a person's words: a hostname the user chose, a
/// state). A missing value (empty text) shows a muted dash.
///
/// ```ignore
/// fact(store.tr(|s| s.field_hostname), move || machine.hostname.clone())
/// ```
pub fn fact(
    label: impl IntoReactive<SharedString>,
    value: impl IntoReactive<SharedString>,
) -> Fact {
    Fact {
        label: label.into_reactive(),
        value: value.into_reactive(),
        mono: false,
        copy: None,
        test_id: None,
    }
}

/// A fact whose value is machine data (an address, a version, a count): Mono with tabular figures, one line.
///
/// ```ignore
/// fact_mono(store.tr(|s| s.field_virtual_ip), move || node.virtual_ip.clone())
/// ```
pub fn fact_mono(
    label: impl IntoReactive<SharedString>,
    value: impl IntoReactive<SharedString>,
) -> Fact {
    Fact {
        mono: true,
        ..fact(label, value)
    }
}

impl Fact {
    /// Adds a copy button after the value.
    #[must_use]
    pub fn copyable(mut self, labels: CopyLabels) -> Self {
        self.copy = Some(labels);
        self
    }

    /// The hook tests find the value by (`id:<test_id>`); the copy button is `<test_id>-copy`.
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    fn build(self) -> View {
        let Self {
            label,
            value,
            mono,
            copy,
            test_id,
        } = self;
        let shown = value.clone();
        let present = {
            let value = value.clone();
            move || !read(&value).is_empty()
        };
        let text_of = move || {
            let text = read(&shown);
            if text.is_empty() {
                SharedString::from("\u{2014}")
            } else {
                text
            }
        };
        let content = if mono {
            machine_text(text_of)
        } else {
            body_text(text_of)
        }
        .number_of_lines(1)
        .selectable(true)
        .text_color(move || {
            if present() {
                theme().foreground
            } else {
                theme().muted_foreground
            }
        });
        let content = match test_id.clone() {
            Some(id) => content.test_id(&id),
            None => content,
        };
        let copy_value = value;
        let copier = copy.map(|labels| {
            let id = test_id.as_ref().map(|id| format!("{id}-copy"));
            copy_button(
                Rc::new(move || read(&copy_value).to_string()),
                labels,
                id.as_deref(),
            )
        });
        view().gap_1().min_w_0().child(section_label(label)).child(
            view()
                .flex_row()
                .items_center()
                .gap_1()
                .child(content.min_w_0().flex_shrink(1.0))
                .child(copier),
        )
    }
}

/// Facts in an auto-fitting grid: columns of at least 180 pt, 16 pt apart vertically and 24 pt horizontally.
///
/// ```ignore
/// kv_grid(vec![
///     fact_mono(store.tr(|s| s.field_virtual_ip), move || ip.clone()),
///     fact(store.tr(|s| s.field_nat), move || nat.clone()),
/// ])
/// ```
pub fn kv_grid(facts: Vec<Fact>) -> View {
    view()
        .grid()
        .grid_template_columns(COLUMN_MIN)
        .gap_x_6()
        .gap_y_4()
        .child(facts.into_iter().map(Fact::build).collect::<Vec<_>>())
}
