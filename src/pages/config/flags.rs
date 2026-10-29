//! The core's behavior switches as a list of switch rows, grouped by what they are about.

use nsh_core::prelude::*;

use crate::{
    components::{section_label, switch_row},
    core::settings::{Flag, FlagGroup, NodeFlags},
    store::Store,
};

/// Every switch of [`NodeFlags`], grouped by [`FlagGroup`] under a quiet sub-label, each a switch row with its label
/// and what it does; a switch that widens exposure says so right under itself while it is on.
///
/// `get` reads the flags (a signal, so the rows follow it), `set` receives the switch and its next value. `locked`
/// shows the rows read-only (a network managed by a control plane). Test ids are `{test_prefix}-flag-{flag.id()}`.
pub(in crate::pages) fn flag_switches(
    store: Store,
    get: impl Fn() -> NodeFlags + Copy + 'static,
    set: impl Fn(Flag, bool) + Copy + 'static,
    locked: bool,
    test_prefix: &'static str,
) -> View {
    let mut list = view().gap_6();
    for group in FlagGroup::ALL {
        let mut block = view().gap_3().child(
            section_label(move || store.strings().config.flag_group(group))
                .role(AccessibilityRole::Header),
        );
        for flag in Flag::of(group) {
            let mut row = switch_row(
                move || store.strings().config.flag_label(flag),
                move || flag.get(&get()),
                move |on| {
                    if !locked {
                        set(flag, on);
                    }
                },
            )
            .description(move || store.strings().config.flag_help(flag))
            .disabled(move || locked)
            .test_id(format!("{test_prefix}-flag-{}", flag.id()));
            if flag.widens_exposure() {
                row = row.exposure(store.tr(|s| s.config.flag_forward_by_system_exposure));
            }
            block = block.child(row);
        }
        list = list.child(block);
    }
    list
}
