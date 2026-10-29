//! The web build's Node pages manage one machine of the serving server. Which one is remembered per server across
//! restarts in `Storage` as `web.node.machine.<server number>` (a machine id: no secret).

use nsh_core::storage::Storage;
use uuid::Uuid;

use super::ConsoleStore;
use crate::core::target::TargetId;

/// The storage key of the machine the Node pages manage on server `id`.
fn key_of(id: TargetId) -> String {
    format!("web.node.machine.{}", id.number())
}

impl ConsoleStore {
    /// The machine the Node pages manage on `id` (tracked): the one picked this run, else the one kept.
    pub fn node_machine(&self, id: TargetId) -> Option<Uuid> {
        if let Some(picked) = self.node_picks.with(|all| all.get(&id).copied()) {
            return Some(picked);
        }
        Storage::get_item(&key_of(id)).and_then(|kept| Uuid::parse_str(&kept).ok())
    }

    /// Makes `machine` the one the Node pages manage on `id`, and keeps it, then reads its networks.
    pub fn pick_node_machine(&self, id: TargetId, machine: Uuid) {
        self.node_picks.update(|all| {
            all.insert(id, machine);
        });
        if let Err(full) = Storage::set_item(key_of(id), machine.to_string()) {
            log::warn!(
                "the managed machine was not kept: storage full at {} bytes",
                full.bytes
            );
        }
        self.peers.set(None);
        self.refresh_networks(id, machine);
    }
}
