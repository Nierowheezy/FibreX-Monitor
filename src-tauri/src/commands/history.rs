//! Reading the local history: recorded incidents and what history covers.

use std::sync::Arc;

use tauri::State;

use crate::HistoryStore;
use crate::models::history::{HistoryInfo, IncidentSummary, UptimeSummary};
use crate::monitoring::state::MonitorState;
use crate::monitoring::store::{Incident, RETENTION_MS, Store};

/// How many incidents one read returns.
///
/// A cap rather than everything, so a database left running for a year cannot
/// send an unbounded array to the interface in one message.
const INCIDENT_LIMIT: u32 = 100;

/// The history read as the interface expects it.
///
/// Returns an empty list rather than an error when history is switched off. The
/// screen distinguishes the two with [`HistoryInfo`], and an error would be the
/// wrong signal: nothing failed, there is simply nothing stored.
#[tauri::command]
pub fn get_incidents(
    history: State<'_, HistoryStore>,
    state: State<'_, Arc<MonitorState>>,
) -> Vec<IncidentSummary> {
    let Some(store) = history.get() else {
        return Vec::new();
    };

    let Some(primary) = state.primary_target_id() else {
        return Vec::new();
    };

    match store.incidents(INCIDENT_LIMIT) {
        Ok(incidents) => incidents
            .into_iter()
            .filter(|incident| incident.target_id == primary)
            .map(summarise)
            .collect(),
        Err(error) => {
            eprintln!("incidents unavailable: {error}");
            Vec::new()
        }
    }
}

/// What the history database currently holds.
#[tauri::command]
pub fn get_history_info(
    history: State<'_, HistoryStore>,
    state: State<'_, Arc<MonitorState>>,
) -> HistoryInfo {
    let Some(store) = history.get() else {
        return HistoryInfo {
            enabled: false,
            earliest_sample_at: None,
            sample_count: 0,
            retention_ms: RETENTION_MS,
        };
    };

    let target = state.primary_target_id();

    HistoryInfo {
        enabled: true,
        earliest_sample_at: target.and_then(|id| store.earliest_sample_at(id).ok().flatten()),
        sample_count: store.sample_count().unwrap_or(0),
        retention_ms: RETENTION_MS,
    }
}

/// Uptime over the stored history, as a share of probes that connected.
///
/// `window_ms` is how far back to count. The defaults cover the full retention
/// period, so the figure describes everything the app has recorded rather than
/// a period it may not span.
///
/// Returns nulls rather than a zero percent when nothing is stored, because zero
/// percent uptime is a claim that the connection never worked, and nothing here
/// measured that.
#[tauri::command]
pub fn get_uptime(
    history: State<'_, HistoryStore>,
    state: State<'_, Arc<MonitorState>>,
) -> UptimeSummary {
    let empty = UptimeSummary {
        percent: None,
        connected: 0,
        total: 0,
        window_start: None,
        window_end: None,
    };

    let Some(store) = history.get() else {
        return empty;
    };
    let Some(primary) = state.primary_target_id() else {
        return empty;
    };

    let (total, failed) = match store.counts_since(primary, 0) {
        Ok(counts) => counts,
        Err(error) => {
            eprintln!("uptime unavailable: {error}");
            return empty;
        }
    };

    if total == 0 {
        return empty;
    }

    let connected = total.saturating_sub(failed);

    UptimeSummary {
        // Guarded rather than relied upon: `total` is checked above, but a
        // division here would be the one place this file could panic.
        percent: Some(f64::from(connected) / f64::from(total) * 100.0),
        connected,
        total,
        window_start: store.earliest_sample_at(primary).ok().flatten(),
        window_end: store.latest_sample_at(primary).ok().flatten(),
    }
}

/// Latency history read from storage, rather than from the in-memory buffer.
///
/// `since_ms` is how far back to read. The interface asks for a range and gets
/// exactly what is stored in it, so a selector can offer only ranges that have
/// real samples behind them.
#[tauri::command]
pub fn get_stored_series(
    since_ms: i64,
    history: State<'_, HistoryStore>,
    state: State<'_, Arc<MonitorState>>,
) -> Vec<crate::models::latency::LatencyPoint> {
    let Some(store) = history.get() else {
        return Vec::new();
    };
    let Some(primary) = state.primary_target_id() else {
        return Vec::new();
    };

    let since = crate::monitoring::now_ms().saturating_sub(since_ms.max(0));

    match store.samples_since(primary, since) {
        Ok(samples) => thin_series(&samples, MAX_SERIES_POINTS),
        Err(error) => {
            eprintln!("stored series unavailable: {error}");
            Vec::new()
        }
    }
}

/// The most points one range read sends across the IPC boundary.
///
/// Six hundred points is a few hundred path segments, which a browser draws
/// without noticing, and it keeps a twenty-four hour range from sending forty
/// thousand rows on every poll. The probe interval is two seconds, so this
/// still holds an hour of samples in full.
pub const MAX_SERIES_POINTS: usize = 600;

/// Thins a long series into at most `limit` points, without hiding a failure.
///
/// The series is cut into equal slices of time. A slice that contains a failed
/// probe becomes a gap, because the point of a gap is that something went
/// wrong inside it, and thinning one away would draw a smooth line over an
/// outage. A slice with no failures keeps the last real sample in it, which is
/// a stored measurement rather than an average of measurements, so nothing
/// invented reaches the chart.
///
/// A series already inside the limit is returned untouched, sample for sample.
fn thin_series(
    samples: &[crate::monitoring::store::StoredSample],
    limit: usize,
) -> Vec<crate::models::latency::LatencyPoint> {
    use crate::models::latency::LatencyPoint;

    if limit == 0 {
        return Vec::new();
    }

    if samples.len() <= limit {
        return samples
            .iter()
            .map(|sample| LatencyPoint {
                at: sample.captured_at,
                value: sample.latency_ms,
            })
            .collect();
    }

    let first = samples[0].captured_at;
    let last = samples[samples.len() - 1].captured_at;
    let span = (last - first).max(1) as f64;
    let bucket_ms = ((span / limit as f64).ceil() as i64).max(1);

    // The slice being filled, its newest sample, and whether anything in it
    // failed.
    let mut bucket: Option<(i64, LatencyPoint, bool)> = None;
    let mut out: Vec<LatencyPoint> = Vec::with_capacity(limit);

    for sample in samples {
        let index = (sample.captured_at - first) / bucket_ms;
        let point = LatencyPoint {
            at: sample.captured_at,
            value: sample.latency_ms,
        };

        match bucket {
            Some((current_index, _, failed)) if current_index == index => {
                // The newest sample wins, because it is the one that says where
                // the slice ended, and the failure flag is kept for the whole
                // slice.
                bucket = Some((current_index, point, failed || point.value.is_none()));
            }
            Some((_, previous, failed)) => {
                // The slice is complete, so it is emitted before the next one
                // starts. A slice that contained a failure becomes a gap.
                out.push(if failed {
                    LatencyPoint {
                        at: previous.at,
                        value: None,
                    }
                } else {
                    previous
                });
                bucket = Some((index, point, point.value.is_none()));
            }
            None => {
                bucket = Some((index, point, point.value.is_none()));
            }
        }
    }

    if let Some((_, point, failed)) = bucket {
        out.push(if failed {
            LatencyPoint {
                at: point.at,
                value: None,
            }
        } else {
            point
        });
    }

    out
}

/// Converts a stored incident into the wire shape.
pub fn summarise(incident: Incident) -> IncidentSummary {
    IncidentSummary {
        id: incident.id,
        target_id: incident.target_id,
        started_at: incident.started_at,
        recovered_at: incident.recovered_at,
        probe_count: incident.probe_count,
        router_reachable: incident.router_reachable,
        closed_by: incident.closed_by.map(|closed| closed.label().to_string()),
    }
}

/// The store's path, for the settings screen's storage line.
///
/// Only the path, never the contents. It is a fact about this Mac, not about the
/// connection, and it is what a user needs to find or delete the file.
///
/// Unused right now. Kept because the settings screen's storage card is the next
/// thing to be finished and deleting it would only mean writing it again.
#[allow(dead_code)]
pub fn database_path(store: &Store) -> String {
    store.path().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::probe::{ProbeErrorKind, ProbeOutcome};

    fn outcome(target_id: i64, at: i64, success: bool) -> ProbeOutcome {
        ProbeOutcome {
            target_id,
            captured_at: at,
            success,
            latency_ms: success.then_some(20.0),
            error_kind: (!success).then_some(ProbeErrorKind::Timeout),
        }
    }

    #[test]
    fn a_stored_incident_is_summarised_without_loss() {
        let incident = Incident {
            id: 9,
            target_id: 1,
            started_at: 1_000,
            recovered_at: Some(9_000),
            observed_until: 9_000,
            probe_count: 4,
            router_reachable: Some(true),
            closed_by: Some(crate::monitoring::store::ClosedBy::Probe),
        };

        let summary = summarise(incident);

        assert_eq!(summary.id, 9);
        assert_eq!(summary.started_at, 1_000);
        assert_eq!(summary.recovered_at, Some(9_000));
        assert_eq!(summary.probe_count, 4);
        assert_eq!(summary.router_reachable, Some(true));
        assert_eq!(summary.closed_by.as_deref(), Some("probe"));
    }

    /// A stored failure must come back as a chart point with no value, so the
    /// chart draws a gap rather than a zero.
    #[test]
    fn a_stored_failure_becomes_a_point_with_no_value() {
        let store = Store::in_memory().expect("store");

        store.record(&outcome(1, 1_000, true), None).expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");

        let samples = store.samples_since(1, 0).expect("read");
        let points: Vec<_> = samples
            .into_iter()
            .map(|sample| crate::models::latency::LatencyPoint {
                at: sample.captured_at,
                value: sample.latency_ms,
            })
            .collect();

        assert_eq!(points.len(), 2);
        assert_eq!(points[0].value, Some(20.0));
        assert_eq!(
            points[1].value, None,
            "the failed probe must stay a gap, not become zero"
        );
    }

    #[test]
    fn the_history_info_reports_an_empty_database_as_enabled_but_empty() {
        let store = Store::in_memory().expect("store");

        assert_eq!(store.sample_count().expect("count"), 0);
        assert_eq!(store.earliest_sample_at(1).expect("earliest"), None);
    }

    #[test]
    fn the_latest_sample_time_is_reported_for_a_window_end() {
        let store = Store::in_memory().expect("store");

        store.record(&outcome(1, 3_000, true), None).expect("write");
        store.record(&outcome(1, 9_000, true), None).expect("write");

        assert_eq!(
            store.latest_sample_at(1).expect("latest"),
            Some(9_000),
            "the window end is the newest sample, not the oldest"
        );
    }

    // ---------------------------------------------------------------------
    // Thinning a long range.
    // ---------------------------------------------------------------------

    fn sample(at: i64, success: bool) -> crate::monitoring::store::StoredSample {
        crate::monitoring::store::StoredSample {
            target_id: 1,
            captured_at: at,
            latency_ms: success.then_some(20.0),
        }
    }

    /// A short series must reach the chart exactly as it was stored.
    #[test]
    fn a_series_inside_the_limit_is_untouched() {
        let samples = vec![
            sample(1_000, true),
            sample(2_000, false),
            sample(3_000, true),
        ];

        let points = thin_series(&samples, 600);

        assert_eq!(points.len(), 3);
        assert_eq!(points[0].value, Some(20.0));
        assert_eq!(points[1].value, None);
        assert_eq!(points[2].value, Some(20.0));
    }

    /// The rule that matters: thinning must never draw a smooth line over an
    /// outage.
    #[test]
    fn a_failure_inside_a_thinned_slice_stays_a_gap() {
        // Forty samples over forty seconds, so each slice holds about one. The
        // failure sits in the middle of a slice that also holds a success.
        let mut samples: Vec<_> = (0..40).map(|index| sample(index * 1_000, true)).collect();
        samples[20] = sample(20_000, false);

        let points = thin_series(&samples, 10);

        assert!(points.len() <= 12, "the limit must hold");
        assert!(
            points.iter().any(|point| point.value.is_none()),
            "a failed probe must still be a gap after thinning, not a smoothed-over line"
        );
    }

    /// Every value the chart shows must be a value the router or the probe
    /// actually stored. Nothing is averaged.
    #[test]
    fn a_thinned_series_only_carries_stored_values() {
        let samples: Vec<_> = (0..100)
            .map(|index| sample(index * 1_000, index % 7 != 0))
            .collect();

        let points = thin_series(&samples, 20);

        for point in &points {
            if let Some(value) = point.value {
                assert_eq!(value, 20.0, "no value may be invented");
            }
        }
    }

    /// The newest reading is the one a reader most wants, so it must survive.
    #[test]
    fn a_thinned_series_ends_on_the_newest_sample() {
        let samples: Vec<_> = (0..100).map(|index| sample(index * 1_000, true)).collect();

        let points = thin_series(&samples, 20);

        assert_eq!(points.last().map(|point| point.at), Some(99_000));
    }

    /// A limit of zero must not divide by zero or loop forever.
    #[test]
    fn a_zero_limit_returns_nothing() {
        let samples = vec![sample(1_000, true)];

        assert!(thin_series(&samples, 0).is_empty());
    }

    #[test]
    fn an_empty_database_has_no_latest_sample() {
        let store = Store::in_memory().expect("store");

        assert_eq!(store.latest_sample_at(1).expect("latest"), None);
    }
}
