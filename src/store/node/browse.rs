//! What the Networks and Peers pages show and filter by: the toolbars' search text and filters, the peer whose drawer is
//! open, and the rows and totals derived from the core's readings. UI state only: nothing here is persisted or sent to
//! a backend.

use super::{NodeStore, Status};
use crate::{
    core::state::{PeerSummary, RouteKind},
    profile::{NetworkProfile, ProfileId},
};

/// Which networks the Networks page lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkFilter {
    /// Every network.
    #[default]
    All,
    /// Networks that run or are starting.
    Running,
    /// Networks that do not run (stopped or failed).
    Stopped,
}

impl NetworkFilter {
    /// Every filter, in the order the toolbar lists them.
    pub const ALL: [NetworkFilter; 3] = [Self::All, Self::Running, Self::Stopped];

    /// The id the toolbar's segmented control goes by.
    pub const fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }

    /// The filter an id of the segmented control stands for.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|filter| filter.key() == key)
    }

    fn keeps(self, status: &Status) -> bool {
        match self {
            Self::All => true,
            Self::Running => status.is_active(),
            Self::Stopped => !status.is_active(),
        }
    }
}

/// Which peers the Peers page lists, by how traffic reaches them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RouteFilter {
    /// Every peer.
    #[default]
    All,
    /// Peers reached over a direct (P2P) link.
    Direct,
    /// Peers reached through other nodes.
    Relayed,
}

impl RouteFilter {
    /// Every filter, in the order the toolbar lists them.
    pub const ALL: [RouteFilter; 3] = [Self::All, Self::Direct, Self::Relayed];

    /// The id the toolbar's segmented control goes by.
    pub const fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Direct => "direct",
            Self::Relayed => "relayed",
        }
    }

    /// The filter an id of the segmented control stands for.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|filter| filter.key() == key)
    }

    fn keeps(self, route: RouteKind) -> bool {
        match self {
            Self::All => true,
            Self::Direct => route == RouteKind::Direct,
            Self::Relayed => matches!(route, RouteKind::Relayed { .. }),
        }
    }
}

/// A peer of a network, by the ids the core uses: what the peer drawer is open on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRef {
    /// The network the peer is in.
    pub network: ProfileId,
    /// The core's numeric peer id.
    pub peer_id: u32,
}

/// One row of the peers table: a peer and the network it is in.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerRow {
    /// The network the peer is in.
    pub network: ProfileId,
    /// The name of that network on this device.
    pub network_name: String,
    /// What the core reports of the peer.
    pub peer: PeerSummary,
}

impl PeerRow {
    /// The peer's reference.
    pub fn reference(&self) -> PeerRef {
        PeerRef {
            network: self.network.clone(),
            peer_id: self.peer.peer_id,
        }
    }

    /// The hostname, or the numeric id of a peer that reports none.
    pub fn display_name(&self) -> String {
        if self.peer.hostname.is_empty() {
            self.peer.peer_id.to_string()
        } else {
            self.peer.hostname.clone()
        }
    }
}

/// How many peers are online and how they are reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeerMix {
    /// Peers listed.
    pub online: usize,
    /// Reached over a direct link.
    pub direct: usize,
    /// Reached through other nodes.
    pub relayed: usize,
}

impl PeerMix {
    /// The mix of `peers`.
    pub fn of<'a>(peers: impl IntoIterator<Item = &'a PeerSummary>) -> Self {
        peers.into_iter().fold(Self::default(), |mut mix, peer| {
            mix.online = mix.online.saturating_add(1);
            match peer.route {
                RouteKind::Direct => mix.direct = mix.direct.saturating_add(1),
                RouteKind::Relayed { .. } => mix.relayed = mix.relayed.saturating_add(1),
                RouteKind::Unknown => {}
            }
            mix
        })
    }

    fn plus(self, other: Self) -> Self {
        Self {
            online: self.online.saturating_add(other.online),
            direct: self.direct.saturating_add(other.direct),
            relayed: self.relayed.saturating_add(other.relayed),
        }
    }
}

/// The figures of a set of peers that the Peers page's summary strip shows.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PeerStats {
    /// How many peers and how they are reached.
    pub mix: PeerMix,
    /// The median of the measured latencies, in milliseconds.
    pub median_latency_ms: Option<f64>,
    /// How many peers have a measured latency.
    pub measured: usize,
    /// The largest measured loss, as a fraction.
    pub worst_loss: Option<f64>,
}

impl PeerStats {
    /// The figures of `peers`.
    pub fn of<'a>(peers: impl IntoIterator<Item = &'a PeerSummary> + Clone) -> Self {
        let mix = PeerMix::of(peers.clone());
        let mut latencies: Vec<f64> = peers
            .clone()
            .into_iter()
            .filter_map(|peer| peer.latency_ms.filter(|ms| ms.is_finite()))
            .collect();
        latencies.sort_by(f64::total_cmp);
        let measured = latencies.len();
        let middle = measured.checked_div(2).unwrap_or(0);
        let median_latency_ms = if measured == 0 {
            None
        } else if measured.checked_rem(2) == Some(1) {
            latencies.get(middle).copied()
        } else {
            let upper = latencies.get(middle).copied();
            let lower = middle
                .checked_sub(1)
                .and_then(|index| latencies.get(index).copied());
            match (lower, upper) {
                (Some(lower), Some(upper)) => Some((lower + upper) / 2.0),
                _ => None,
            }
        };
        let worst_loss = peers
            .into_iter()
            .filter_map(|peer| peer.loss.filter(|loss| loss.is_finite()))
            .max_by(f64::total_cmp);
        Self {
            mix,
            median_latency_ms,
            measured,
            worst_loss,
        }
    }
}

/// Whether `text` holds `needle` (already lower case), ignoring case.
fn holds(text: &str, needle: &str) -> bool {
    text.to_lowercase().contains(needle)
}

impl NodeStore {
    // ----- the Networks page -----

    /// The Networks toolbar's search text (tracked).
    pub fn networks_query(&self) -> String {
        self.networks_query.get()
    }

    /// Sets the Networks toolbar's search text.
    pub fn set_networks_query(&self, query: &str) {
        if self.alive() {
            self.networks_query.set(query.to_owned());
        }
    }

    /// The Networks toolbar's filter (tracked).
    pub fn networks_filter(&self) -> NetworkFilter {
        self.networks_filter.get()
    }

    /// Sets the Networks toolbar's filter.
    pub fn set_networks_filter(&self, filter: NetworkFilter) {
        if self.alive() {
            self.networks_filter.set(filter);
        }
    }

    /// Clears the Networks page's search and filter.
    pub fn clear_networks_narrowing(&self) {
        if self.alive() {
            self.networks_query.set(String::new());
            self.networks_filter.set(NetworkFilter::All);
        }
    }

    /// The networks the search and the filter keep, in list order (tracked).
    pub fn visible_ids(&self) -> Vec<ProfileId> {
        let needle = self.networks_query().trim().to_lowercase();
        let filter = self.networks_filter();
        let profiles: Vec<NetworkProfile> = self.profiles.with(Clone::clone);
        profiles
            .into_iter()
            .filter(|profile| {
                needle.is_empty()
                    || holds(&profile.name, &needle)
                    || holds(&profile.network_name, &needle)
            })
            .filter(|profile| filter.keeps(&self.status(&profile.id)))
            .map(|profile| profile.id)
            .collect()
    }

    /// How many networks run or are starting (tracked).
    pub fn running_count(&self) -> usize {
        self.active_ids()
            .into_iter()
            .filter(|id| self.status(id).is_active())
            .count()
    }

    /// The peers of one network's last reading, if it runs (tracked).
    pub fn peer_mix(&self, id: &ProfileId) -> Option<PeerMix> {
        self.snapshot(id)
            .map(|snapshot| PeerMix::of(&snapshot.peers))
    }

    /// The peers of every running network together, if any runs (tracked).
    pub fn total_peer_mix(&self) -> Option<PeerMix> {
        self.ids()
            .into_iter()
            .filter_map(|id| self.peer_mix(&id))
            .reduce(PeerMix::plus)
    }

    // ----- the Peers page -----

    /// The networks that run (have a reading), in list order (tracked).
    pub fn running_ids(&self) -> Vec<ProfileId> {
        self.ids()
            .into_iter()
            .filter(|id| self.status(id) == Status::Running)
            .collect()
    }

    /// The network the Peers page is narrowed to; `None` for every running network (tracked).
    pub fn peers_network(&self) -> Option<ProfileId> {
        let chosen = self.peers_network.get()?;
        self.running_ids().contains(&chosen).then_some(chosen)
    }

    /// Narrows the Peers page to one network, or to all.
    pub fn set_peers_network(&self, network: Option<ProfileId>) {
        if self.alive() {
            self.peers_network.set(network);
        }
    }

    /// The running networks the Peers page covers: the chosen one, or all (tracked).
    pub fn peers_scope(&self) -> Vec<ProfileId> {
        match self.peers_network() {
            Some(id) => vec![id],
            None => self.running_ids(),
        }
    }

    /// The Peers toolbar's search text (tracked).
    pub fn peers_query(&self) -> String {
        self.peers_query.get()
    }

    /// Sets the Peers toolbar's search text.
    pub fn set_peers_query(&self, query: &str) {
        if self.alive() {
            self.peers_query.set(query.to_owned());
        }
    }

    /// The Peers toolbar's route filter (tracked).
    pub fn peers_route(&self) -> RouteFilter {
        self.peers_route.get()
    }

    /// Sets the Peers toolbar's route filter.
    pub fn set_peers_route(&self, route: RouteFilter) {
        if self.alive() {
            self.peers_route.set(route);
        }
    }

    /// Clears the Peers table's search and route filter.
    pub fn clear_peers_narrowing(&self) {
        if self.alive() {
            self.peers_query.set(String::new());
            self.peers_route.set(RouteFilter::All);
        }
    }

    /// Every peer of the networks in scope, in a stable order (hostname, then id) so rows do not jump when a figure
    /// changes (tracked).
    pub fn scope_peers(&self) -> Vec<PeerRow> {
        let mut rows = Vec::new();
        for id in self.peers_scope() {
            let Some(snapshot) = self.snapshot(&id) else {
                continue;
            };
            let network_name = self.profile(&id).map(|p| p.name).unwrap_or_default();
            rows.extend(snapshot.peers.into_iter().map(|peer| PeerRow {
                network: id.clone(),
                network_name: network_name.clone(),
                peer,
            }));
        }
        rows.sort_by(|a, b| {
            a.peer
                .hostname
                .cmp(&b.peer.hostname)
                .then_with(|| a.peer.peer_id.cmp(&b.peer.peer_id))
                .then_with(|| a.network.cmp(&b.network))
        });
        rows
    }

    /// The peers the search and the route filter keep (tracked).
    pub fn peer_rows(&self) -> Vec<PeerRow> {
        let needle = self.peers_query().trim().to_lowercase();
        let route = self.peers_route();
        self.scope_peers()
            .into_iter()
            .filter(|row| route.keeps(row.peer.route))
            .filter(|row| {
                needle.is_empty()
                    || holds(&row.display_name(), &needle)
                    || row
                        .peer
                        .virtual_ipv4
                        .is_some_and(|net| holds(&net.addr().to_string(), &needle))
            })
            .collect()
    }

    /// The peer the drawer is open on (tracked).
    pub fn open_peer_ref(&self) -> Option<PeerRef> {
        self.peer_open.get()
    }

    /// Opens the peer drawer on `peer`, closing the network drawer.
    pub fn open_peer(&self, peer: PeerRef) {
        if !self.alive() {
            return;
        }
        self.close_dialogs();
        self.peer_open.set(Some(peer));
    }

    /// Closes the peer drawer.
    pub fn close_peer(&self) {
        if self.alive() {
            self.peer_open.set(None);
        }
    }

    /// What the core reports of `peer` now, if it is still there (tracked).
    pub fn peer(&self, peer: &PeerRef) -> Option<PeerSummary> {
        self.snapshot(&peer.network)?
            .peers
            .into_iter()
            .find(|found| found.peer_id == peer.peer_id)
    }

    // ----- the log tab -----

    /// Whether the log tab re-reads the output at every poll (tracked).
    pub fn logs_follow(&self) -> bool {
        self.logs_follow.get()
    }

    /// Turns following the log on or off; turning it on reads the output at once.
    pub fn set_logs_follow(&self, follow: bool) {
        if !self.alive() {
            return;
        }
        self.logs_follow.set(follow);
        if follow && let Some(id) = self.detail.get_untracked() {
            self.load_logs(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(latency: Option<f64>, loss: Option<f64>, route: RouteKind) -> PeerSummary {
        PeerSummary {
            latency_ms: latency,
            loss,
            route,
            ..PeerSummary::default()
        }
    }

    #[test]
    fn stats_take_the_median_and_the_worst() {
        let peers = [
            peer(Some(30.0), Some(0.0), RouteKind::Direct),
            peer(Some(10.0), Some(0.2), RouteKind::Relayed { hops: 2 }),
            peer(None, None, RouteKind::Unknown),
            peer(Some(20.0), Some(0.05), RouteKind::Direct),
        ];
        let stats = PeerStats::of(&peers);
        assert_eq!(
            stats.mix,
            PeerMix {
                online: 4,
                direct: 2,
                relayed: 1
            }
        );
        assert_eq!(stats.measured, 3);
        assert_eq!(stats.median_latency_ms, Some(20.0));
        assert_eq!(stats.worst_loss, Some(0.2));
    }

    #[test]
    fn an_even_count_averages_the_middle_pair() {
        let peers = [
            peer(Some(10.0), None, RouteKind::Direct),
            peer(Some(40.0), None, RouteKind::Direct),
        ];
        assert_eq!(PeerStats::of(&peers).median_latency_ms, Some(25.0));
    }

    #[test]
    fn nothing_measured_has_no_figures() {
        let peers = [peer(None, None, RouteKind::Unknown)];
        let stats = PeerStats::of(&peers);
        assert_eq!(stats.median_latency_ms, None);
        assert_eq!(stats.worst_loss, None);
        assert_eq!(stats.measured, 0);
    }

    #[test]
    fn filters_round_trip_through_their_keys() {
        for filter in NetworkFilter::ALL {
            assert_eq!(NetworkFilter::from_key(filter.key()), Some(filter));
        }
        for filter in RouteFilter::ALL {
            assert_eq!(RouteFilter::from_key(filter.key()), Some(filter));
        }
        assert_eq!(RouteFilter::from_key("sideways"), None);
    }

    #[test]
    fn the_route_filter_keeps_what_it_names() {
        assert!(RouteFilter::Direct.keeps(RouteKind::Direct));
        assert!(!RouteFilter::Direct.keeps(RouteKind::Relayed { hops: 2 }));
        assert!(RouteFilter::Relayed.keeps(RouteKind::Relayed { hops: 3 }));
        assert!(!RouteFilter::Relayed.keeps(RouteKind::Unknown));
        assert!(RouteFilter::All.keeps(RouteKind::Unknown));
    }
}
