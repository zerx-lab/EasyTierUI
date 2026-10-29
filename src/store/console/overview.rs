//! What the web build's control-plane pages read from the serving easytier-web: the account's device count
//! (`/summary`) and every machine session (`/sessions`). The machine list itself is the console's own feed.

use nsh_core::time::unix_ms;

use super::{ConsoleStore, Feed};
use crate::core::{
    remote::{RemoteClient, ServerSession, ServerSummary},
    target::TargetId,
};

/// The two reads of one server's overview, each keeping its last good value.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Overview {
    /// `GET /summary`.
    pub summary: Feed<ServerSummary>,
    /// `GET /sessions`.
    pub sessions: Feed<Vec<ServerSession>>,
}

impl ConsoleStore {
    /// `id`'s overview (tracked).
    pub fn overview(&self, id: TargetId) -> Overview {
        self.overview
            .with(|all| all.get(&id).cloned())
            .unwrap_or_default()
    }

    /// Reads `id`'s summary, sessions and machines again.
    pub fn refresh_overview(&self, id: TargetId) {
        self.refresh_machines(id);
        let Some(client) = self.client(id) else {
            return;
        };
        let loading = self.overview.with_untracked(|all| {
            all.get(&id)
                .is_some_and(|o| o.summary.loading || o.sessions.loading)
        });
        if loading {
            return;
        }
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            this.load_overview(id, epoch, &client).await;
        });
    }

    /// Reads the summary, then the sessions; a stale answer is dropped.
    async fn load_overview(&self, id: TargetId, epoch: u64, client: &RemoteClient) {
        self.overview.update(|all| {
            let entry = all.entry(id).or_default();
            entry.summary.begin();
            entry.sessions.begin();
        });
        let summary = client.summary().await;
        if !self.current_epoch(id, epoch) {
            return;
        }
        match summary {
            Ok(value) => self.overview.update(|all| {
                all.entry(id).or_default().summary.succeed(value, unix_ms());
            }),
            Err(error) => {
                self.overview
                    .update(|all| all.entry(id).or_default().summary.fail(error.clone()));
                self.react_to(id, &error);
            }
        }
        let sessions = client.sessions().await;
        if !self.current_epoch(id, epoch) {
            return;
        }
        match sessions {
            Ok(value) => self.overview.update(|all| {
                all.entry(id)
                    .or_default()
                    .sessions
                    .succeed(value, unix_ms());
            }),
            Err(error) => {
                self.overview
                    .update(|all| all.entry(id).or_default().sessions.fail(error.clone()));
                self.react_to(id, &error);
            }
        }
    }
}
