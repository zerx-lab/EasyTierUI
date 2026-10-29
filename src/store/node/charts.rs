//! The series the charts draw, cut from the history: throughput of a scope of networks and the latency of the peers
//! that are slowest now, each with the moments its samples were read (the charts' time axis), the series of a chart
//! laid on one shared set of moments so a sample missing for one peer is a gap in its line and not a shift of the
//! others.

use super::{NodeStore, PeerSeries, history::SLOT_MS};
use crate::profile::ProfileId;

/// How many peers the latency chart draws.
pub const LATENCY_LINES: usize = 5;

/// Throughput over time: one value per sample, oldest first.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThroughputSeries {
    /// When each sample was read, in Unix milliseconds.
    pub t: Vec<f64>,
    /// Bytes per second received.
    pub rx: Vec<f64>,
    /// Bytes per second sent.
    pub tx: Vec<f64>,
}

/// One peer's latency over the shared moments of its chart.
#[derive(Debug, Clone, PartialEq)]
pub struct LatencyLine {
    /// Stable key of the peer across networks (`<network>/<peer id>`), what its color is hashed from.
    pub key: String,
    /// The network the peer is in.
    pub network: ProfileId,
    /// The core's numeric peer id.
    pub peer_id: u32,
    /// Milliseconds per moment of the chart; `NaN` where the peer had no measurement.
    pub samples: Vec<f64>,
}

/// The latency chart: its moments and the lines of the slowest peers on them, in the order of their keys.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LatencyLines {
    /// When each slot of the chart starts, in Unix milliseconds.
    pub t: Vec<f64>,
    /// The lines, at most [`LATENCY_LINES`].
    pub lines: Vec<LatencyLine>,
}

/// One series over the moments it was read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimedSamples {
    /// When, in Unix milliseconds.
    pub t: Vec<f64>,
    /// The value at each moment; `NaN` where there was none.
    pub values: Vec<f64>,
}

/// Series laid on one axis by [`align`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Aligned {
    /// When each slot starts, in Unix milliseconds.
    pub t: Vec<f64>,
    /// One row per series, a value per slot (`NaN` where the series has none).
    pub rows: Vec<Vec<f64>>,
}

/// Lays `series` (each `(t_ms, value)` oldest first) on one axis: the slots are the distinct moments of all the
/// series, moments less than a second apart sharing one (it starts at the first of them).
pub fn align(series: &[Vec<(f64, f64)>]) -> Aligned {
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
    let rows = series
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
        .collect();
    Aligned { t: starts, rows }
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
            t: points.iter().map(|point| point.t_ms).collect(),
            rx: points.iter().map(|point| point.rx_bps).collect(),
            tx: points.iter().map(|point| point.tx_bps).collect(),
        }
    }

    /// The latency of the (at most [`LATENCY_LINES`]) slowest peers of the networks in `scope`, ordered by key so a
    /// line keeps its place while the figures change, on one shared axis (tracked).
    pub fn latency_lines(&self, scope: &[ProfileId]) -> LatencyLines {
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
        let aligned = align(
            &found
                .iter()
                .map(|(_, series)| series.points.clone())
                .collect::<Vec<_>>(),
        );
        let mut lines: Vec<LatencyLine> = found
            .into_iter()
            .zip(aligned.rows)
            .map(|((network, series), samples)| LatencyLine {
                key: format!("{network}/{}", series.peer_id),
                network,
                peer_id: series.peer_id,
                samples,
            })
            .collect();
        lines.sort_by(|a, b| a.key.cmp(&b.key));
        LatencyLines {
            t: aligned.t,
            lines,
        }
    }

    /// One peer's latency over the samples of its network (tracked): the shared axis is the network's own samples, so
    /// a missing measurement is a gap.
    pub fn peer_latency_samples(&self, network: &ProfileId, peer_id: u32) -> TimedSamples {
        let latency = self.peer_latency_series(network, peer_id);
        self.on_network_axis(network, latency)
    }

    /// One peer's packet loss over the samples of its network, in percent (tracked).
    pub fn peer_loss_samples(&self, network: &ProfileId, peer_id: u32) -> TimedSamples {
        let loss: Vec<(f64, f64)> = self
            .peer_loss_series(network, peer_id)
            .into_iter()
            .map(|(t, fraction)| (t, fraction * 100.0))
            .collect();
        self.on_network_axis(network, loss)
    }

    /// `points` on the moments network `network` was read.
    fn on_network_axis(&self, network: &ProfileId, points: Vec<(f64, f64)>) -> TimedSamples {
        let axis: Vec<(f64, f64)> = self
            .peer_counts_series(network)
            .into_iter()
            .map(|point| (point.t_ms, 0.0))
            .collect();
        let Aligned { t, rows } = align(&[axis, points]);
        TimedSamples {
            t,
            values: rows.into_iter().nth(1).unwrap_or_default(),
        }
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
        let Aligned { t, rows } = align(&[a, b]);
        assert_eq!(t, vec![1_000.0, 3_000.0, 5_000.0]);
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
    fn readings_of_one_poll_share_a_slot_and_consecutive_polls_do_not() {
        let a = vec![(10_000.0, 1.0), (10_950.0, 3.0)];
        let b = vec![(10_300.0, 2.0)];
        let Aligned { t, rows } = align(&[a, b]);
        assert_eq!(t, vec![10_000.0, 10_950.0]);
        let second = rows.get(1).cloned().unwrap_or_default();
        assert_eq!(rows.first().cloned().unwrap_or_default(), vec![1.0, 3.0]);
        assert_eq!(second.first(), Some(&2.0));
        assert!(second.get(1).is_some_and(|v| v.is_nan()));
    }

    #[test]
    fn nothing_aligns_to_nothing() {
        assert_eq!(align(&[]), Aligned::default());
        assert_eq!(align(&[Vec::new()]).rows, vec![Vec::<f64>::new()]);
    }
}
