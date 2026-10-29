//! The app's styled components: nsh-base behavior with DESIGN.md's look, colors from `theme.json` through
//! `theme()`. They know nothing of the store or EasyTier; callers pass text and handlers. Every public item has an
//! example in its doc comment.

mod button;
mod callout;
mod card;
mod chip;
mod choice;
mod data;
mod data_table;
mod dialog;
mod drawer;
mod empty_state;
mod field;
mod icon;
mod icon_button;
mod kv_grid;
mod lamp;
mod live_dot;
mod metric_chart;
mod more_menu;
mod motion;
mod nav_item;
mod notice;
mod peer;
mod qr;
mod search_field;
mod secret_field;
mod select;
mod sidebar_toggle;
mod summary_strip;
mod switch_row;
mod text;
mod threshold_bar;
mod title_bar;
mod toolbar;

pub use button::{ButtonKind, action_button, busy_button};
pub use callout::{Callout, CalloutKind, callout};
pub use card::{card, group, group_row};
pub use chip::{kind_chip, neutral_chip, status_chip};
pub use choice::choice_row;
pub use data::{CopyLabels, age_text, code_block, copy_value, stat, stat_captioned};
pub use data_table::{
    Column, Columns, DataTable, TableRow, column, columns, data_table, table_group_header,
    table_row,
};
pub use dialog::{DialogFrame, confirm, dialog_footer, dialog_frame};
pub use drawer::{Drawer, drawer};
pub use empty_state::{empty_panel, empty_state};
pub use field::{Field, field};
pub use icon::{ICON_SIZE, ICON_SIZE_DENSE, ICON_SIZE_EMPTY, Icon, icon, icon_colored};
pub use icon_button::{IconButton, IconTone, icon_button};
pub use kv_grid::{Fact, fact, fact_mono, kv_grid};
pub use lamp::{Lamp, status_lamp};
pub use live_dot::live_dot;
pub use metric_chart::{
    ChartSeries, ChartUnit, MetricChart, SeriesColor, chart_series, metric_chart, sparkline,
};
pub use more_menu::{MenuAction, MoreMenu, menu_action, more_menu};
pub use nav_item::nav_item;
pub use notice::capability_notice;
pub use peer::{PeerColumn, PeerRow, peer_header, peer_row};
pub use qr::qr_view;
pub use search_field::{SearchField, search_field};
pub use secret_field::{HintLevel, SecretField, SecretLabels, secret_field};
pub use select::{Select, SelectOption, segmented, segmented_inline, select};
pub use sidebar_toggle::sidebar_toggle;
pub use summary_strip::{SummaryCell, summary_cell, summary_strip};
pub use switch_row::{SwitchRow, switch_row};
pub use text::{app_color, body_text, control_radius, machine_text, meta_text, section_label};
pub use threshold_bar::{Thresholds, threshold_bar};
pub use title_bar::{BarEdges, CONTROLS_GAP, TITLE_BAR_HEIGHT, bar_height, drag_region};
pub use toolbar::{PAGE_GUTTER, toolbar, toolbar_spacer};
