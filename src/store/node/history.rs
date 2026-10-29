//! The last few minutes of every running network, kept in memory for the charts.
//!
//! Each poll that lands a snapshot pushes one sample per network into a ring of [`HISTORY_CAPACITY`] samples (three
//! minutes at the 1 s poll). A sample keeps what the charts need: the byte totals, the peer counts, and per peer
//! its latency, loss and byte counters. Rates are derived once, when the sample lands, against the sample before it:
//!
//! - a rate is the growth of a peer's counter over the elapsed seconds; a counter that went down (the peer reconnected,
//!   the core restarted) counts as 0 for that step, never as a negative rate;
//! - a network's rate is the sum of its peers' rates over the peers present in both samples, so a peer joining or
//!   leaving moves the totals by its own traffic only (no spike, no dip);
//! - a step longer than [`MAX_GAP_MS`] has no rate (the poll was paused or failed), and the sample after it starts
//!   over from there.
//!
//! A network's history is dropped when it stops, restarts or is deleted. Nothing here is persisted.
//!
//! The data lives outside the signal graph (`Stored`) and a version signal is bumped once per poll, so a poll costs one
//! push and readers never clone the ring: they copy out the series they draw.

use std::collections::{BTreeMap, VecDeque};

use super::{NodeStore, views::Runtime};
use crate::{
    core::state::{NetworkState, NodeSnapshot, PeerSummary, RouteKind},
    profile::ProfileId,
};

/// How many samples a network keeps: [`HISTORY_SPAN_MS`] at the 1 s poll interval.
pub const HISTORY_CAPACITY: usize = 180;
/// The time the charts span, up to the newest sample: what the ring holds at the poll interval.
pub const HISTORY_SPAN_MS: f64 = 180_000.0;
/// The longest step between two samples that still yields a rate.
pub const MAX_GAP_MS: f64 = 10_000.0;
/// Samples closer than this are the same reading (a rate over them would be noise).
const MIN_STEP_MS: f64 = 1.0;
/// Readings closer than this belong to one poll and share one moment on a chart's axis: half the 1 s poll interval,
/// so the networks read in one poll line up and two polls of one network never merge.
pub const SLOT_MS: f64 = 500.0;

/// Throughput at one moment, in bytes per second.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RatePoint {
    /// When, in Unix milliseconds (`nsh_core::time::unix_ms`).
    pub t_ms: f64,
    /// Bytes per second received.
    pub rx_bps: f64,
    /// Bytes per second sent.
    pub tx_bps: f64,
}

/// One peer's traffic now, in bytes per second; each direction `None` before two readings of its counter.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PeerRates {
    /// Received from the peer.
    pub rx_bps: Option<f64>,
    /// Sent to the peer.
    pub tx_bps: Option<f64>,
}

/// How many peers a network had at one moment.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PeerCountsPoint {
    /// When, in Unix milliseconds.
    pub t_ms: f64,
    /// Peers listed (online).
    pub online: usize,
    /// Peers reached over a direct (P2P) link.
    pub direct: usize,
    /// Peers reached through other nodes.
    pub relayed: usize,
}

impl PeerCountsPoint {
    fn at(t_ms: f64, peers: &[PeerSummary]) -> Self {
        let mut point = Self {
            t_ms,
            online: peers.len(),
            direct: 0,
            relayed: 0,
        };
        for peer in peers {
            match peer.route {
                RouteKind::Direct => point.direct = point.direct.saturating_add(1),
                RouteKind::Relayed { .. } => point.relayed = point.relayed.saturating_add(1),
                RouteKind::Unknown => {}
            }
        }
        point
    }
}

/// The bytes a network moved with all its peers since they connected: the sum of the peers' counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Totals {
    /// Bytes received.
    pub rx_bytes: u64,
    /// Bytes sent.
    pub tx_bytes: u64,
}

/// One peer's series of `(t_ms, value)` points.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerSeries {
    /// The core's numeric peer id.
    pub peer_id: u32,
    /// The points, oldest first.
    pub points: Vec<(f64, f64)>,
}

/// What one sample keeps of a peer.
#[derive(Debug, Clone, PartialEq)]
struct PeerSample {
    peer_id: u32,
    latency_ms: Option<f64>,
    loss: Option<f64>,
    rx_bytes: Option<u64>,
    tx_bytes: Option<u64>,
    rx_bps: Option<f64>,
    tx_bps: Option<f64>,
}

/// One poll of one network.
#[derive(Debug, Clone, PartialEq)]
struct Sample {
    t_ms: f64,
    totals: Totals,
    counts: PeerCountsPoint,
    /// Against the sample before; `None` for the first sample, after a gap, or when no peer counted bytes.
    rate: Option<RatePoint>,
    peers: Vec<PeerSample>,
}

impl Sample {
    fn of(snapshot: &NodeSnapshot) -> Self {
        let mut totals = Totals::default();
        let mut peers = Vec::with_capacity(snapshot.peers.len());
        for peer in &snapshot.peers {
            totals.rx_bytes = totals.rx_bytes.saturating_add(peer.rx_bytes.unwrap_or(0));
            totals.tx_bytes = totals.tx_bytes.saturating_add(peer.tx_bytes.unwrap_or(0));
            peers.push(PeerSample {
                peer_id: peer.peer_id,
                latency_ms: peer.latency_ms.filter(|ms| ms.is_finite()),
                loss: peer.loss.filter(|loss| loss.is_finite()),
                rx_bytes: peer.rx_bytes,
                tx_bytes: peer.tx_bytes,
                rx_bps: None,
                tx_bps: None,
            });
        }
        Self {
            t_ms: snapshot.read_at_ms,
            totals,
            counts: PeerCountsPoint::at(snapshot.read_at_ms, &snapshot.peers),
            rate: None,
            peers,
        }
    }

    /// Derives the rates of this sample against `prev`.
    fn derive_rates(&mut self, prev: &Sample) {
        let dt_ms = self.t_ms - prev.t_ms;
        if dt_ms > MAX_GAP_MS {
            return;
        }
        let dt_s = dt_ms / 1000.0;
        let before: BTreeMap<u32, &PeerSample> =
            prev.peers.iter().map(|peer| (peer.peer_id, peer)).collect();
        let (mut rx, mut tx, mut counted) = (0.0, 0.0, false);
        for peer in &mut self.peers {
            let Some(old) = before.get(&peer.peer_id) else {
                continue;
            };
            peer.rx_bps = bps(old.rx_bytes, peer.rx_bytes, dt_s);
            peer.tx_bps = bps(old.tx_bytes, peer.tx_bytes, dt_s);
            if let Some(value) = peer.rx_bps {
                rx += value;
                counted = true;
            }
            if let Some(value) = peer.tx_bps {
                tx += value;
                counted = true;
            }
        }
        if counted || self.peers.is_empty() {
            self.rate = Some(RatePoint {
                t_ms: self.t_ms,
                rx_bps: rx,
                tx_bps: tx,
            });
        }
    }
}

/// Bytes per second from two counter readings `dt_s` apart; a counter that went down counts as no traffic.
fn bps(before: Option<u64>, now: Option<u64>, dt_s: f64) -> Option<f64> {
    let (before, now) = (before?, now?);
    Some(now.saturating_sub(before) as f64 / dt_s)
}

/// The ring of one network's samples.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct History {
    samples: VecDeque<Sample>,
}

impl History {
    /// Appends the reading `snapshot`. Returns whether it was kept: a reading not newer than the last one is
    /// dropped (the same poll landing twice).
    pub(super) fn push(&mut self, snapshot: &NodeSnapshot) -> bool {
        if !snapshot.read_at_ms.is_finite() {
            return false;
        }
        let mut sample = Sample::of(snapshot);
        if let Some(last) = self.samples.back() {
            if sample.t_ms - last.t_ms < MIN_STEP_MS {
                return false;
            }
            sample.derive_rates(last);
        }
        while self.samples.len() >= HISTORY_CAPACITY {
            let _oldest = self.samples.pop_front();
        }
        self.samples.push_back(sample);
        true
    }

    fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn rate_series(&self) -> Vec<RatePoint> {
        self.samples
            .iter()
            .filter_map(|sample| sample.rate)
            .collect()
    }

    fn latest_rate(&self) -> Option<RatePoint> {
        self.samples.back().and_then(|sample| sample.rate)
    }

    fn latest_totals(&self) -> Option<Totals> {
        self.samples.back().map(|sample| sample.totals)
    }

    fn counts_series(&self) -> Vec<PeerCountsPoint> {
        self.samples.iter().map(|sample| sample.counts).collect()
    }

    fn latest_counts(&self) -> Option<PeerCountsPoint> {
        self.samples.back().map(|sample| sample.counts)
    }

    /// `(t_ms, value)` of `value(peer)` over the samples that list `peer_id` and have a value.
    fn peer_series(
        &self,
        peer_id: u32,
        value: impl Fn(&PeerSample) -> Option<f64>,
    ) -> Vec<(f64, f64)> {
        self.samples
            .iter()
            .filter_map(|sample| {
                let peer = sample.peers.iter().find(|peer| peer.peer_id == peer_id)?;
                Some((sample.t_ms, value(peer)?))
            })
            .collect()
    }

    fn peer_rate_series(&self, peer_id: u32) -> Vec<RatePoint> {
        self.samples
            .iter()
            .filter_map(|sample| {
                let peer = sample.peers.iter().find(|peer| peer.peer_id == peer_id)?;
                Some(RatePoint {
                    t_ms: sample.t_ms,
                    rx_bps: peer.rx_bps?,
                    tx_bps: peer.tx_bps.unwrap_or(0.0),
                })
            })
            .collect()
    }

    /// Peer `peer_id`'s received and sent rate at the latest sample; each `None` when the peer was not in the sample
    /// before, a gap came between them, or the core did not count that direction.
    fn latest_peer_rates(&self, peer_id: u32) -> PeerRates {
        self.samples
            .back()
            .and_then(|sample| sample.peers.iter().find(|peer| peer.peer_id == peer_id))
            .map_or(PeerRates::default(), |peer| PeerRates {
                rx_bps: peer.rx_bps,
                tx_bps: peer.tx_bps,
            })
    }

    /// The `n` peers of the latest sample with the highest latency, each with its latency series.
    fn top_latency(&self, n: usize) -> Vec<PeerSeries> {
        let Some(last) = self.samples.back() else {
            return Vec::new();
        };
        let mut ranked: Vec<(u32, f64)> = last
            .peers
            .iter()
            .filter_map(|peer| Some((peer.peer_id, peer.latency_ms?)))
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        ranked
            .into_iter()
            .take(n)
            .map(|(peer_id, _)| PeerSeries {
                peer_id,
                points: self.peer_series(peer_id, |peer| peer.latency_ms),
            })
            .collect()
    }
}

/// The traffic of all networks now: the latest rate of each, for the networks whose latest reading is recent.
fn aggregate_rate<'a>(histories: impl Iterator<Item = &'a History>) -> Option<RatePoint> {
    let latest: Vec<RatePoint> = histories.filter_map(History::latest_rate).collect();
    let newest = latest.iter().map(|point| point.t_ms).reduce(f64::max)?;
    let mut sum = RatePoint {
        t_ms: newest,
        ..RatePoint::default()
    };
    for point in latest.iter().filter(|p| newest - p.t_ms <= MAX_GAP_MS) {
        sum.rx_bps += point.rx_bps;
        sum.tx_bps += point.tx_bps;
    }
    Some(sum)
}

/// The peer counts of all networks now, summed over the networks whose latest reading is recent.
fn aggregate_counts<'a>(histories: impl Iterator<Item = &'a History>) -> Option<PeerCountsPoint> {
    let latest: Vec<PeerCountsPoint> = histories.filter_map(History::latest_counts).collect();
    let newest = latest.iter().map(|point| point.t_ms).reduce(f64::max)?;
    let mut sum = PeerCountsPoint {
        t_ms: newest,
        ..PeerCountsPoint::default()
    };
    for point in latest.iter().filter(|p| newest - p.t_ms <= MAX_GAP_MS) {
        sum.online = sum.online.saturating_add(point.online);
        sum.direct = sum.direct.saturating_add(point.direct);
        sum.relayed = sum.relayed.saturating_add(point.relayed);
    }
    Some(sum)
}

/// The traffic of all networks over time. Networks are polled at slightly different moments, so the samples are
/// grouped (those within [`SLOT_MS`] of the group's first) and each group sums, per network, its latest
/// rate at or before the group's last sample, when that is not older than [`MAX_GAP_MS`].
fn aggregate_rate_series<'a>(histories: impl Iterator<Item = &'a History>) -> Vec<RatePoint> {
    let series: Vec<Vec<RatePoint>> = histories.map(History::rate_series).collect();
    let mut times: Vec<f64> = series.iter().flatten().map(|point| point.t_ms).collect();
    times.sort_by(f64::total_cmp);
    let mut ends: Vec<f64> = Vec::new();
    let mut group_start: Option<f64> = None;
    for t in times {
        let joins = group_start.is_some_and(|start| t - start <= SLOT_MS);
        if joins && let Some(end) = ends.last_mut() {
            *end = t;
            continue;
        }
        group_start = Some(t);
        ends.push(t);
    }
    ends.into_iter()
        .map(|end| {
            let mut sum = RatePoint {
                t_ms: end,
                ..RatePoint::default()
            };
            for points in &series {
                let upto = points.partition_point(|point| point.t_ms <= end);
                let Some(point) = upto.checked_sub(1).and_then(|i| points.get(i)) else {
                    continue;
                };
                if end - point.t_ms <= MAX_GAP_MS {
                    sum.rx_bps += point.rx_bps;
                    sum.tx_bps += point.tx_bps;
                }
            }
            sum
        })
        .collect()
}

impl NodeStore {
    // ----- keeping the history -----

    /// Called by [`NodeStore::change`] with a network's runtime before and after: drops the history when the network
    /// stopped, restarted or went busy, and records the snapshot a poll (or a start) brought.
    pub(super) fn track_history(&self, id: &ProfileId, old: &Runtime, new: &Runtime) {
        let live = match new.op {
            None => match &new.state {
                NetworkState::Running(snapshot) => Some(snapshot),
                NetworkState::Stopped
                | NetworkState::Starting
                | NetworkState::Stopping
                | NetworkState::Failed { .. } => None,
            },
            Some(_) => None,
        };
        if new.revision != old.revision || live.is_none() {
            self.clear_history(id);
        }
        let Some(snapshot) = live else {
            return;
        };
        let already_seen = old
            .state
            .snapshot()
            .is_some_and(|seen| seen.read_at_ms == snapshot.read_at_ms);
        if already_seen {
            return;
        }
        let kept = self
            .history
            .with(|map| map.entry(id.clone()).or_default().push(snapshot));
        if kept {
            self.bump_history();
        }
    }

    /// Drops everything kept of network `id` (it stopped, restarted or was deleted).
    pub(super) fn clear_history(&self, id: &ProfileId) {
        let had = self
            .history
            .with(|map| map.remove(id).is_some_and(|history| !history.is_empty()));
        if had {
            self.bump_history();
        }
    }

    fn bump_history(&self) {
        self.history_version
            .update(|version| *version = version.wrapping_add(1));
    }

    /// Reads the histories, re-running the reader when a poll lands.
    fn read_history<R>(&self, f: impl FnOnce(&BTreeMap<ProfileId, History>) -> R) -> R {
        self.history_version.with(|_| ());
        self.history.with(|map| f(map))
    }

    // ----- what pages read (all tracked) -----

    /// How many samples network `id` keeps now (at most [`HISTORY_CAPACITY`]).
    pub fn sample_count(&self, id: &ProfileId) -> usize {
        self.read_history(|map| map.get(id).map_or(0, History::len))
    }

    /// Network `id`'s throughput over the kept samples, oldest first. Samples without a rate (the first one, after a
    /// gap) are left out.
    pub fn rate_series(&self, id: &ProfileId) -> Vec<RatePoint> {
        self.read_history(|map| map.get(id).map(History::rate_series).unwrap_or_default())
    }

    /// Network `id`'s throughput at its latest sample; `None` before a second sample, after a gap, or while it does
    /// not run.
    pub fn latest_rate(&self, id: &ProfileId) -> Option<RatePoint> {
        self.read_history(|map| map.get(id).and_then(History::latest_rate))
    }

    /// The bytes network `id` moved with all its peers, at its latest sample.
    pub fn latest_totals(&self, id: &ProfileId) -> Option<Totals> {
        self.read_history(|map| map.get(id).and_then(History::latest_totals))
    }

    /// The traffic of all running networks now: the sum of each one's latest rate. `None` when none has a rate.
    pub fn aggregate_rate(&self) -> Option<RatePoint> {
        self.read_history(|map| aggregate_rate(map.values()))
    }

    /// The traffic of all running networks over time (see the module docs for how their samples line up).
    pub fn aggregate_rate_series(&self) -> Vec<RatePoint> {
        self.read_history(|map| aggregate_rate_series(map.values()))
    }

    /// Network `id`'s peer counts over the kept samples, oldest first.
    pub fn peer_counts_series(&self, id: &ProfileId) -> Vec<PeerCountsPoint> {
        self.read_history(|map| map.get(id).map(History::counts_series).unwrap_or_default())
    }

    /// Network `id`'s peer counts at its latest sample.
    pub fn latest_peer_counts(&self, id: &ProfileId) -> Option<PeerCountsPoint> {
        self.read_history(|map| map.get(id).and_then(History::latest_counts))
    }

    /// The peer counts of all running networks now, summed. `None` when none has a sample.
    pub fn aggregate_peer_counts(&self) -> Option<PeerCountsPoint> {
        self.read_history(|map| aggregate_counts(map.values()))
    }

    /// Peer `peer_id`'s latency in milliseconds over time in network `id`, as `(t_ms, ms)`, oldest first.
    pub fn peer_latency_series(&self, id: &ProfileId, peer_id: u32) -> Vec<(f64, f64)> {
        self.read_history(|map| {
            map.get(id)
                .map(|history| history.peer_series(peer_id, |peer| peer.latency_ms))
                .unwrap_or_default()
        })
    }

    /// Peer `peer_id`'s packet loss over time in network `id`, as `(t_ms, fraction 0..=1)`, oldest first.
    pub fn peer_loss_series(&self, id: &ProfileId, peer_id: u32) -> Vec<(f64, f64)> {
        self.read_history(|map| {
            map.get(id)
                .map(|history| history.peer_series(peer_id, |peer| peer.loss))
                .unwrap_or_default()
        })
    }

    /// Peer `peer_id`'s own throughput over time in network `id`, oldest first.
    pub fn peer_rate_series(&self, id: &ProfileId, peer_id: u32) -> Vec<RatePoint> {
        self.read_history(|map| {
            map.get(id)
                .map(|history| history.peer_rate_series(peer_id))
                .unwrap_or_default()
        })
    }

    /// Peer `peer_id`'s received and sent rate in network `id` now (bytes per second at the latest sample).
    pub fn latest_peer_rates(&self, id: &ProfileId, peer_id: u32) -> PeerRates {
        self.read_history(|map| {
            map.get(id).map_or(PeerRates::default(), |history| {
                history.latest_peer_rates(peer_id)
            })
        })
    }

    /// The `n` peers of network `id` with the highest latency now, each with its latency series (the Peers page's
    /// latency chart).
    pub fn top_latency_series(&self, id: &ProfileId, n: usize) -> Vec<PeerSeries> {
        self.read_history(|map| {
            map.get(id)
                .map(|history| history.top_latency(n))
                .unwrap_or_default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::state::NodeInfo;

    fn peer(id: u32, rx: u64, tx: u64) -> PeerSummary {
        PeerSummary {
            peer_id: id,
            rx_bytes: Some(rx),
            tx_bytes: Some(tx),
            ..PeerSummary::default()
        }
    }

    fn snapshot(t_ms: f64, peers: Vec<PeerSummary>) -> NodeSnapshot {
        NodeSnapshot {
            read_at_ms: t_ms,
            node: NodeInfo::default(),
            peers,
        }
    }

    fn pushed(history: &mut History, t_ms: f64, peers: Vec<PeerSummary>) -> bool {
        history.push(&snapshot(t_ms, peers))
    }

    #[test]
    fn first_sample_has_no_rate_and_the_second_does() {
        let mut history = History::default();
        assert!(pushed(&mut history, 10_000.0, vec![peer(1, 1_000, 500)]));
        assert_eq!(history.latest_rate(), None);
        assert!(pushed(&mut history, 12_000.0, vec![peer(1, 5_000, 1_500)]));
        let rate = history.latest_rate().unwrap();
        assert_eq!(rate.t_ms, 12_000.0);
        assert_eq!(rate.rx_bps, 2_000.0);
        assert_eq!(rate.tx_bps, 500.0);
        assert_eq!(history.rate_series(), vec![rate]);
        assert_eq!(
            history.latest_totals(),
            Some(Totals {
                rx_bytes: 5_000,
                tx_bytes: 1_500
            })
        );
    }

    #[test]
    fn rates_sum_over_peers_and_follow_the_elapsed_time() {
        let mut history = History::default();
        pushed(&mut history, 0.0, vec![peer(1, 0, 0), peer(2, 100, 0)]);
        pushed(
            &mut history,
            4_000.0,
            vec![peer(1, 4_000, 800), peer(2, 2_100, 0)],
        );
        let rate = history.latest_rate().unwrap();
        assert_eq!(rate.rx_bps, 1_500.0);
        assert_eq!(rate.tx_bps, 200.0);
        let second = history.peer_rate_series(2);
        assert_eq!(second.len(), 1);
        assert_eq!(second.first().map(|p| p.rx_bps), Some(500.0));
    }

    #[test]
    fn a_counter_reset_is_zero_not_negative() {
        let mut history = History::default();
        pushed(
            &mut history,
            0.0,
            vec![peer(1, 10_000, 10_000), peer(2, 0, 0)],
        );
        pushed(
            &mut history,
            2_000.0,
            vec![peer(1, 50, 20_000), peer(2, 2_000, 0)],
        );
        let rate = history.latest_rate().unwrap();
        assert_eq!(rate.rx_bps, 1_000.0);
        assert_eq!(rate.tx_bps, 5_000.0);
        let reset = history.peer_rate_series(1);
        assert_eq!(reset.first().map(|p| p.rx_bps), Some(0.0));
    }

    #[test]
    fn a_peer_joining_or_leaving_does_not_move_the_rate() {
        let mut history = History::default();
        pushed(
            &mut history,
            0.0,
            vec![peer(1, 0, 0), peer(2, 90_000, 90_000)],
        );
        pushed(&mut history, 2_000.0, vec![peer(1, 2_000, 0)]);
        assert_eq!(history.latest_rate().map(|r| r.rx_bps), Some(1_000.0));
        pushed(
            &mut history,
            4_000.0,
            vec![peer(1, 4_000, 0), peer(3, 7_000_000, 0)],
        );
        assert_eq!(history.latest_rate().map(|r| r.rx_bps), Some(1_000.0));
    }

    #[test]
    fn a_gap_longer_than_ten_seconds_has_no_rate() {
        let mut history = History::default();
        pushed(&mut history, 0.0, vec![peer(1, 0, 0)]);
        pushed(&mut history, 10_001.0, vec![peer(1, 9_000, 0)]);
        assert_eq!(history.latest_rate(), None);
        pushed(&mut history, 12_001.0, vec![peer(1, 11_000, 0)]);
        assert_eq!(history.latest_rate().map(|r| r.rx_bps), Some(1_000.0));
        assert_eq!(history.rate_series().len(), 1);
        // exactly ten seconds is still a step
        pushed(&mut history, 22_001.0, vec![peer(1, 21_000, 0)]);
        assert_eq!(history.latest_rate().map(|r| r.rx_bps), Some(1_000.0));
    }

    #[test]
    fn no_peers_is_zero_traffic_and_uncounted_peers_have_no_rate() {
        let mut history = History::default();
        pushed(&mut history, 0.0, Vec::new());
        pushed(&mut history, 2_000.0, Vec::new());
        assert_eq!(
            history.latest_rate().map(|r| (r.rx_bps, r.tx_bps)),
            Some((0.0, 0.0))
        );

        let mut history = History::default();
        let uncounted = PeerSummary {
            peer_id: 4,
            ..PeerSummary::default()
        };
        pushed(&mut history, 0.0, vec![uncounted.clone()]);
        pushed(&mut history, 2_000.0, vec![uncounted]);
        assert_eq!(history.latest_rate(), None);
    }

    #[test]
    fn readings_not_newer_than_the_last_are_dropped() {
        let mut history = History::default();
        assert!(pushed(&mut history, 5_000.0, Vec::new()));
        assert!(!pushed(&mut history, 5_000.0, Vec::new()));
        assert!(!pushed(&mut history, 4_000.0, Vec::new()));
        assert!(!pushed(&mut history, f64::NAN, Vec::new()));
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn the_ring_keeps_the_newest_three_minutes() {
        let mut history = History::default();
        for step in 0..205_u32 {
            let bytes = u64::from(step).saturating_mul(1_000);
            pushed(
                &mut history,
                f64::from(step) * 1_000.0,
                vec![peer(1, bytes, 0)],
            );
        }
        assert_eq!(history.len(), HISTORY_CAPACITY);
        let series = history.rate_series();
        assert_eq!(series.len(), HISTORY_CAPACITY);
        assert_eq!(series.first().map(|p| p.t_ms), Some(25.0 * 1_000.0));
        assert_eq!(series.last().map(|p| p.t_ms), Some(204.0 * 1_000.0));
    }

    #[test]
    fn peer_counts_split_direct_and_relayed() {
        let mut history = History::default();
        let mut direct = peer(1, 0, 0);
        direct.route = RouteKind::Direct;
        let mut relayed = peer(2, 0, 0);
        relayed.route = RouteKind::Relayed { hops: 2 };
        let unknown = peer(3, 0, 0);
        pushed(&mut history, 1_000.0, vec![direct, relayed, unknown]);
        let counts = history.latest_counts().unwrap();
        assert_eq!((counts.online, counts.direct, counts.relayed), (3, 1, 1));
        assert_eq!(counts.t_ms, 1_000.0);
        assert_eq!(history.counts_series(), vec![counts]);
    }

    #[test]
    fn peer_latency_and_loss_series_skip_unmeasured_samples() {
        let mut history = History::default();
        let measured = |id: u32, ms: Option<f64>, loss: Option<f64>| PeerSummary {
            peer_id: id,
            latency_ms: ms,
            loss,
            ..PeerSummary::default()
        };
        pushed(&mut history, 0.0, vec![measured(7, Some(12.0), Some(0.0))]);
        pushed(
            &mut history,
            2_000.0,
            vec![measured(7, None, None), measured(8, Some(3.0), None)],
        );
        pushed(
            &mut history,
            4_000.0,
            vec![measured(7, Some(20.0), Some(0.25))],
        );
        assert_eq!(
            history.peer_series(7, |p| p.latency_ms),
            vec![(0.0, 12.0), (4_000.0, 20.0)]
        );
        assert_eq!(
            history.peer_series(7, |p| p.loss),
            vec![(0.0, 0.0), (4_000.0, 0.25)]
        );
        assert_eq!(
            history.peer_series(8, |p| p.latency_ms),
            vec![(2_000.0, 3.0)]
        );
        assert!(history.peer_series(99, |p| p.latency_ms).is_empty());
    }

    #[test]
    fn top_latency_ranks_the_latest_sample() {
        let mut history = History::default();
        let at = |id: u32, ms: f64| PeerSummary {
            peer_id: id,
            latency_ms: Some(ms),
            ..PeerSummary::default()
        };
        pushed(
            &mut history,
            0.0,
            vec![at(1, 5.0), at(2, 50.0), at(3, 20.0)],
        );
        pushed(
            &mut history,
            2_000.0,
            vec![at(1, 6.0), at(2, 40.0), at(3, 90.0)],
        );
        let top = history.top_latency(2);
        let ids: Vec<u32> = top.iter().map(|series| series.peer_id).collect();
        assert_eq!(ids, vec![3, 2]);
        assert_eq!(top.first().map(|s| s.points.len()), Some(2));
    }

    #[test]
    fn the_aggregate_sums_the_latest_rates_of_fresh_networks() {
        let mut a = History::default();
        let mut b = History::default();
        let mut stale = History::default();
        pushed(&mut a, 100_000.0, vec![peer(1, 0, 0)]);
        pushed(&mut a, 102_000.0, vec![peer(1, 2_000, 0)]);
        pushed(&mut b, 100_300.0, vec![peer(1, 0, 0)]);
        pushed(&mut b, 102_300.0, vec![peer(1, 0, 4_000)]);
        pushed(&mut stale, 0.0, vec![peer(1, 0, 0)]);
        pushed(&mut stale, 2_000.0, vec![peer(1, 8_000, 8_000)]);
        let all = [&a, &b, &stale];
        let sum = aggregate_rate(all.iter().copied()).unwrap();
        assert_eq!(sum.t_ms, 102_300.0);
        assert_eq!((sum.rx_bps, sum.tx_bps), (1_000.0, 2_000.0));
        assert_eq!(aggregate_rate(std::iter::empty()), None);
        assert_eq!(aggregate_rate([&History::default()].into_iter()), None);
    }

    #[test]
    fn the_aggregate_series_lines_networks_up() {
        let mut a = History::default();
        let mut b = History::default();
        for step in 0..3_u32 {
            // One poll a second; the second network's reading lands 300 ms after the first's.
            let t = 1_000_000.0 + f64::from(step) * 1_000.0;
            let bytes = u64::from(step);
            pushed(&mut a, t, vec![peer(1, bytes.saturating_mul(1_000), 0)]);
            pushed(
                &mut b,
                t + 300.0,
                vec![peer(1, bytes.saturating_mul(3_000), 0)],
            );
        }
        let series = aggregate_rate_series([&a, &b].into_iter());
        assert_eq!(series.len(), 2);
        assert!(series.iter().all(|p| p.rx_bps == 4_000.0));
        assert_eq!(series.first().map(|p| p.t_ms), Some(1_001_300.0));
        assert_eq!(series.last().map(|p| p.t_ms), Some(1_002_300.0));
        assert!(aggregate_rate_series(std::iter::empty()).is_empty());
    }

    #[test]
    fn aggregate_counts_sum_the_networks() {
        let mut a = History::default();
        let mut b = History::default();
        let mut direct = peer(1, 0, 0);
        direct.route = RouteKind::Direct;
        let mut relayed = peer(2, 0, 0);
        relayed.route = RouteKind::Relayed { hops: 3 };
        pushed(&mut a, 5_000.0, vec![direct.clone(), relayed]);
        pushed(&mut b, 5_100.0, vec![direct]);
        let sum = aggregate_counts([&a, &b].into_iter()).unwrap();
        assert_eq!((sum.online, sum.direct, sum.relayed), (3, 2, 1));
        assert_eq!(sum.t_ms, 5_100.0);
    }
}
