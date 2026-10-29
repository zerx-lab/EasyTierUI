//! The icon set (DESIGN.md "Icons"): one authored family, drawn natively as strokes on a canvas from SVG path data,
//! never a glyph or an emoji. Every icon is drawn on a 24-unit grid with round caps and joins and a 1.5 pt stroke at
//! any size, tinted by a theme token read in the canvas's binding, so a change of appearance redraws it.

use nsh_base::theme;
use nsh_core::{
    canvas::{LineCap, LineJoin},
    prelude::*,
};

/// The grid the paths are authored on.
const GRID: f32 = 24.0;

/// The stroke width in points, whatever the icon's size (DESIGN.md "Icons").
const STROKE: f32 = 1.5;

/// The default size of an icon in a row or a button.
pub const ICON_SIZE: f32 = 16.0;

/// The size of an icon in a dense table row.
pub const ICON_SIZE_DENSE: f32 = 14.0;

/// The size of the icon of an empty state.
pub const ICON_SIZE_EMPTY: f32 = 32.0;

/// An icon of the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// Three linked nodes: a network.
    Network,
    /// Two people: peers.
    Peers,
    /// Two stacked units: a machine or server.
    Server,
    /// A key: a credential.
    Key,
    /// A pulse line: activity and status.
    Activity,
    /// A shield: hardening.
    Shield,
    /// A rocket: deployment.
    Rocket,
    /// Two sliders: settings.
    Settings,
    /// A plus: create, add.
    Plus,
    /// An arrow into a tray: import.
    Import,
    /// An arrow out of a tray: export.
    Export,
    /// An arrow down to a line: download.
    Download,
    /// Two chain links: a link, join.
    Link,
    /// A triangle: start.
    Play,
    /// A square: stop.
    Stop,
    /// A counter-clockwise arrow: restart.
    Restart,
    /// A clockwise arrow: refresh.
    Refresh,
    /// Three dots: more actions.
    More,
    /// Two sheets: copy.
    Copy,
    /// A tick: done, selected.
    Check,
    /// A cross: close, clear.
    Close,
    /// A magnifier: search.
    Search,
    /// A chevron pointing down: opens a list.
    ChevronDown,
    /// A chevron pointing right: opens a detail.
    ChevronRight,
    /// A circled i: information.
    Info,
    /// A triangle with a mark: a warning.
    AlertTriangle,
    /// A circle with a mark: an error.
    AlertCircle,
    /// An arrow leaving a frame: opens elsewhere.
    External,
    /// A prompt: logs and terminal.
    Terminal,
    /// A QR code.
    Qr,
    /// An eye: show.
    Eye,
    /// A struck-through eye: hide.
    EyeOff,
    /// A bin: delete.
    Trash,
    /// A pencil: edit.
    Edit,
    /// A globe: a public address.
    Globe,
    /// A padlock: encryption, privacy.
    Lock,
    /// An arrow down: received traffic.
    ArrowDown,
    /// An arrow up: sent traffic.
    ArrowUp,
    /// A funnel: filter.
    Filter,
}

impl Icon {
    /// The strokes of the icon: SVG path data on a 24-unit grid.
    fn paths(self) -> &'static [&'static str] {
        match self {
            Self::Network => &[
                "M14.5 5a2.5 2.5 0 1 0-5 0a2.5 2.5 0 1 0 5 0z",
                "M7.5 18a2.5 2.5 0 1 0-5 0a2.5 2.5 0 1 0 5 0z",
                "M21.5 18a2.5 2.5 0 1 0-5 0a2.5 2.5 0 1 0 5 0z",
                "M10.7 7.5 6.3 15.5",
                "M13.3 7.5l4.4 8",
                "M7.5 18h9",
            ],
            Self::Peers => &[
                "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2",
                "M13 7a4 4 0 1 0-8 0a4 4 0 1 0 8 0z",
                "M22 21v-2a4 4 0 0 0-3-3.87",
                "M16 3.13a4 4 0 0 1 0 7.75",
            ],
            Self::Server => &[
                "M5 3h14a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
                "M5 13h14a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2z",
                "M7 7h.01",
                "M7 17h.01",
            ],
            Self::Key => &[
                "M13 15.5a5.5 5.5 0 1 0-11 0a5.5 5.5 0 1 0 11 0z",
                "M11.4 11.6 21 2",
                "M15.5 7.5l3 3L22 7l-3-3",
            ],
            Self::Activity => &["M22 12h-4l-3 9L9 3l-3 9H2"],
            Self::Shield => &["M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"],
            Self::Rocket => &[
                "M12 2c3 2 5 5.5 5 9.5 0 2-.5 3.5-1 4.5H8c-.5-1-1-2.5-1-4.5C7 7.5 9 4 12 2z",
                "M13.5 9.5a1.5 1.5 0 1 0-3 0a1.5 1.5 0 1 0 3 0z",
                "M7 13.5l-3 3 1 3.5 3-2",
                "M17 13.5l3 3-1 3.5-3-2",
                "M12 17v4",
            ],
            Self::Settings => &[
                "M4 7h9",
                "M17 7h3",
                "M4 17h3",
                "M11 17h9",
                "M17 7a2 2 0 1 0-4 0a2 2 0 1 0 4 0z",
                "M11 17a2 2 0 1 0-4 0a2 2 0 1 0 4 0z",
            ],
            Self::Plus => &["M12 5v14", "M5 12h14"],
            Self::Import => &[
                "M12 3v11",
                "M8 10l4 4 4-4",
                "M5 15v3a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-3",
            ],
            Self::Export => &[
                "M12 15V4",
                "M8 8l4-4 4 4",
                "M5 15v3a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-3",
            ],
            Self::Download => &["M12 4v11", "M7 11l5 5 5-5", "M5 20h14"],
            Self::Link => &[
                "M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71",
                "M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71",
            ],
            Self::Play => &["M7 4.5v15l12-7.5z"],
            Self::Stop => {
                &["M7 5h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"]
            }
            Self::Restart => &[
                "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8",
                "M3 3v5h5",
            ],
            Self::Refresh => &[
                "M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8",
                "M21 3v5h-5",
            ],
            Self::More => &[
                "M6 12a1 1 0 1 0-2 0a1 1 0 1 0 2 0z",
                "M13 12a1 1 0 1 0-2 0a1 1 0 1 0 2 0z",
                "M20 12a1 1 0 1 0-2 0a1 1 0 1 0 2 0z",
            ],
            Self::Copy => &[
                "M10 8h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H10a2 2 0 0 1-2-2V10a2 2 0 0 1 2-2z",
                "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2",
            ],
            Self::Check => &["M20 6 9 17l-5-5"],
            Self::Close => &["M18 6 6 18", "M6 6l12 12"],
            Self::Search => &["M18 11a7 7 0 1 0-14 0a7 7 0 1 0 14 0z", "M21 21l-4.3-4.3"],
            Self::ChevronDown => &["M6 9l6 6 6-6"],
            Self::ChevronRight => &["M9 6l6 6-6 6"],
            Self::Info => &[
                "M21 12a9 9 0 1 0-18 0a9 9 0 1 0 18 0z",
                "M12 16v-5",
                "M12 8h.01",
            ],
            Self::AlertTriangle => &[
                "M10.3 3.9L1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
                "M12 9v4",
                "M12 17h.01",
            ],
            Self::AlertCircle => &[
                "M21 12a9 9 0 1 0-18 0a9 9 0 1 0 18 0z",
                "M12 8v4",
                "M12 16h.01",
            ],
            Self::External => &[
                "M15 3h6v6",
                "M10 14 21 3",
                "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6",
            ],
            Self::Terminal => &["M4 17l6-6-6-6", "M12 19h8"],
            Self::Qr => &[
                "M3 3h7v7H3z",
                "M14 3h7v7h-7z",
                "M3 14h7v7H3z",
                "M14 14h3v3h-3z",
                "M21 14v3",
                "M17 21h4",
            ],
            Self::Eye => &[
                "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z",
                "M15 12a3 3 0 1 0-6 0a3 3 0 1 0 6 0z",
            ],
            Self::EyeOff => &[
                "M10.6 5.1A10.8 10.8 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3.2 4.2",
                "M6.6 6.7A17 17 0 0 0 2 12s3.5 7 10 7a10 10 0 0 0 4.4-1",
                "M9.9 9.9a3 3 0 0 0 4.2 4.2",
                "M3 3l18 18",
            ],
            Self::Trash => &[
                "M3 6h18",
                "M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2",
                "M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6",
                "M10 11v6",
                "M14 11v6",
            ],
            Self::Edit => &["M17 3a2.83 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5z"],
            Self::Globe => &[
                "M21 12a9 9 0 1 0-18 0a9 9 0 1 0 18 0z",
                "M3 12h18",
                "M12 3c2.5 2.5 3.8 5.5 3.8 9S14.5 18.5 12 21c-2.5-2.5-3.8-5.5-3.8-9S9.5 5.5 12 3z",
            ],
            Self::Lock => &[
                "M6 11h12a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2z",
                "M8 11V7a4 4 0 0 1 8 0v4",
            ],
            Self::ArrowDown => &["M12 5v14", "M6 13l6 6 6-6"],
            Self::ArrowUp => &["M12 19V5", "M6 11l6-6 6 6"],
            Self::Filter => &["M3 5h18l-7 8.5V20l-4-2v-4.5z"],
        }
    }
}

/// An icon of `size` points in `muted-foreground`, the color of icons at rest. It is decoration: whatever carries it
/// (a button, a row) has the accessible name.
///
/// ```ignore
/// icon(Icon::Search, ICON_SIZE)
/// ```
pub fn icon(icon: Icon, size: f32) -> CanvasView {
    icon_colored(icon, size, move || theme().muted_foreground)
}

/// An icon tinted by `tint()`, read in the canvas's binding (a token such as `theme().foreground`, or a lamp hue
/// for state), so it follows the appearance and whatever signals `tint` reads.
///
/// ```ignore
/// icon_colored(Icon::AlertTriangle, ICON_SIZE, move || app_color("warning"))
/// ```
pub fn icon_colored(
    icon: Icon,
    size: f32,
    tint: impl Fn() -> AdaptiveColor + 'static,
) -> CanvasView {
    let size = size.max(1.0);
    let scale = size / GRID;
    canvas(move || {
        let ink = color(tint());
        icon.paths().iter().fold(
            Painter::new()
                .stroke_color(ink)
                .line_width(STROKE / scale)
                .line_cap(LineCap::Round)
                .line_join(LineJoin::Round)
                .scale(scale, scale),
            |painter, data| painter.stroke_svg(data),
        )
    })
    .w(Points(size))
    .h(Points(size))
    .flex_shrink_0()
    .accessible(false)
}
