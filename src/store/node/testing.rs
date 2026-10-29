//! A seam for headless tests, compiled into debug builds only (like the in-memory secret store): a test registers a
//! [`NodeBackend`] under a token and seeds the app's storage with `test.node-backend = <token>`; the node store then
//! uses that backend instead of the EasyTier sidecar. Nothing here is reachable in a release build, and a storage
//! value that names no registered token changes nothing.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

use nsh_core::storage::Storage;

use crate::core::node::NodeBackend;

/// The storage key a test seeds with the token of its backend.
pub const STORAGE_KEY: &str = "test.node-backend";

/// The backends tests registered, by token.
static BACKENDS: Mutex<BTreeMap<String, Arc<dyn NodeBackend>>> = Mutex::new(BTreeMap::new());

/// Registers `backend` under `token`; seed `test.node-backend = token` in the app's storage before it starts.
pub fn register(token: &str, backend: Arc<dyn NodeBackend>) {
    BACKENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(token.to_owned(), backend);
}

/// The backend the storage names, if a test registered one.
pub(super) fn from_storage() -> Option<Arc<dyn NodeBackend>> {
    let token = Storage::get_item(STORAGE_KEY)?;
    BACKENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&token)
        .cloned()
}
