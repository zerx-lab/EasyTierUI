//! Console: an easytier-web server's machines, networks and credentials.

use nsh_core::prelude::*;

use crate::{components::empty_state, core::target::TargetKind, store::Store};

/// A console page for the selected target: it needs a server, and the app has not connected to one yet.
pub fn console(store: Store) -> AnyElement {
    dynamic(move || {
        let on_server = store
            .current_target()
            .is_some_and(|target| matches!(target.kind, TargetKind::Server | TargetKind::Hosted));
        if on_server {
            empty_state(
                store.tr(|s| s.console_not_connected_title),
                store.tr(|s| s.console_not_connected_body),
            )
        } else {
            empty_state(
                store.tr(|s| s.console_no_server_title),
                store.tr(|s| s.console_no_server_body),
            )
        }
    })
}
