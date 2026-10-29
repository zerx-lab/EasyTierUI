//! Charts (DESIGN.md "Charts"): [`metric_chart`], a live line or area chart of in-memory samples over time with a
//! hairline grid, a unit-formatted y axis and a legend of swatches and words above it, and [`sparkline`], a 64 x 20 pt
//! trend in a table cell. Both draw only samples the core reported and show an empty state before there are two.
//!
//! The plot is mounted once and follows its samples through its props: a new sample re-records its marks instead of
//! making a new plot. Samples sit at the moment they were read on a time axis that ends at the newest one, so a gap
//! in the readings is a gap on the axis and a fresh network fills the window from the right. The charts are not
//! interactive: the readout is the axis and the numbers beside it.

use std::rc::Rc;

use nsh_base::theme;
use nsh_core::{
    canvas::{LineCap, LineJoin},
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

/// The grid's stroke: a 1 pt hairline (DESIGN.md "Borders"), a physical width rather than a step of the scale.
const HAIRLINE: f32 = 1.0;

/// A series' stroke: 1.5 pt, the stroke of the app's drawn icons.
const LINE_WIDTH: f32 = 1.5;

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

/// A tidy top for an axis of `unit` that holds `value`. Bytes are tidied in the binary unit they are shown in
/// (`2 KiB/s` is 2048 B/s), so the top and the middle line read as round numbers in that unit.
fn axis_top(unit: ChartUnit, value: f64) -> f64 {
    if unit != ChartUnit::BytesPerSecond || !value.is_finite() || value < 1024.0 {
        return tidy_ceiling(value);
    }
    let mut scale = 1024.0;
    while value / scale >= 1024.0 && scale < 1024f64.powi(3) {
        scale *= 1024.0;
    }
    tidy_ceiling(value / scale) * scale
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
    color: Rc<dyn Fn() -> SeriesColor>,
    samples: Rc<dyn Fn() -> Vec<f64>>,
    shown: Option<Rc<dyn Fn() -> bool>>,
    filled: bool,
}

/// A series named `label` drawn in `color` from `samples()`: one value per moment of the chart's times, oldest
/// first (`NaN` is a gap). The samples are read in the plot's binding, so a new sample redraws it.
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
        color: Rc::new(move || color),
        samples: Rc::new(samples),
        shown: None,
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

    /// A color that follows `color()` (a slot whose peer changes).
    #[must_use]
    pub fn color_with(mut self, color: impl Fn() -> SeriesColor + 'static) -> Self {
        self.color = Rc::new(color);
        self
    }

    /// Lists the series in the legend only while `shown()` holds (a slot with nothing in it).
    #[must_use]
    pub fn shown_while(mut self, shown: impl Fn() -> bool + 'static) -> Self {
        self.shown = Some(Rc::new(shown));
        self
    }
}

/// A chart under construction.
pub struct MetricChart {
    label: Reactive<SharedString>,
    times: Rc<dyn Fn() -> Vec<f64>>,
    series: Vec<ChartSeries>,
    unit: ChartUnit,
    empty: Reactive<SharedString>,
    height: f32,
    span: Option<(f64, Reactive<SharedString>, Reactive<SharedString>)>,
    test_id: Option<SharedString>,
}

/// A chart named `label` (for screen readers) of `series` measured in `unit`, their samples taken at `times()`
/// (Unix milliseconds, oldest first, one per sample of every series); `empty` is the sentence shown before there are
/// two samples ("Collecting samples\u{2026}").
///
/// ```ignore
/// metric_chart(
///     store.tr(|s| s.chart_throughput),
///     move || history.times(),
///     vec![
///         chart_series(rx_word, SeriesColor::Received, move || history.rx()).filled(),
///         chart_series(tx_word, SeriesColor::Sent, move || history.tx()).filled(),
///     ],
///     ChartUnit::BytesPerSecond,
///     store.tr(|s| s.chart_collecting),
/// )
/// .span(180_000.0, store.tr(|s| s.chart_three_minutes_ago), store.tr(|s| s.chart_now))
/// .test_id("throughput-chart")
/// ```
pub fn metric_chart(
    label: impl IntoReactive<SharedString>,
    times: impl Fn() -> Vec<f64> + 'static,
    series: Vec<ChartSeries>,
    unit: ChartUnit,
    empty: impl IntoReactive<SharedString>,
) -> MetricChart {
    MetricChart {
        label: label.into_reactive(),
        times: Rc::new(times),
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

    /// The time axis spans the last `window_ms` up to the newest sample, with the words under its two ends: how far
    /// back it reaches and "now". Without a span the axis runs from the oldest sample to the newest.
    #[must_use]
    pub fn span(
        mut self,
        window_ms: f64,
        from: impl IntoReactive<SharedString>,
        to: impl IntoReactive<SharedString>,
    ) -> Self {
        self.span = Some((window_ms, from.into_reactive(), to.into_reactive()));
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
            times,
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
            |row, series| row.child(legend_item(series)),
        );
        let window = span.as_ref().map(|(window, _, _)| *window);
        let samples: Vec<Rc<dyn Fn() -> Vec<f64>>> =
            series.iter().map(|series| series.samples.clone()).collect();
        let frame = computed(move || {
            let columns: Vec<Vec<f64>> = samples.iter().map(|samples| samples()).collect();
            Frame::of(times(), columns, unit, window)
        });
        let drawable = move || frame.with(|frame| frame.drawable);
        let top = move || frame.with(|frame| frame.top);
        let kinds: Vec<(Rc<dyn Fn() -> SeriesColor>, bool)> = series
            .iter()
            .map(|series| (series.color.clone(), series.filled))
            .collect();
        let size = signal(Size::<Points>::default());
        let marks = canvas(move || {
            let area = size.get();
            let grid = color(theme().border.opacity(GRID_OPACITY));
            let inks: Vec<(Hsla, Hsla, bool)> = kinds
                .iter()
                .map(|(ink, filled)| {
                    let ink = ink().color();
                    (color(ink), color(ink.opacity(AREA_OPACITY)), *filled)
                })
                .collect();
            frame.with(|frame| frame.paint(area, Some(grid), &inks))
        })
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .left_0()
        .accessible(false);
        let empty_id = test_id.as_ref().map(|id| format!("{id}-empty"));
        let waiting = move || {
            let note = meta_text(empty.clone()).text_center();
            let note = match &empty_id {
                Some(id) => note.test_id(id),
                None => note,
            };
            view()
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .left_0()
                .items_center()
                .justify_center()
                .rounded(move || theme().radius)
                .border_1()
                .border_color(move || theme().border)
                .child(note)
        };
        let axis = view()
            .w(Points(AXIS_WIDTH))
            .h(Points(height))
            .flex_shrink_0()
            .accessible(false)
            .child(when(drawable, move || {
                view()
                    .flex_1()
                    .justify_between()
                    .items_end()
                    .child(axis_label(move || unit.format(top())))
                    .child(axis_label(move || unit.format(top() / 2.0)))
                    .child(axis_label(move || unit.format(0.0)))
            }));
        let area = view()
            .flex_1()
            .h(Points(height))
            .role(AccessibilityRole::Image)
            .label(move || read(&label))
            .on_layout(move |bounds: &Bounds<Points>| size.set(bounds.size))
            .child(marks)
            .child(when(move || !drawable(), waiting));
        let ends = span.map(|(_, from, to)| {
            view()
                .flex_row()
                .justify_between()
                .pl(Points(AXIS_WIDTH + 8.0))
                .child(meta_text(from))
                .child(meta_text(to))
        });
        let chart = view().gap_2().child(legend).child(
            view()
                .gap_1()
                .child(view().flex_row().gap_2().child(axis).child(area))
                .child(ends),
        );
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

/// A series' swatch and name in the legend, while it is shown.
fn legend_item(series: &ChartSeries) -> AnyElement {
    let color = series.color.clone();
    let item = view()
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
                .bg(move || color().color()),
        )
        .child(meta_text(series.label.clone()).number_of_lines(1));
    match series.shown.clone() {
        Some(shown) => {
            let item = Rc::new(item);
            when(move || shown(), move || (*item).clone()).into_element()
        }
        None => item.into_element(),
    }
}

/// What a chart draws at one moment: the times, a column per series, the time span of its x axis, the top of its y
/// axis, and whether there is enough to draw.
#[derive(Clone, PartialEq, Default)]
struct Frame {
    times: Vec<f64>,
    columns: Vec<Vec<f64>>,
    domain: [f64; 2],
    top: f64,
    drawable: bool,
}

impl Frame {
    /// The frame of `columns` read at `times`, its axis the last `window` milliseconds up to the newest time (all of
    /// them without a window).
    fn of(times: Vec<f64>, columns: Vec<Vec<f64>>, unit: ChartUnit, window: Option<f64>) -> Self {
        let first = times.iter().copied().find(|t| t.is_finite());
        let last = times.iter().rev().copied().find(|t| t.is_finite());
        let drawable = times.iter().filter(|t| t.is_finite()).count() >= 2;
        let domain = match (first, last, window) {
            (_, Some(last), Some(window)) => [last - window, last],
            (Some(first), Some(last), None) => [first, last],
            _ => [0.0, 1.0],
        };
        let top = axis_top(unit, peak(&columns).unwrap_or(0.0) * 1.1);
        Self {
            times,
            columns,
            domain,
            top,
            drawable,
        }
    }

    /// Where the sample read at `t` with value `value` goes in an `area` (`None` for a gap): `t` along the domain,
    /// `value` from the bottom to the top, kept [`LINE_WIDTH`] inside the top and bottom edges so a stroke along
    /// them is not cut in half.
    fn place(&self, area: Size<Points>, t: f64, value: f64) -> Option<(f32, f32)> {
        if !t.is_finite() || !value.is_finite() {
            return None;
        }
        let [start, end] = self.domain;
        let span = end - start;
        let along = if span > 0.0 { (t - start) / span } else { 1.0 };
        let height = (area.height.0 - 2.0 * LINE_WIDTH).max(0.0);
        let up = if self.top > 0.0 {
            (value / self.top).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Some((
            points(along) * area.width.0,
            LINE_WIDTH + height - points(up) * height,
        ))
    }

    /// The runs of consecutive samples of `column` in an `area`, split where a sample is missing.
    fn runs(&self, area: Size<Points>, column: &[f64]) -> Vec<Vec<(f32, f32)>> {
        let mut runs: Vec<Vec<(f32, f32)>> = vec![Vec::new()];
        for (t, value) in self.times.iter().zip(column) {
            match self.place(area, *t, *value) {
                Some(spot) => {
                    if let Some(run) = runs.last_mut() {
                        run.push(spot);
                    }
                }
                None => runs.push(Vec::new()),
            }
        }
        runs.retain(|run| !run.is_empty());
        runs
    }

    /// The drawing in an `area`: the grid in `grid` (none for a sparkline), then each series in its ink, its area
    /// tinted under it when it is filled. Nothing before there is enough to draw or before the area is laid out.
    fn paint(
        &self,
        area: Size<Points>,
        grid: Option<Hsla>,
        inks: &[(Hsla, Hsla, bool)],
    ) -> Painter {
        let mut painter = Painter::new();
        if !self.drawable || area.width.0 <= 0.0 || area.height.0 <= 0.0 {
            return painter;
        }
        // Older samples than the window are cut at its leading edge.
        painter = painter
            .save()
            .begin_path()
            .rect(Bounds {
                origin: Point::default(),
                size: area,
            })
            .clip();
        if let Some(grid) = grid {
            painter = painter.stroke_color(grid).line_width(HAIRLINE).begin_path();
            for value in [0.0, self.top / 2.0, self.top] {
                if let Some((_, y)) = self.place(area, self.domain[1], value) {
                    painter = painter.move_to(0.0, y).line_to(area.width.0, y);
                }
            }
            painter = painter.stroke();
        }
        let bottom = area.height.0 - LINE_WIDTH;
        for (column, (stroke, fill, filled)) in self.columns.iter().zip(inks) {
            let runs = self.runs(area, column);
            if *filled {
                painter = painter.fill_color(*fill);
                for run in runs.iter().filter(|run| run.len() >= 2) {
                    painter = trace(painter.begin_path(), run, Some(bottom))
                        .close_path()
                        .fill();
                }
            }
            painter = painter
                .stroke_color(*stroke)
                .line_width(LINE_WIDTH)
                .line_join(LineJoin::Round)
                .line_cap(LineCap::Round);
            for run in runs.iter().filter(|run| run.len() >= 2) {
                painter = trace(painter.begin_path(), run, None).stroke();
            }
        }
        painter.restore()
    }
}

/// `painter` with a path through `run`; down to `base` and back at both ends for an area.
fn trace(painter: Painter, run: &[(f32, f32)], base: Option<f32>) -> Painter {
    let (Some(&(first_x, first_y)), Some(&(last_x, _))) = (run.first(), run.last()) else {
        return painter;
    };
    let painter = match base {
        Some(base) => painter.move_to(first_x, base).line_to(first_x, first_y),
        None => painter.move_to(first_x, first_y),
    };
    let painter = run
        .iter()
        .skip(1)
        .fold(painter, |painter, &(x, y)| painter.line_to(x, y));
    match base {
        Some(base) => painter.line_to(last_x, base),
        None => painter,
    }
}

/// A fraction of a length as the `f32` the canvas takes.
fn points(fraction: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a fraction in 0..=1 (or a little past the edges) fits an f32"
    )]
    let fraction = fraction as f32;
    fraction
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

/// A y-axis label: Meta size, Mono, `muted-foreground`, one line.
fn axis_label(text: impl IntoReactive<SharedString>) -> Text {
    machine_text(text)
        .text_xs()
        .number_of_lines(1)
        .text_color(move || theme().muted_foreground)
}

/// A 64 x 20 pt trend line of `samples()` (evenly spaced, oldest first) in `ink` for a table cell. With fewer than
/// two samples the box stays empty, so the column keeps its rhythm; the numbers beside it are the readout.
///
/// ```ignore
/// sparkline(move || history.rx_rates(), SeriesColor::Received)
/// ```
pub fn sparkline(samples: impl Fn() -> Vec<f64> + 'static, ink: SeriesColor) -> View {
    let frame = computed(move || {
        let values = samples();
        let times = (0..values.len()).map(index_time).collect();
        Frame::of(times, vec![values], ChartUnit::Count, None)
    });
    let size = signal(Size::<Points>::default());
    view()
        .w(Points(SPARK_WIDTH))
        .h(Points(SPARK_HEIGHT))
        .flex_shrink_0()
        .accessible(false)
        .on_layout(move |bounds: &Bounds<Points>| size.set(bounds.size))
        .child(
            canvas(move || {
                let stroke = color(ink.color());
                frame.with(|frame| frame.paint(size.get(), None, &[(stroke, stroke, false)]))
            })
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .left_0(),
        )
}

/// The place of sample `index` along a sparkline's evenly spaced axis.
fn index_time(index: usize) -> f64 {
    u32::try_from(index).map_or(f64::MAX, f64::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_axes_top_out_at_round_binary_units() {
        // 1.95 KiB/s of traffic: the axis tops at 2 KiB/s, its middle line at 1 KiB/s.
        let top = axis_top(ChartUnit::BytesPerSecond, 2000.0);
        assert_eq!(top, 2048.0);
        assert_eq!(ChartUnit::BytesPerSecond.format(top), "2 KiB/s");
        assert_eq!(ChartUnit::BytesPerSecond.format(top / 2.0), "1 KiB/s");
        assert_eq!(
            axis_top(ChartUnit::BytesPerSecond, 3.5 * 1024.0 * 1024.0),
            5.0 * 1024.0 * 1024.0
        );
        // Under a kibibyte the bytes themselves are tidied.
        assert_eq!(axis_top(ChartUnit::BytesPerSecond, 130.0), 200.0);
        assert_eq!(axis_top(ChartUnit::Milliseconds, 1300.0), 2000.0);
    }

    #[test]
    fn the_time_axis_ends_at_the_newest_sample() {
        let frame = Frame::of(
            vec![10_000.0, 11_000.0, 12_000.0],
            vec![vec![1.0, 2.0, 3.0]],
            ChartUnit::Count,
            Some(180_000.0),
        );
        assert!(frame.drawable);
        assert_eq!(frame.domain, [12_000.0 - 180_000.0, 12_000.0]);
        let one = Frame::of(
            vec![10_000.0],
            vec![vec![1.0]],
            ChartUnit::Count,
            Some(180_000.0),
        );
        assert!(!one.drawable);
    }

    #[test]
    fn samples_sit_at_their_moment_and_a_gap_splits_the_line() {
        let frame = Frame::of(
            vec![0.0, 50.0, 100.0, 150.0, 200.0],
            vec![vec![0.0, 10.0, f64::NAN, 10.0, 5.0]],
            ChartUnit::Count,
            Some(200.0),
        );
        let area = Size::new(px(200.0), px(100.0));
        let runs = frame.runs(area, &frame.columns[0]);
        assert_eq!(runs.len(), 2);
        // The newest sample is at the right edge; a sample at the window's start at the left one.
        assert_eq!(runs[1].last().map(|spot| spot.0), Some(200.0));
        assert_eq!(runs[0].first().map(|spot| spot.0), Some(0.0));
        // Zero sits on the bottom edge, inside room for the stroke.
        assert_eq!(runs[0].first().map(|spot| spot.1), Some(100.0 - LINE_WIDTH));
    }
}
