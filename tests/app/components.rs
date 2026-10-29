//! The data components on NSH's real engine: a small probe root mounts a table, a summary strip, a drawer and a chart
//! with plain strings (no store), and the steps press and read it as a person would.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use nsh_app_easytierui::components::{
    ChartUnit, Lamp, SeriesColor, chart_series, column, columns, data_table, drawer, metric_chart,
    neutral_chip, summary_cell, summary_strip, table_row,
};
use nsh_core::{MountItem, prelude::*};
use nsh_engine::{EngineConfig, Idiom, Os, Points, Root, Size};
use nsh_testing::Headless;

/// A row taller than this holds a wrapped line: a 40 pt row plus its hairline is the tallest a one-line row gets.
const ONE_LINE_ROW: f32 = 44.0;

/// The longest address of the probe, far wider than its 60 pt column.
const LONG_ADDRESS: &str = "10.126.126.2/24 and a tail that cannot possibly fit";

#[component]
fn Probe() -> impl IntoElement {
    let opened = signal(String::from("none"));
    let drawer_open = signal(false);
    let samples = signal(Vec::<f64>::new());
    let cols = columns(vec![
        column("Name").flex(),
        column("Address").width(60.0).mono().right(),
        column("Kind").width(80.0),
    ]);
    let table = data_table(&cols)
        .row(
            table_row(&cols)
                .text("alpha")
                .text(LONG_ADDRESS)
                .cell(neutral_chip("TUN"))
                .on_press(move || opened.set("alpha".to_owned()))
                .test_id("row-alpha"),
        )
        .row(
            table_row(&cols)
                .text("beta")
                .text("")
                .cell(neutral_chip("No TUN"))
                .on_press(move || opened.set("beta".to_owned()))
                .test_id("row-beta"),
        )
        .test_id("probe-table");
    view()
        .child(text(move || {
            SharedString::from(format!("opened:{}", opened.get()))
        }))
        .child(table)
        .child(summary_strip(vec![
            summary_cell("Peers online", "12")
                .unit("of 14")
                .bar(Lamp::Good, "9", "direct")
                .bar(Lamp::Degraded, "3", "relayed"),
            summary_cell("Median latency", "23")
                .unit("ms")
                .caption("over 12 peers")
                .test_id("summary-latency"),
        ]))
        .child(
            view()
                .test_id("open-drawer")
                .on_press(move || drawer_open.set(true))
                .child(text("Open drawer")),
        )
        .child(
            drawer(
                move || drawer_open.get(),
                "Home network",
                "Close",
                move || drawer_open.set(false),
            )
            .kind("Network")
            .status(|| Lamp::Good, "Running")
            .body(|| text("Drawer body"))
            .test_id("probe-drawer"),
        )
        .child(
            metric_chart(
                "Throughput",
                vec![
                    chart_series("Received", SeriesColor::Received, move || samples.get()).filled(),
                ],
                ChartUnit::Count,
                "Collecting samples\u{2026}",
            )
            .test_id("chart"),
        )
        .child(
            view()
                .test_id("feed")
                .on_press(move || samples.set(vec![1.0, 2.0, 3.0]))
                .child(text("Feed samples")),
        )
}

/// The probe on a 1000 x 800 pt desktop; the frames the layout gives views are collected as they arrive.
fn start() -> (Headless, Arc<Mutex<HashMap<u32, f32>>>) {
    let theme = nsh_base::Theme::from_json(include_str!("../../theme.json")).unwrap();
    let mut config = EngineConfig::new(Root::of::<Probe>().tokens(theme.into_tokens()));
    let size = Size {
        width: Points(1000.0),
        height: Points(800.0),
    };
    config.viewport = size;
    config.environment.window = size;
    config.environment.os = Os::Web;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).unwrap();
    let heights = Arc::new(Mutex::new(HashMap::new()));
    let seen = heights.clone();
    app.observe(move |_, batch| {
        for item in &batch.items {
            if let MountItem::Frame { id, frame } = item {
                seen.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(*id, frame.size.height.0);
            }
        }
    });
    app.first_render().unwrap();
    (app, heights)
}

#[test]
fn a_pressed_row_reports_itself_and_its_cells_stay_on_one_line() {
    let (mut app, heights) = start();
    app.wait_for("opened:none").unwrap();
    // The address is one text in its cell, whole, and the missing one is a dash.
    assert!(app.shows(LONG_ADDRESS));
    assert!(app.shows("\u{2014}"));

    // Pressing anywhere on the row opens it.
    app.press("id:row-beta").unwrap();
    app.wait_for("opened:beta").unwrap();
    app.press("id:row-alpha").unwrap();
    app.wait_for("opened:alpha").unwrap();

    // A cell that wrapped would stretch its row past one line: the long address in a 60 pt column is cut instead.
    let row = app
        .screen()
        .press_target("id:row-alpha")
        .unwrap()
        .expect("the row is pressable");
    let height = *heights
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&row)
        .expect("the layout framed the row");
    assert!(
        height < ONE_LINE_ROW,
        "the row is {height} pt tall: a cell wrapped"
    );
}

#[test]
fn the_summary_strip_shows_its_values_units_captions_and_counts() {
    let (mut app, _) = start();
    app.wait_for("Peers online").unwrap();
    for shown in [
        "12",
        "of 14",
        "9",
        "direct",
        "3",
        "relayed",
        "23",
        "ms",
        "over 12 peers",
    ] {
        assert!(app.shows(shown), "the strip does not show {shown:?}");
    }
    app.wait_for("id:summary-latency").unwrap();
}

#[test]
fn the_drawer_opens_with_its_parts_and_closes_with_its_close_button() {
    let (mut app, _) = start();
    app.wait_for("opened:none").unwrap();
    assert!(!app.shows("Drawer body"));

    app.press("id:open-drawer").unwrap();
    app.wait_for("Drawer body").unwrap();
    for part in ["Home network", "Network", "Running"] {
        assert!(app.shows(part), "the drawer header does not show {part:?}");
    }

    app.press("id:probe-drawer-close >> label:Close").unwrap();
    app.wait_for_gone("Drawer body", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn the_chart_says_it_is_collecting_until_it_has_samples() {
    let (mut app, _) = start();
    app.wait_for("Collecting samples\u{2026}").unwrap();
    app.wait_for("id:chart-empty").unwrap();

    app.press("id:feed").unwrap();
    app.wait_for_gone("Collecting samples\u{2026}", Duration::from_secs(2))
        .unwrap();
    // Samples 1..3 peak at 3.3, so the axis tops out at 5 and its middle reads 2.5.
    app.wait_for("2.5").unwrap();
    assert!(app.shows("Received"));
}
