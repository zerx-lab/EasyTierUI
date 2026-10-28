//! The app's own desktop title bar (the system's is hidden, `TitleBarStyle::Hidden`): bars of
//! [`bar_height`] marked as drag regions, clear of the native window controls the platform keeps (macOS's traffic
//! lights, Windows' caption buttons, GTK's window buttons) at the edges `title_bar()` reports.

use nsh_core::{element::desktop::title_bar, prelude::*};

/// The height asked of the platform for the custom title bar: up to 44 pt keeps macOS's compact chrome, and
/// Windows' caption buttons fill it. A platform dimension, not a spacing step.
pub const TITLE_BAR_HEIGHT: Points = Points(44.0);

/// The padding beside the native window controls (spacing step 3).
pub const CONTROLS_GAP: f32 = 12.0;

/// The title bar's height (tracked): what the platform reports for the hidden title bar, or
/// [`TITLE_BAR_HEIGHT`] where it reports none (the system's title bar, full screen, the web), so headers line up
/// across layouts.
pub fn bar_height() -> Points {
    let top = title_bar().top;
    if top > Points(0.0) {
        top
    } else {
        TITLE_BAR_HEIGHT
    }
}

/// Which window edges a part of the title bar touches: they keep room for the window controls there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarEdges {
    /// It starts at the window's leading edge.
    pub leading: bool,
    /// It ends at the window's trailing edge.
    pub trailing: bool,
}

/// A row of the title bar: it moves the window when pressed (its buttons stay clickable), is [`bar_height`] tall
/// at least, and keeps clear of the native controls at the window `edges` it touches.
pub fn drag_region(edges: BarEdges) -> View {
    view()
        .window_drag(true)
        .flex_row()
        .items_center()
        .min_h(bar_height)
        .pl(move || {
            let controls = if edges.leading {
                title_bar().left.0
            } else {
                0.0
            };
            Points(controls + CONTROLS_GAP)
        })
        .pr(move || {
            let controls = if edges.trailing {
                title_bar().right.0
            } else {
                0.0
            };
            Points(controls + CONTROLS_GAP)
        })
}
