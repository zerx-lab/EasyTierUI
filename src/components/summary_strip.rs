//! The summary strip (DESIGN.md "Summary strip"): one panel row of 3-5 cells divided by hairlines. A cell is a Label
//! over a Mono value with a muted unit, an optional caption, and optional status-count bars (a 3 x 14 pt bar in the
//! lamp hue and a tabular count). Only facts the core reported belong here: hide a cell with no data.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    lamp::Lamp,
    text::{machine_text, meta_text, section_label},
};

/// The narrowest a cell gets in the grid, in points.
const CELL_MIN: &str = "repeat(auto-fit, minmax(176px, 1fr))";

/// The status bar's size in points (DESIGN.md: 3 x 14).
const BAR_WIDTH: f32 = 3.0;
const BAR_HEIGHT: f32 = 14.0;

/// A cell of a [`summary_strip`].
pub struct SummaryCell {
    label: Reactive<SharedString>,
    value: Reactive<SharedString>,
    unit: Option<Reactive<SharedString>>,
    caption: Option<Reactive<SharedString>>,
    bars: Vec<(Lamp, Reactive<SharedString>, Reactive<SharedString>)>,
    test_id: Option<SharedString>,
}

/// A cell captioned `label` with the headline `value` (Mono 20/28, tabular, one line).
///
/// ```ignore
/// summary_cell(store.tr(|s| s.summary_peers), move || peers_online.to_string())
///     .unit(store.tr(|s| s.unit_online))
///     .test_id("summary-peers")
/// ```
pub fn summary_cell(
    label: impl IntoReactive<SharedString>,
    value: impl IntoReactive<SharedString>,
) -> SummaryCell {
    SummaryCell {
        label: label.into_reactive(),
        value: value.into_reactive(),
        unit: None,
        caption: None,
        bars: Vec::new(),
        test_id: None,
    }
}

impl SummaryCell {
    /// A muted unit after the value ("ms", "of 4", "MiB/s").
    #[must_use]
    pub fn unit(mut self, unit: impl IntoReactive<SharedString>) -> Self {
        self.unit = Some(unit.into_reactive());
        self
    }

    /// A Meta line under the value ("median of 12 peers").
    #[must_use]
    pub fn caption(mut self, caption: impl IntoReactive<SharedString>) -> Self {
        self.caption = Some(caption.into_reactive());
        self
    }

    /// A status-count bar: a bar in the hue of `lamp`, the tabular `count` and its `word` ("direct", "relayed").
    /// The word carries the state; the bar only repeats it.
    #[must_use]
    pub fn bar(
        mut self,
        lamp: Lamp,
        count: impl IntoReactive<SharedString>,
        word: impl IntoReactive<SharedString>,
    ) -> Self {
        self.bars
            .push((lamp, count.into_reactive(), word.into_reactive()));
        self
    }

    /// The hook tests find the value by (`id:<test_id>`).
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    fn build(self) -> View {
        let Self {
            label,
            value,
            unit,
            caption,
            bars,
            test_id,
        } = self;
        let number = machine_text(value)
            .text_xl()
            .font_weight(FontWeight::MEDIUM)
            .number_of_lines(1)
            .text_color(move || theme().foreground);
        let number = match test_id {
            Some(id) => number.test_id(&id),
            None => number,
        };
        let line = view()
            .flex_row()
            .items_end()
            .gap_1()
            .child(number)
            .child(unit.map(|unit| meta_text(unit).number_of_lines(1).pb_0p5()));
        let counts = (!bars.is_empty()).then(|| {
            bars.into_iter().fold(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_x_3()
                    .gap_y_1(),
                |row, (lamp, count, word)| {
                    row.child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(
                                view()
                                    .w(Points(BAR_WIDTH))
                                    .h(Points(BAR_HEIGHT))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .accessible(false)
                                    .bg(move || lamp.hue()),
                            )
                            .child(machine_text(count).number_of_lines(1))
                            .child(meta_text(word).number_of_lines(1)),
                    )
                },
            )
        });
        view()
            .gap_1()
            .px_4()
            .py_3()
            .border_r_1()
            .border_b_1()
            .border_color(move || theme().border)
            .bg(move || theme().card)
            .child(section_label(label).number_of_lines(1))
            .child(line)
            .child(caption.map(|caption| meta_text(caption).number_of_lines(1)))
            .child(counts)
    }
}

/// A panel row of `cells` divided by hairlines: panel fill, hairline border, container radius, no shadow. The cells
/// share the width and wrap to a second row on a narrow window.
///
/// ```ignore
/// summary_strip(vec![
///     summary_cell(label_a, value_a),
///     summary_cell(label_b, value_b).bar(Lamp::Good, direct, direct_word).bar(Lamp::Degraded, relayed, relayed_word),
/// ])
/// ```
pub fn summary_strip(cells: Vec<SummaryCell>) -> View {
    // Each cell draws a hairline on its right and bottom edges; the grid is pulled 1 pt past the clipped frame so the
    // last column's and the last row's lines fall outside it and only dividers remain.
    view()
        .overflow_hidden()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .child(
            view()
                .grid()
                .grid_template_columns(CELL_MIN)
                .mr(Points(-1.0))
                .mb(Points(-1.0))
                .child(
                    cells
                        .into_iter()
                        .map(SummaryCell::build)
                        .collect::<Vec<_>>(),
                ),
        )
}
