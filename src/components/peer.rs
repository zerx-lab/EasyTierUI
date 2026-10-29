//! The Peer Row (DESIGN.md signature): the hostname in Body; virtual IP, latency, loss and traffic in Mono, right-aligned
//! in columns on the desktops and stacked under the hostname on phones; a trailing status chip. 44 pt rows separated by
//! hairlines, `accent` on hover and selection.

use std::rc::Rc;

use nsh_base::theme;
use nsh_core::prelude::*;

use super::text::{app_color, body_text, machine_text, meta_text};

/// One machine-data column of a [`PeerRow`].
pub struct PeerColumn {
    text: Reactive<SharedString>,
    width: Points,
    warn: Option<Rc<dyn Fn() -> bool>>,
}

impl PeerColumn {
    /// A column showing `text` (machine data), `width` wide on the desktops.
    ///
    /// ```ignore
    /// PeerColumn::new(move || peer.latency_text(), Points(72.0))
    /// ```
    pub fn new(text: impl IntoReactive<SharedString>, width: Points) -> Self {
        Self {
            text: text.into_reactive(),
            width,
            warn: None,
        }
    }

    /// Draws the number in Relay Amber while `warn()` holds (latency past a threshold). The number stays readable:
    /// the color only repeats what the figure says.
    #[must_use]
    pub fn warn(mut self, warn: impl Fn() -> bool + 'static) -> Self {
        self.warn = Some(Rc::new(warn));
        self
    }
}

/// A peer row under construction: add columns, a trailing chip and a handler, then place it as a child.
pub struct PeerRow {
    primary: Reactive<SharedString>,
    columns: Vec<PeerColumn>,
    trailing: Option<AnyElement>,
    separated: bool,
    selected: Option<Rc<dyn Fn() -> bool>>,
    on_press: Option<Rc<dyn Fn()>>,
    test_id: Option<SharedString>,
}

/// A row for the peer called `primary`.
///
/// ```ignore
/// peer_row(move || peer.hostname.clone())
///     .column(PeerColumn::new(move || peer.ip.clone(), Points(128.0)))
///     .column(PeerColumn::new(move || peer.latency.clone(), Points(72.0)).warn(move || peer.slow))
///     .trailing(status_chip(move || peer.lamp, store.tr(|s| s.route_p2p)))
///     .separated(index > 0)
///     .on_press(move || store.select_peer(id))
///     .test_id(format!("peer-{id}"))
/// ```
pub fn peer_row(primary: impl IntoReactive<SharedString>) -> PeerRow {
    PeerRow {
        primary: primary.into_reactive(),
        columns: Vec::new(),
        trailing: None,
        separated: false,
        selected: None,
        on_press: None,
        test_id: None,
    }
}

/// The column captions above a list of [`PeerRow`]s, aligned with its columns: `(caption, width)` per column. Empty
/// on phones, where the columns are stacked and each figure speaks for itself.
///
/// ```ignore
/// peer_header(vec![(store.tr(|s| s.col_ip).into_reactive(), Points(128.0))])
/// ```
pub fn peer_header(columns: Vec<(Reactive<SharedString>, Points)>) -> View {
    if !is_desktop() {
        return view();
    }
    columns.into_iter().fold(
        view()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_1()
            .child(view().flex_1()),
        |row, (caption, width)| {
            row.child(
                meta_text(caption)
                    .w(width)
                    .flex_shrink_0()
                    .text_right()
                    .font_weight(FontWeight::MEDIUM),
            )
        },
    )
}

impl PeerRow {
    /// Adds a machine-data column.
    #[must_use]
    pub fn column(mut self, column: PeerColumn) -> Self {
        self.columns.push(column);
        self
    }

    /// The element after the columns: a status chip.
    #[must_use]
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_element());
        self
    }

    /// Draws the hairline above the row (every row after the first).
    #[must_use]
    pub fn separated(mut self, separated: bool) -> Self {
        self.separated = separated;
        self
    }

    /// Fills the row with the selection tint while `selected()` holds.
    #[must_use]
    pub fn selected(mut self, selected: impl Fn() -> bool + 'static) -> Self {
        self.selected = Some(Rc::new(selected));
        self
    }

    /// Makes the row a button that runs `on_press`.
    #[must_use]
    pub fn on_press(mut self, on_press: impl Fn() + 'static) -> Self {
        self.on_press = Some(Rc::new(on_press));
        self
    }

    /// The hook tests find the row by (`id:<test_id>`).
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view.
    pub fn build(self) -> View {
        let Self {
            primary,
            columns,
            trailing,
            separated,
            selected,
            on_press,
            test_id,
        } = self;
        let selected_for_fill = selected.clone();
        let fill = move || {
            if selected_for_fill.as_ref().is_some_and(|f| f()) {
                theme().accent
            } else {
                AdaptiveColor::TRANSPARENT
            }
        };
        let interactive = on_press.is_some();
        let row = view()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .min_h_11()
            .bg(fill)
            .when(separated, |row| {
                row.border_t_1().border_color(move || theme().border)
            })
            .when(interactive, |row| {
                row.hover(|s| s.bg(move || theme().accent))
                    .active(|s| s.bg(move || theme().accent))
                    .role(AccessibilityRole::Button)
            })
            .accessibility_state(move || AccessibilityState {
                selected: selected.as_ref().is_some_and(|f| f()),
                ..AccessibilityState::default()
            })
            .when_some(on_press, |row, on_press| row.on_press(move || on_press()))
            .when_some(test_id, |row, id| row.test_id(&id));
        if is_desktop() {
            let row = row.child(body_text(primary).flex_1());
            let row = columns.into_iter().fold(row, |row, column| {
                row.child(
                    column_text(&column)
                        .w(column.width)
                        .flex_shrink_0()
                        .text_right(),
                )
            });
            row.when_some(trailing, |row, chip| row.child(chip))
        } else {
            // Phones: the hostname, then the figures in a wrapping line under it, then the chip at the end.
            let figures = columns.into_iter().fold(
                view().flex_row().flex_wrap().gap_x_3(),
                |figures, column| figures.child(column_text(&column)),
            );
            row.child(
                view()
                    .flex_1()
                    .gap_1()
                    .child(body_text(primary))
                    .child(figures),
            )
            .when_some(trailing, |row, chip| row.child(chip))
        }
    }
}

/// A column's figure in Mono, amber while it warns.
fn column_text(column: &PeerColumn) -> Text {
    let warn = column.warn.clone();
    machine_text(column.text.clone()).text_color(move || {
        if warn.as_ref().is_some_and(|f| f()) {
            app_color("warning")
        } else {
            theme().muted_foreground
        }
    })
}

impl IntoElement for PeerRow {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}
