//! The one looping motion of the app: a slow opacity pulse for "working" marks (DESIGN.md "Status Lamp"). It runs
//! natively on the animation graph, so a pulsing lamp costs no engine work per frame, and it stops under Reduce Motion.

use nsh_core::{
    element::{AnimatedValue, Easing, Keyframe, KeyframeTiming},
    prelude::*,
};

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
