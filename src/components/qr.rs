//! A QR code drawn from a module matrix (a pairing or invite link shown on screen). The code itself is computed by the
//! caller; this only paints it.

use nsh_base::theme;
use nsh_core::{element::Painter, prelude::*};

/// The quiet zone around the modules, in modules (the QR specification asks for four).
const QUIET_ZONE: usize = 4;

/// The side the code aims for, in points; the module size is the largest whole number of points that fits.
const TARGET_SIDE: f32 = 200.0;

/// The smallest module a camera can still resolve, in points.
const MIN_MODULE: f32 = 2.0;

/// A small count as a point length; a count past `u16::MAX` is no QR code and draws as zero.
fn pt(count: usize) -> f32 {
    u16::try_from(count).map_or(0.0, f32::from)
}

/// The QR code `modules` (`size` × `size`, row by row, `true` = dark) with its quiet zone, `foreground` modules on a
/// `card` ground, about 200 pt wide. The ground is the panel color so the code sits in a card; in the dark appearance
/// that is light on dark, which phone cameras read. `label` is what screen readers say ("QR code for the invite
/// link"). A matrix that is not `size` × `size` draws only the modules it has.
///
/// ```ignore
/// qr_view(21, modules, store.tr(|s| s.qr_label))
/// ```
pub fn qr_view(size: usize, modules: Vec<bool>, label: impl IntoReactive<SharedString>) -> View {
    let total = size.saturating_add(QUIET_ZONE.saturating_mul(2));
    let module = (TARGET_SIDE / pt(total).max(1.0)).floor().max(MIN_MODULE);
    let side = module * pt(total);
    let dark = move |x: usize, y: usize| {
        y.checked_mul(size)
            .and_then(|row| row.checked_add(x))
            .and_then(|index| modules.get(index))
            .copied()
            .unwrap_or(false)
    };
    let drawing = canvas(move || {
        let mut painter = Painter::new()
            .fill_color(color(theme().card))
            .fill_rect(0.0, 0.0, side, side)
            .fill_color(color(theme().foreground));
        for y in 0..size {
            let mut x = 0;
            while x < size {
                if !dark(x, y) {
                    x = x.saturating_add(1);
                    continue;
                }
                // Neighbouring dark modules of a row are one rectangle: far fewer drawing operations.
                let start = x;
                while x < size && dark(x, y) {
                    x = x.saturating_add(1);
                }
                painter = painter.fill_rect(
                    pt(start.saturating_add(QUIET_ZONE)) * module,
                    pt(y.saturating_add(QUIET_ZONE)) * module,
                    pt(x.saturating_sub(start)) * module,
                    module,
                );
            }
        }
        painter
    });
    view()
        .role(AccessibilityRole::Image)
        .label(label)
        .w(Points(side))
        .h(Points(side))
        .flex_shrink_0()
        .rounded(move || theme().radius)
        .overflow_hidden()
        .child(drawing.size_full())
}
