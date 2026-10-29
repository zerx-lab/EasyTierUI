//! The series the charts draw, cut from the history: throughput of a scope of networks and the latency of the peers
//! that are slowest now, each laid on one shared time axis so a sample missing for one peer is a gap in its line and
//! not a shift of the others.

use super::{NodeStore, PeerSeries};
use crate::profile::ProfileId;

/// Points closer than this share one slot of the axis (samples of different networks never land on the same
/// millisecond).
const SLOT_MS: f64 = 1_000.0;

/// How many peers the latency chart draws.
pub const LATENCY_LINES: usize = 5;

/// Throughput over time: one value per sample, oldest first.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThroughputSeries {
    /// Bytes per second received.
    pub rx: Vec<f64>,
    /// Bytes per second sent.
    pub tx: Vec<f64>,
}

/// One peer's latency over the shared axis of its chart.
#[derive(Debug, Clone, PartialEq)]
pub struct LatencyLine {
    /// Stable key of the peer across networks (`<network>/<peer id>`), what its color is hashed from.
    pub key: String,
    /// The network the peer is in.
    pub network: ProfileId,
    /// The core's numeric peer id.
    pub peer_id: u32,
    /// Milliseconds per slot of the axis; `NaN` where the peer had no measurement.
    pub samples: Vec<f64>,
}

/// Lays `series` (each `(t_ms, value)` oldest first) on one axis: the slots are the distinct moments of all the
/// series, moments less than a second apart sharing one. Returns one row per series with `NaN` where a series has no
/// point in a slot.
pub fn align(series: &[Vec<(f64, f64)>]) -> Vec<Vec<f64>> {
    let mut moments: Vec<f64> = series
        .iter()
        .flatten()
        .map(|(t, _)| *t)
        .filter(|t| t.is_finite())
        .collect();
    moments.sort_by(f64::total_cmp);
    let mut starts: Vec<f64> = Vec::new();
    for t in moments {
        match starts.last() {
            Some(start) if t - start <= SLOT_MS => {}
            _ => starts.push(t),
        }
    }
    series
        .iter()
        .map(|points| {
            let mut row = vec![f64::NAN; starts.len()];
            for (t, value) in points {
                if !t.is_finite() {
                    continue;
                }
                let slot = starts.partition_point(|start| start <= t).checked_sub(1);
                if let Some(cell) = slot.and_then(|slot| row.get_mut(slot)) {
                    *cell = *value;
                }
            }
            row
        })
        .collect()
}

impl NodeStore {
    /// The throughput of the networks in `scope` over time: one network's own series, or the sum of several
    /// (tracked).
    pub fn throughput_series(&self, scope: &[ProfileId]) -> ThroughputSeries {
        let points = match scope {
            [] => Vec::new(),
            [only] => self.rate_series(only),
            _ => self.aggregate_rate_series(),
        };
        ThroughputSeries {
            rx: points.iter().map(|point| point.rx_bps).collect(),
            tx: points.iter().map(|point| point.tx_bps).collect(),
        }
    }

    /// The latency of the (at most [`LATENCY_LINES`]) slowest peers of the networks in `scope`, slowest first, on one
    /// shared axis (tracked).
    pub fn latency_lines(&self, scope: &[ProfileId]) -> Vec<LatencyLine> {
        let mut found: Vec<(ProfileId, PeerSeries)> = scope
            .iter()
            .flat_map(|id| {
                self.top_latency_series(id, LATENCY_LINES)
                    .into_iter()
                    .map(|series| (id.clone(), series))
            })
            .collect();
        // Each network's list is slowest first; keep the slowest overall by the latest point.
        found.sort_by(|(_, a), (_, b)| {
            let last =
                |series: &PeerSeries| series.points.last().map_or(f64::NEG_INFINITY, |p| p.1);
            last(b).total_cmp(&last(a))
        });
        found.truncate(LATENCY_LINES);
        let rows = align(
            &found
                .iter()
                .map(|(_, series)| series.points.clone())
                .collect::<Vec<_>>(),
        );
        found
            .into_iter()
            .zip(rows)
            .map(|((network, series), samples)| LatencyLine {
                key: format!("{network}/{}", series.peer_id),
                network,
                peer_id: series.peer_id,
                samples,
            })
            .collect()
    }

    /// One peer's latency over the samples of its network (tracked): the shared axis is the network's own samples, so
    /// a missing measurement is a gap.
    pub fn peer_latency_samples(&self, network: &ProfileId, peer_id: u32) -> Vec<f64> {
        let axis: Vec<(f64, f64)> = self
            .peer_counts_series(network)
            .into_iter()
            .map(|point| (point.t_ms, 0.0))
            .collect();
        let latency = self.peer_latency_series(network, peer_id);
        align(&[axis, latency])
            .into_iter()
            .nth(1)
            .unwrap_or_default()
    }

    /// One peer's packet loss over the samples of its network, in percent (tracked).
    pub fn peer_loss_samples(&self, network: &ProfileId, peer_id: u32) -> Vec<f64> {
        let axis: Vec<(f64, f64)> = self
            .peer_counts_series(network)
            .into_iter()
            .map(|point| (point.t_ms, 0.0))
            .collect();
        let loss: Vec<(f64, f64)> = self
            .peer_loss_series(network, peer_id)
            .into_iter()
            .map(|(t, fraction)| (t, fraction * 100.0))
            .collect();
        align(&[axis, loss]).into_iter().nth(1).unwrap_or_default()
    }

    /// The received rate of network `id` over time, for a sparkline (tracked).
    pub fn rx_rates(&self, id: &ProfileId) -> Vec<f64> {
        self.rate_series(id)
            .into_iter()
            .map(|point| point.rx_bps)
            .collect()
    }

    /// The sent rate of network `id` over time, for a sparkline (tracked).
    pub fn tx_rates(&self, id: &ProfileId) -> Vec<f64> {
        self.rate_series(id)
            .into_iter()
            .map(|point| point.tx_bps)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_share_the_moments_they_have_in_common() {
        let a = vec![(1_000.0, 1.0), (3_000.0, 2.0), (5_000.0, 3.0)];
        let b = vec![(3_000.0, 20.0), (5_000.0, 30.0)];
        let rows = align(&[a, b]);
        assert_eq!(rows.len(), 2);
        let first = rows.first().cloned().unwrap_or_default();
        let second = rows.get(1).cloned().unwrap_or_default();
        assert_eq!(first, vec![1.0, 2.0, 3.0]);
        assert_eq!(second.len(), 3);
        assert!(second.first().is_some_and(|v| v.is_nan()));
        assert_eq!(second.get(1), Some(&20.0));
        assert_eq!(second.get(2), Some(&30.0));
    }

    #[test]
    fn moments_less_than_a_second_apart_are_one_slot() {
        let a = vec![(10_000.0, 1.0)];
        let b = vec![(10_400.0, 2.0)];
        let rows = align(&[a, b]);
        assert_eq!(rows, vec![vec![1.0], vec![2.0]]);
    }

    #[test]
    fn nothing_aligns_to_nothing() {
        assert!(align(&[]).is_empty());
        assert_eq!(align(&[Vec::new()]), vec![Vec::<f64>::new()]);
    }
}
