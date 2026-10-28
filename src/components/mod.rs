//! The app's styled components: nsh-base behavior with DESIGN.md's look, colors from `theme.json` through
//! `theme()`. They know nothing of the store or EasyTier; callers pass text and handlers.

mod button;
mod choice;
mod empty_state;
mod lamp;
mod nav_item;
mod notice;
mod sidebar_toggle;
mod text;
mod title_bar;

pub use button::{ButtonKind, action_button};
pub use choice::choice_row;
pub use empty_state::empty_state;
pub use lamp::{Lamp, status_lamp};
pub use nav_item::nav_item;
pub use notice::capability_notice;
pub use sidebar_toggle::sidebar_toggle;
pub use text::{app_color, control_radius, machine_text, section_label};
pub use title_bar::{BarEdges, CONTROLS_GAP, TITLE_BAR_HEIGHT, bar_height, drag_region};
