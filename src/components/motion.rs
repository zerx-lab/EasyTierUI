//! The app's own motion, all on NSH's native animation graph (no engine work per frame) and all still under Reduce
//! Motion: a slow opacity pulse for "working" marks (DESIGN.md "Status Lamp") and the entrance of a desktop dialog's
//! panel (DESIGN.md "Dialogs and Sheets").

use nsh_core::{
    element::{AnimatedValue, Easing, Keyframe, KeyframeTiming},
    prelude::*,
};

/// An overlay's entrance, in milliseconds (DESIGN.md: 200 ms overlay).
const ENTRANCE_MS: f32 = 200.0;

/// Ease-out quart: most of the way at once, then a long soft landing with no overshoot.
const ENTRANCE_EASING: Easing = Easing::Bezier(0.25, 1.0, 0.5, 1.0);

/// One full pulse (bright, dim, bright), in milliseconds: slow enough to read as "waiting", not as an alert.
const PULSE_MS: f32 = 1600.0;

/// How dim the pulse gets.
const DIM: f32 = 0.35;

/// An opacity that pulses while `active()` holds and the user has not asked for less motion, and rests at 1 else.
/// Bind it with `.bind_opacity(&value)`; it follows `active` and `reduce_motion()` as they change.
pub(super) fn pulse(active: impl Fn() -> bool + 'static) -> AnimatedValue {
    let value = animated_value(1.0);
    effect(move || {
        if active() && !reduce_motion() {
            let frames = [1.0, DIM, 1.0].map(Keyframe::new);
            let timing = KeyframeTiming::new(PULSE_MS)
                .easing(Easing::EaseInOut)
                .iterations(None);
            // The motion ends with its owner or with `stop`; nobody waits for it.
            let _end = value.keyframes(frames, timing);
        } else {
            value.stop();
            value.set(1.0);
        }
    });
    value
}

/// A progress from 0 to 1 that starts when it is made, for an overlay coming in: bind what moves to an
/// interpolation of it. Under Reduce Motion it is 1 at once.
pub(super) fn entrance() -> AnimatedValue {
    if reduce_motion() {
        return animated_value(1.0);
    }
    let value = animated_value(0.0);
    // The motion ends with its owner; nobody waits for it.
    let _end = value.timing(1.0, ENTRANCE_MS, ENTRANCE_EASING);
    value
}
