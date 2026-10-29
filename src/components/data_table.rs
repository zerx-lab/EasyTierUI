//! The data table (DESIGN.md "Data table"): a panel with a header row on the well fill and 40 pt rows (44 pt on touch
//! screens) separated by hairlines. Whole rows are pressable (they open a drawer); cells are one line each, cut with
//! an ellipsis rather than wrapped; numbers and addresses are Mono and right-aligned; a missing value is a muted
//! dash. On phones and narrow windows each row stacks: the first cell is the row's title and the others follow in a
//! wrapping line, each still one line.
//!
//! Declare the [`Columns`] once, build the [`data_table`] from them and a [`table_row`] per record.

use std::rc::Rc;

use nsh_base::{theme, tooltip};
use nsh_core::prelude::*;

use super::{
    data::read,
    icon::{Icon, icon},
    text::{app_color, body_text, machine_text, meta_text, section_label},
};

/// The narrowest the flexible column gets in points.
const FLEX_MIN_WIDTH: f32 = 160.0;

/// Below this window width rows stack (the layout collapses to one column here too).
const STACK_BELOW: Points = Points(640.0);

/// The header's size in points: the well-filled row at the top of the panel.
const HEADER_MIN_HEIGHT: f32 = 36.0;

/// How a column is sized.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Sizing {
    /// A fixed width in points.
    Fixed(f32),
    /// The rest of the row (the one flexible text column).
    Flex,
}

/// A column of a [`data_table`] under construction.
#[derive(Clone)]
pub struct Column {
    header: Reactive<SharedString>,
    sizing: Sizing,
    right: bool,
    mono: bool,
    actions: bool,
    hint: Option<Reactive<SharedString>>,
    hide_below: Option<f32>,
}

/// A column captioned `header` (sentence case): 120 pt wide until [`width`](Column::width) or
/// [`flex`](Column::flex) says otherwise, left-aligned, in the UI font.
///
/// ```ignore
/// column(store.tr(|s| s.col_virtual_ip)).width(140.0).mono().right()
/// ```
pub fn column(header: impl IntoReactive<SharedString>) -> Column {
    Column {
        header: header.into_reactive(),
        sizing: Sizing::Fixed(120.0),
        right: false,
        mono: false,
        actions: false,
        hint: None,
        hide_below: None,
    }
}

impl Column {
    /// A fixed width in points (wide enough for the longest value; cells never wrap).
    #[must_use]
    pub fn width(mut self, points: f32) -> Self {
        self.sizing = Sizing::Fixed(points);
        self
    }

    /// Takes the rest of the row: use it for the one text column (a name). Cut with an ellipsis, never wrapped.
    #[must_use]
    pub fn flex(mut self) -> Self {
        self.sizing = Sizing::Flex;
        self
    }

    /// Right-aligns the header and the cells: numeric columns.
    #[must_use]
    pub fn right(mut self) -> Self {
        self.right = true;
        self
    }

    /// Machine data (addresses, versions, counts, rates): Mono with tabular figures.
    #[must_use]
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// A column of buttons (start/stop, the more menu): right-aligned, kept at the end of a stacked row.
    #[must_use]
    pub fn actions(mut self) -> Self {
        self.actions = true;
        self.right = true;
        self
    }

    /// An info icon after the header whose tooltip explains the column (thresholds, units).
    #[must_use]
    pub fn hint(mut self, hint: impl IntoReactive<SharedString>) -> Self {
        self.hint = Some(hint.into_reactive());
        self
    }

    /// Leaves the column out while the window is narrower than `width` points (decided when the table is built).
    #[must_use]
    pub fn hide_below(mut self, width: f32) -> Self {
        self.hide_below = Some(width);
        self
    }

    /// Whether the column shows at this window width.
    fn shows(&self) -> bool {
        self.hide_below
            .is_none_or(|width| untracked(window).width >= Points(width))
    }

    /// The box a cell of this column lives in.
    fn slot(&self) -> View {
        let slot = match self.sizing {
            Sizing::Fixed(width) => view().w(Points(width)).flex_shrink_0(),
            Sizing::Flex => view().flex_1().min_w(Points(FLEX_MIN_WIDTH)),
        };
        let slot = slot.flex_row().items_center().min_w_0();
        if self.right { slot.justify_end() } else { slot }
    }
}

/// The columns of a table, shared by its header and its rows.
#[derive(Clone)]
pub struct Columns(Rc<Vec<Column>>);

/// The columns `list`, in order.
///
/// ```ignore
/// let cols = columns(vec![column(name).flex(), column(ip).width(140.0).mono().right()]);
/// ```
pub fn columns(list: Vec<Column>) -> Columns {
    Columns(Rc::new(list))
}

/// Whether rows stack here: touch screens and windows narrower than the compact breakpoint.
fn stacked() -> bool {
    !is_desktop() || untracked(window).width < STACK_BELOW
}

/// When a table is empty and what it shows then.
type Emptiness = (Rc<dyn Fn() -> bool>, Rc<dyn Fn() -> AnyElement>);

/// A table under construction: add rows ([`row`](DataTable::row)) or a body that makes them
/// ([`body`](DataTable::body)), then place it as a child.
pub struct DataTable {
    columns: Columns,
    body: Vec<AnyElement>,
    empty: Option<Emptiness>,
    max_height: Option<f32>,
    test_id: Option<SharedString>,
}

/// A table with the header row of `columns`.
///
/// ```ignore
/// data_table(&cols)
///     .row(table_row(&cols).text(name).text(ip).on_press(move || store.open_peer(id)))
///     .test_id("peers-table")
/// ```
pub fn data_table(columns: &Columns) -> DataTable {
    DataTable {
        columns: columns.clone(),
        body: Vec::new(),
        empty: None,
        max_height: None,
        test_id: None,
    }
}

impl DataTable {
    /// Adds a row.
    #[must_use]
    pub fn row(mut self, row: TableRow) -> Self {
        self.body.push(row.build().into_element());
        self
    }

    /// Adds rows made elsewhere: an `each(..)` or `dynamic(..)` of [`table_row`]s for a list that changes.
    #[must_use]
    pub fn body(mut self, rows: impl IntoElement) -> Self {
        self.body.push(rows.into_element());
        self
    }

    /// Shows `content` under the header while `is_empty()` holds (an [`empty_state`](super::empty_state) with the
    /// primary action).
    #[must_use]
    pub fn empty(
        mut self,
        is_empty: impl Fn() -> bool + 'static,
        content: impl Fn() -> AnyElement + 'static,
    ) -> Self {
        self.empty = Some((Rc::new(is_empty), Rc::new(content)));
        self
    }

    /// Scrolls the rows past `points` while the header stays.
    #[must_use]
    pub fn max_height(mut self, points: f32) -> Self {
        self.max_height = Some(points);
        self
    }

    /// The hook tests find the table by (`id:<test_id>`).
    #[must_use]
    pub fn test_id(mut self, id: impl Into<SharedString>) -> Self {
        self.test_id = Some(id.into());
        self
    }

    /// The finished view.
    pub fn build(self) -> View {
        let Self {
            columns,
            body,
            empty,
            max_height,
            test_id,
        } = self;
        let header = if stacked() {
            None
        } else {
            Some(header_row(&columns))
        };
        // Every row draws its own hairline below it; the rows are pulled 1 pt past the clipped panel so the last
        // one's falls outside it.
        let rows = view().mb(Points(-1.0)).child(body);
        let rows = match max_height {
            Some(points) => view().child(scroll().max_h(Points(points)).child(rows)),
            None => rows,
        };
        let panel = view()
            .overflow_hidden()
            .rounded(move || theme().radius)
            .border_1()
            .border_color(move || theme().border)
            .bg(move || theme().card)
            .child(header)
            .child(rows)
            .child(empty.map(|(is_empty, content)| when(move || is_empty(), move || content())));
        match test_id {
            Some(id) => panel.test_id(&id),
            None => panel,
        }
    }
}

impl IntoElement for DataTable {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}

/// The header row: Label type on the well fill, a hairline below.
fn header_row(columns: &Columns) -> View {
    let cells: Vec<AnyElement> = columns
        .0
        .iter()
        .filter(|column| column.shows())
        .map(|column| {
            let label = section_label(column.header.clone()).number_of_lines(1);
            let label = if column.right {
                label.text_right()
            } else {
                label
            };
            let hint = column.hint.clone();
            let cell = column
                .slot()
                .gap_1()
                .child(label)
                .child(hint.clone().map(|_| icon(Icon::Info, 12.0)));
            match hint {
                Some(hint) => cell
                    .tooltip(
                        tooltip().child(
                            view()
                                .max_w_72()
                                .px_2()
                                .py_1()
                                .rounded(move || theme().radius)
                                .border_1()
                                .border_color(move || theme().border)
                                .bg(move || theme().popover)
                                .shadow_md()
                                .child(
                                    meta_text(move || read(&hint))
                                        .text_color(move || theme().popover_foreground),
                                ),
                        ),
                    )
                    .into_element(),
                None => cell.into_element(),
            }
        })
        .collect();
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .min_h(Points(HEADER_MIN_HEIGHT))
        .bg(move || theme().muted)
        .border_b_1()
        .border_color(move || theme().border)
        .child(cells)
}

/// A row of a [`data_table`] under construction: add one cell per column, in order.
pub struct TableRow {
    columns: Columns,
    stacked: bool,
    /// Cells (the column's index and the element), for the columns that show.
    cells: Vec<(usize, AnyElement)>,
    next: usize,
    detail: Option<AnyElement>,
    selected: Option<Rc<dyn Fn() -> bool>>,
    on_press: Option<Rc<dyn Fn()>>,
    label: Option<Reactive<SharedString>>,
    test_id: Option<SharedString>,
}

/// A row for `columns`.
///
/// ```ignore
/// table_row(&cols)
///     .text(move || peer.hostname.clone())
///     .text(move || peer.ip.clone())
///     .cell(status_chip(move || peer.lamp, store.tr(|s| s.route_p2p)))
///     .selected(move || store.selected_peer() == Some(id))
///     .on_press(move || store.open_peer(id))
///     .test_id(format!("peer-{id}"))
/// ```
pub fn table_row(columns: &Columns) -> TableRow {
    TableRow {
        columns: columns.clone(),
        stacked: stacked(),
        cells: Vec::new(),
        next: 0,
        detail: None,
        selected: None,
        on_press: None,
        label: None,
        test_id: None,
    }
}

impl TableRow {
    /// The column the next cell belongs to, when it shows; moves to the one after.
    fn take_column(&mut self) -> Option<(usize, Column)> {
        let index = self.next;
        self.next = self.next.saturating_add(1);
        self.columns
            .0
            .get(index)
            .filter(|column| column.shows())
            .map(|column| (index, column.clone()))
    }

    /// A text cell: one line cut with an ellipsis, Mono when the column is, right-aligned when it is. Empty text
    /// shows a muted dash. In a stacked row the first cell is the title and the others follow in Meta type (Mono for
    /// machine columns).
    #[must_use]
    pub fn text(mut self, content: impl IntoReactive<SharedString>) -> Self {
        let Some((index, column)) = self.take_column() else {
            return self;
        };
        let content = content.into_reactive();
        let present = {
            let content = content.clone();
            move || !read(&content).is_empty()
        };
        let shown = move || {
            let text = read(&content);
            if text.is_empty() {
                SharedString::from(DASH)
            } else {
                text
            }
        };
        let title = self.stacked && index == 0;
        let cell = if column.mono {
            machine_text(shown)
        } else if title {
            body_text(shown).font_weight(FontWeight::MEDIUM)
        } else if self.stacked {
            meta_text(shown)
        } else {
            body_text(shown)
        }
        .number_of_lines(1)
        .min_w_0()
        .text_color(move || {
            if !present() {
                theme().muted_foreground
            } else if column.mono && !title {
                theme().card_foreground
            } else {
                theme().foreground
            }
        });
        let cell = if column.right && !self.stacked {
            cell.text_right()
        } else {
            cell
        };
        self.cells.push((index, cell.into_element()));
        self
    }

    /// An element cell (a chip, a threshold bar, buttons, a sparkline) in the next column. A stacked row puts it in
    /// the facts line, or at the end for an [`actions`](Column::actions) column.
    #[must_use]
    pub fn cell(mut self, element: impl IntoElement) -> Self {
        if let Some((index, _)) = self.take_column() {
            self.cells.push((index, element.into_element()));
        }
        self
    }

    /// A full-width element under the row, outside its press target: a failure's reason and its action.
    #[must_use]
    pub fn detail(mut self, element: impl IntoElement) -> Self {
        self.detail = Some(element.into_element());
        self
    }

    /// Fills the row with the selection tint while `selected()` holds.
    #[must_use]
    pub fn selected(mut self, selected: impl Fn() -> bool + 'static) -> Self {
        self.selected = Some(Rc::new(selected));
        self
    }

    /// Makes the whole row a button that runs `on_press`.
    #[must_use]
    pub fn on_press(mut self, on_press: impl Fn() + 'static) -> Self {
        self.on_press = Some(Rc::new(on_press));
        self
    }

    /// The accessible name of the pressable row (the record's name), so screen readers do not read every cell.
    #[must_use]
    pub fn label(mut self, label: impl IntoReactive<SharedString>) -> Self {
        self.label = Some(label.into_reactive());
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
            columns,
            stacked,
            cells,
            next: _,
            detail,
            selected,
            on_press,
            label,
            test_id,
        } = self;
        let interactive = on_press.is_some();
        let selected_fill = selected.clone();
        let line = if stacked {
            stacked_line(&columns, cells)
        } else {
            wide_line(&columns, cells)
        };
        let line = line
            .bg(move || {
                if selected_fill.as_ref().is_some_and(|f| f()) {
                    app_color("primary-muted")
                } else {
                    AdaptiveColor::TRANSPARENT
                }
            })
            .accessibility_state(move || AccessibilityState {
                selected: selected.as_ref().is_some_and(|f| f()),
                ..AccessibilityState::default()
            })
            .when(interactive, |line| {
                line.role(AccessibilityRole::Button)
                    .hover(|s| s.bg(move || theme().accent))
                    .active(|s| s.bg(move || theme().accent))
            })
            .when_some(label, |line, label| line.label(label))
            .when_some(on_press, |line, on_press| line.on_press(move || on_press()))
            .when_some(test_id, |line, id| line.test_id(&id));
        view()
            .border_b_1()
            .border_color(move || theme().border)
            .child(line)
            .child(detail)
    }
}

impl IntoElement for TableRow {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}

/// The missing-value mark: a dash, muted by the cell.
const DASH: &str = "\u{2014}";

/// The row's line on the desktops: each cell in its column's slot.
fn wide_line(columns: &Columns, cells: Vec<(usize, AnyElement)>) -> View {
    let slots: Vec<AnyElement> = cells
        .into_iter()
        .filter_map(|(index, element)| {
            columns
                .0
                .get(index)
                .map(|column| column.slot().child(element).into_element())
        })
        .collect();
    let line = view()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_1()
        .child(slots);
    if is_desktop() {
        line.min_h_10()
    } else {
        line.min_h_11()
    }
}

/// The row's line when stacked: the title and a wrapping line of facts on the left, the action cells at the end.
fn stacked_line(columns: &Columns, cells: Vec<(usize, AnyElement)>) -> View {
    let mut title = None;
    let mut facts = Vec::new();
    let mut actions = Vec::new();
    for (index, element) in cells {
        let is_actions = columns.0.get(index).is_some_and(|column| column.actions);
        if is_actions {
            actions.push(element);
        } else if index == 0 && title.is_none() {
            title = Some(element);
        } else {
            facts.push(element);
        }
    }
    view()
        .flex_row()
        .items_start()
        .gap_3()
        .px_4()
        .py_3()
        .min_h_11()
        .child(
            view().flex_1().min_w_0().gap_1().child(title).child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_x_3()
                    .gap_y_1()
                    .child(facts),
            ),
        )
        .child(view().flex_row().items_center().gap_1().child(actions))
}

/// The header of a group of rows (a network that several machines run): a 36 pt row on the well fill with a hairline
/// below, to hold the group's name, its count and its status bars. Add the children; rows follow it directly.
///
/// ```ignore
/// table_group_header().child(text(name)).child(meta_text(count))
/// ```
pub fn table_group_header() -> View {
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .min_h(Points(HEADER_MIN_HEIGHT))
        .bg(move || theme().muted)
        .border_b_1()
        .border_color(move || theme().border)
}
