//! Charts (DESIGN.md "Charts"): [`metric_chart`], a live line or area chart of in-memory samples with a hairline grid,
//! a unit-formatted y axis and a legend of swatches and words above it, and [`sparkline`], a 64 x 20 pt trend in a
//! table cell. Both draw only samples the core reported and show an empty state before there are two.
//!
//! The plot is made again whenever a sample arrives, with its values read once at that moment (nsh-base components
//! must not have reactive closures bound into them inside a branch that re-renders), so the charts are not
//! interactive: the readout is the axis and the numbers beside it.

use std::rc::Rc;

use nsh_base::{
    plot::{Curve, Scale, area, grid, line, plot},
    theme,
};
use nsh_core::{
    element::{RowAccessor, Rows},
    prelude::*,
};

use super::{
    data::read,
    text::{machine_text, meta_text},
};

/// The y axis' labels column, in points: wide enough for "1023.9 KiB/s".
const AXIS_WIDTH: f32 = 72.0;

/// The default plot height in points.
const DEFAULT_HEIGHT: f32 = 160.0;

/// The sparkline's size in points.
const SPARK_WIDTH: f32 = 64.0;
const SPARK_HEIGHT: f32 = 20.0;

/// A series' swatch size in points (DESIGN.md: 8 pt).
const SWATCH: f32 = 8.0;

/// How strongly a filled series tints the area under its line.
const AREA_OPACITY: f32 = 0.14;

/// How strongly the grid shows (the `border` token at low opacity).
const GRID_OPACITY: f32 = 0.7;

/// What a chart's y axis measures; it formats the axis labels and the numbers beside charts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartUnit {
    /// Bytes per second: B/s, KiB/s, MiB/s, GiB/s.
    BytesPerSecond,
    /// Milliseconds: ms, and s from 1000.
    Milliseconds,
    /// A percentage: 0-100 %.
    Percent,
    /// A plain count.
    Count,
}

impl ChartUnit {
    /// `value` with this unit ("1.5 MiB/s", "23 ms", "4 %"); a value that is not a number is a dash.
    pub fn format(self, value: f64) -> String {
        if !value.is_finite() {
            return "\u{2014}".to_owned();
        }
        match self {
            Self::BytesPerSecond => {
                const STEPS: [&str; 4] = ["B/s", "KiB/s", "MiB/s", "GiB/s"];
                let mut scaled = value;
                let mut steps = STEPS.iter();
                let mut unit = steps.next().copied().unwrap_or("B/s");
                while scaled.abs() >= 1024.0 {
                    let Some(next) = steps.next() else { break };
                    scaled /= 1024.0;
                    unit = next;
                }
                format!("{} {unit}", trimmed(scaled))
            }
            Self::Milliseconds => {
                if value.abs() >= 1000.0 {
                    format!("{} s", trimmed(value / 1000.0))
                } else {
                    format!("{} ms", trimmed(value))
                }
            }
            Self::Percent => format!("{} %", trimmed(value)),
            Self::Count => trimmed(value),
        }
    }
}

/// `value` with one decimal below 100, none above, and no trailing `.0`.
fn trimmed(value: f64) -> String {
    let text = if value.abs() >= 100.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    text.strip_suffix(".0")
        .map_or_else(|| text.clone(), str::to_owned)
}

/// The smallest 1, 2, 5 times a power of ten that holds `value`: a tidy top for an axis.
fn tidy_ceiling(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 1.0;
    }
    let magnitude = 10f64.powf(value.log10().floor());
    let mantissa = value / magnitude;
    let step = if mantissa <= 1.0 {
        1.0
    } else if mantissa <= 2.0 {
        2.0
    } else if mantissa <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * magnitude
}

/// The color of a series: one of the red-free chart tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesColor {
    /// Received traffic: `chart-1`.
    Received,
    /// Sent traffic: `chart-2`.
    Sent,
    /// A further series: `chart-1` to `chart-5` by number (1-based; other numbers fall back to `chart-5`).
    Chart(u8),
}

impl SeriesColor {
    /// A stable color for `id` (a peer's id): the same id always gets the same one, in every chart.
    pub fn hashed(id: &str) -> Self {
        // FNV-1a over the bytes.
        let hash = id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
        let slot = u8::try_from(hash.checked_rem(5).unwrap_or(0)).unwrap_or(0);
        Self::Chart(slot.saturating_add(1))
    }

    /// The token's color.
    pub fn color(self) -> AdaptiveColor {
        let theme = theme();
        match self {
            Self::Received | Self::Chart(1) => theme.chart_1,
            Self::Sent | Self::Chart(2) => theme.chart_2,
            Self::Chart(3) => theme.chart_3,
            Self::Chart(4) => theme.chart_4,
            Self::Chart(_) => theme.chart_5,
        }
    }
}

/// A series of a chart: its name, color and samples.
pub struct ChartSeries {
    label: Reactive<SharedString>,
    color: SeriesColor,
    samples: Rc<dyn Fn() -> Vec<f64>>,
    filled: bool,
}

/// A series named `label` drawn in `color` from `samples()` (oldest first, evenly spaced; `NaN` is a gap). The
/// samples are read in the chart's binding, so a new sample redraws it.
///
/// ```ignore
/// chart_series(store.tr(|s| s.legend_received), SeriesColor::Received, move || history.rx_rates())
///     .filled()
/// ```
pub fn chart_series(
    label: impl IntoReactive<SharedString>,
    color: SeriesColor,
    samples: impl Fn() -> Vec<f64> + 'static,
) -> ChartSeries {
    ChartSeries {
        label: label.into_reactive(),
        color,
        samples: Rc::new(samples),
        filled: false,
    }
}

impl ChartSeries {
    /// Tints the area under the line.
    #[must_use]
    pub fn filled(mut self) -> Self {
        self.filled = true;
        self
    }
}

/// A chart under construction.
pub struct MetricChart {
    label: Reactive<SharedString>,
    series: Vec<ChartSeries>,
    unit: ChartUnit,
    empty: Reactive<SharedString>,
    height: f32,
    span: Option<(Reactive<SharedString>, Reactive<SharedString>)>,
    test_id: Option<SharedString>,
}

/// A chart named `label` (for screen readers) of `series` measured in `unit`; `empty` is the sentence shown before
/// there are two samples ("Collecting samples\u{2026}").
///
/// ```ignore
/// metric_chart(
///     store.tr(|s| s.chart_throughput),
///     vec![
///         chart_series(rx_word, SeriesColor::Received, move || history.rx()).filled(),
///         chart_series(tx_word, SeriesColor::Sent, move || history.tx()).filled(),
///     ],
///     ChartUnit::BytesPerSecond,
///     store.tr(|s| s.chart_collecting),
/// )
/// .span(store.tr(|s| s.chart_three_minutes_ago), store.tr(|s| s.chart_now))
/// .test_id("throughput-chart")
/// ```
pub fn metric_chart(
    label: impl IntoReactive<SharedString>,
    series: Vec<ChartSeries>,
    unit: ChartUnit,
    empty: impl IntoReactive<SharedString>,
) -> MetricChart {
    MetricChart {
        label: label.into_reactive(),
        series,
        unit,
        empty: empty.into_reactive(),
        height: DEFAULT_HEIGHT,
        span: None,
        test_id: None,
    }
}

impl MetricChart {
    /// The plot's height in points (default 160).
    #[must_use]
    pub fn height(mut self, points: f32) -> Self {
        self.height = points;
        self
    }

    /// The words under the plot's two ends: how far back it reaches and "now".
    #[must_use]
    pub fn span(
        mut self,
        from: impl IntoReactive<SharedString>,
        to: impl IntoReactive<SharedString>,
    ) -> Self {
        self.span = Some((from.into_reactive(), to.into_reactive()));
        self
    }

    /// The hook tests find the chart by (`id:<test_id>`); its empty state is `<test_id>-empty`.
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view.
    pub fn build(self) -> View {
        let Self {
            label,
            series,
            unit,
            empty,
            height,
            span,
            test_id,
        } = self;
        let legend = series.iter().fold(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_x_4()
                .gap_y_1(),
            |row, series| {
                let color = series.color;
                row.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .child(
                            view()
                                .w(Points(SWATCH))
                                .h(Points(SWATCH))
                                .flex_shrink_0()
                                .rounded_full()
                                .accessible(false)
                                .bg(color.color()),
                        )
                        .child(meta_text(series.label.clone()).number_of_lines(1)),
                )
            },
        );
        let empty_id = test_id.as_ref().map(|id| format!("{id}-empty"));
        let spans = span;
        let body = dynamic(move || {
            let columns: Vec<Vec<f64>> = series.iter().map(|series| (series.samples)()).collect();
            let name = read(&label);
            let hint = read(&empty);
            let kinds: Vec<(SeriesColor, bool)> =
                series.iter().map(|s| (s.color, s.filled)).collect();
            let spans = spans.as_ref().map(|(from, to)| (read(from), read(to)));
            let empty_id = empty_id.clone();
            untracked(move || {
                chart_body(Snapshot {
                    name,
                    hint,
                    columns,
                    kinds,
                    unit,
                    height,
                    span: spans,
                    empty_id,
                })
            })
        });
        let chart = view().gap_2().child(legend).child(body);
        match test_id {
            Some(id) => chart.test_id(&id),
            None => chart,
        }
    }
}

impl IntoElement for MetricChart {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}

/// How many samples the longest series has.
fn longest(columns: &[Vec<f64>]) -> usize {
    columns.iter().map(Vec::len).max().unwrap_or(0)
}

/// The rows of a plot from `columns`: one numeric field `s<n>` per series.
fn rows_of(columns: &[Vec<f64>]) -> Rows {
    Rows::from_columns(
        longest(columns),
        columns
            .iter()
            .enumerate()
            .map(|(index, values)| (SharedString::from(format!("s{index}")), values.clone())),
    )
}

/// The largest sample of any series, if there is one.
fn peak(columns: &[Vec<f64>]) -> Option<f64> {
    columns
        .iter()
        .flatten()
        .copied()
        .filter(|value| value.is_finite())
        .max_by(f64::total_cmp)
}

/// What a chart shows at one moment: every value it reads, taken together so the plot is built from constants.
struct Snapshot {
    name: SharedString,
    hint: SharedString,
    columns: Vec<Vec<f64>>,
    kinds: Vec<(SeriesColor, bool)>,
    unit: ChartUnit,
    height: f32,
    span: Option<(SharedString, SharedString)>,
    empty_id: Option<String>,
}

/// The plot, its axis and its span words, or the empty state.
fn chart_body(snapshot: Snapshot) -> View {
    let Snapshot {
        name,
        hint,
        columns,
        kinds,
        unit,
        height,
        span,
        empty_id,
    } = snapshot;
    if longest(&columns) < 2 {
        let note = meta_text(hint).text_center();
        let note = match empty_id {
            Some(id) => note.test_id(&id),
            None => note,
        };
        return view()
            .h(Points(height))
            .items_center()
            .justify_center()
            .rounded(move || theme().radius)
            .border_1()
            .border_color(move || theme().border)
            .child(note);
    }
    let top = tidy_ceiling(peak(&columns).unwrap_or(0.0) * 1.1);
    let middle = top / 2.0;
    let ink = theme().border.opacity(GRID_OPACITY);
    let marks = kinds.iter().enumerate().fold(
        plot()
            .data(rows_of(&columns))
            .y_scale(Scale::Linear {
                domain: Some([0.0, top]),
            })
            .interactive(false)
            .accessibility_label(name)
            .h(Points(height))
            .flex_1()
            .child(
                grid()
                    .y(vec![0.0, middle, top])
                    .stroke_color(ink)
                    .stroke_width(px(1.0)),
            ),
        |plot, (index, (color, filled))| {
            let key = RowAccessor::key(format!("s{index}"));
            let stroke = color.color();
            let plot = if *filled {
                plot.child(
                    area()
                        .y1(key.clone())
                        .curve(Curve::Linear)
                        .fill_color(stroke.opacity(AREA_OPACITY)),
                )
            } else {
                plot
            };
            plot.child(
                line()
                    .y(key)
                    .curve(Curve::Linear)
                    .stroke_color(stroke)
                    .stroke_width(px(1.5)),
            )
        },
    );
    let axis = view()
        .w(Points(AXIS_WIDTH))
        .h(Points(height))
        .flex_shrink_0()
        .justify_between()
        .items_end()
        .accessible(false)
        .child(axis_label(unit.format(top)))
        .child(axis_label(unit.format(middle)))
        .child(axis_label(unit.format(0.0)));
    let plot_row = view().flex_row().gap_2().child(axis).child(marks);
    let ends = span.map(|(from, to)| {
        view()
            .flex_row()
            .justify_between()
            .pl(Points(AXIS_WIDTH + 8.0))
            .child(meta_text(from))
            .child(meta_text(to))
    });
    view().gap_1().child(plot_row).child(ends)
}

/// A y-axis label: Meta size, Mono, `muted-foreground`, one line.
fn axis_label(text: String) -> Text {
    machine_text(text)
        .text_xs()
        .number_of_lines(1)
        .text_color(move || theme().muted_foreground)
}

/// A 64 x 20 pt trend line of `samples()` in `color` for a table cell. With fewer than two samples the box stays
/// empty, so the column keeps its rhythm; the numbers beside it are the readout.
///
/// ```ignore
/// sparkline(move || history.rx_rates(), SeriesColor::Received)
/// ```
pub fn sparkline(samples: impl Fn() -> Vec<f64> + 'static, color: SeriesColor) -> View {
    let drawn = dynamic(move || {
        let values = samples();
        untracked(move || {
            if values.len() < 2 {
                return view().into_element();
            }
            let top = tidy_ceiling(peak(std::slice::from_ref(&values)).unwrap_or(0.0));
            plot()
                .data(rows_of(&[values]))
                .y_scale(Scale::Linear {
                    domain: Some([0.0, top]),
                })
                .interactive(false)
                .size_full()
                .child(
                    line()
                        .y(RowAccessor::key("s0"))
                        .curve(Curve::Linear)
                        .stroke_color(color.color())
                        .stroke_width(px(1.5)),
                )
                .into_element()
        })
    });
    view()
        .w(Points(SPARK_WIDTH))
        .h(Points(SPARK_HEIGHT))
        .flex_shrink_0()
        .accessible(false)
        .child(drawn)
}
