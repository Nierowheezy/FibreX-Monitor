//! The latest probe outcome per target, the recent series behind the chart, and
//! the tick that produces them.
//!
//! Storage is deliberately in-memory only. Feature 5 owns persistence, and
//! nothing here should grow a database dependency before then.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use super::config::ProbeSettings;
use super::now_ms;
use super::probe::{Network, ProbeOutcome};
use crate::models::overview::{ProbeFailureBasis, ProbeTargetSummary};

/// How far back the chart and the health verdict look.
///
/// Five minutes at the shipped interval is a few hundred points, which is
/// enough to see a pattern and few enough to stay cheap to serialise on every
/// poll. Feature 5 moves the floor of this window to the database instead of
/// this buffer.
pub const SERIES_WINDOW_MS: u64 = 5 * 60 * 1_000;

/// Hard cap on buffered points, so a configuration probing far faster than the
/// default cannot grow the series without limit.
pub const SERIES_MAX_POINTS: usize = 1_000;

/// The oldest capture time still inside the window.
pub fn window_cutoff(now: i64) -> i64 {
    now.saturating_sub(SERIES_WINDOW_MS as i64)
}

/// A latency reading and how much the app can still vouch for it.
///
/// `Fresh` and `Stale` carry the same pair, and the difference is entirely in
/// the name. A caller that renders them identically has thrown the distinction
/// away, which is why they are separate variants rather than a flag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FreshLatency {
    /// The newest outcome in the window succeeded.
    Fresh { latency: f64, measured_at: i64 },
    /// The newest outcome failed, and this is an earlier success being held. Its
    /// `measured_at` is when that number was actually taken, never the time of
    /// the failure that made it stale.
    Stale { latency: f64, measured_at: i64 },
    /// No successful reading inside the window.
    None,
}

/// Shared monitoring state, read by the overview command on every request.
#[derive(Default)]
pub struct MonitorState {
    latest: Mutex<HashMap<i64, ProbeOutcome>>,
    /// Recent outcomes in capture order, across all targets. The chart reads a
    /// slice of this; health reads it to decide connected or degraded.
    series: Mutex<VecDeque<ProbeOutcome>>,
    /// The target the overview and chart report on. Zero means not yet set.
    primary_target: AtomicI64,
    /// The local router target and the address to show beside its reading. Set
    /// by the loop at startup. Kept apart from the primary target so the router
    /// reading is reported beside the internet reading instead of overwriting
    /// it. Zero means no router target is configured.
    router_target: AtomicI64,
    router_host: Mutex<Option<String>>,
    /// The whole target the headline reading comes from, so the interface can
    /// name what the latency is a measurement of. Set by the loop at startup.
    primary_target_detail: Mutex<Option<ProbeTargetSummary>>,
    /// When the loop last saw a probe succeed. Separate from the latest outcome
    /// on purpose: a failure must not move "last probe" forward, or a broken
    /// connection would look freshly checked.
    last_success_at: AtomicI64,
    active: AtomicBool,
    /// Whether the loop is paused.
    ///
    /// A pause and a stop are different things. A stop ends the loop; a pause
    /// keeps it alive so it can carry on, and the readings taken before the
    /// pause stay stored. The flag lives here so the command that sets it and
    /// the loop that obeys it do not need to know about each other.
    paused: AtomicBool,
}

impl MonitorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Names the target the single overview and chart describe. Set by the loop
    /// at startup so the commands do not need the configuration themselves.
    pub fn set_primary_target(&self, target_id: i64) {
        self.primary_target.store(target_id, Ordering::Relaxed);
    }

    /// Records what the headline target actually is, so the interface can name
    /// its subject instead of showing a latency with nothing attached.
    pub fn set_primary_target_detail(&self, target: &ProbeTargetSummary) {
        *self
            .primary_target_detail
            .lock()
            .expect("monitor state poisoned") = Some(target.clone());
    }

    /// The headline target, or `None` before the loop has named one.
    pub fn primary_target_detail(&self) -> Option<ProbeTargetSummary> {
        self.primary_target_detail
            .lock()
            .expect("monitor state poisoned")
            .clone()
    }

    /// Falls back to the target most recently probed, so state that was written
    /// without a named primary still reports something real rather than nothing.
    pub fn primary_target_id(&self) -> Option<i64> {
        match self.primary_target.load(Ordering::Relaxed) {
            0 => self
                .series
                .lock()
                .expect("monitor state poisoned")
                .back()
                .map(|outcome| outcome.target_id),
            id => Some(id),
        }
    }

    /// Names the local router target and the address to show beside its reading.
    pub fn set_router_target(&self, target_id: i64, host: &str) {
        self.router_target.store(target_id, Ordering::Relaxed);
        *self.router_host.lock().expect("monitor state poisoned") = Some(host.to_string());
    }

    /// The configured router address, or `None` when no router target exists.
    /// Reported so the UI can name what it probed rather than showing a bare
    /// number with no subject.
    pub fn router_host(&self) -> Option<String> {
        self.router_host
            .lock()
            .expect("monitor state poisoned")
            .clone()
    }

    /// Pauses or resumes the probe loop.
    ///
    /// The loop stays alive either way. A resume is therefore immediate, and
    /// the samples taken before the pause are still in the history.
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Whether the probe loop is paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    fn router_target_id(&self) -> Option<i64> {
        match self.router_target.load(Ordering::Relaxed) {
            0 => None,
            id => Some(id),
        }
    }

    /// The newest router probe still inside the window.
    ///
    /// `None` means no router probe has completed inside the window, which is
    /// not the same as a router that was probed and did not answer. The two are
    /// kept apart here so the caller cannot accidentally show "unreachable" for
    /// a router that was never checked.
    pub fn router_outcome(&self) -> Option<ProbeOutcome> {
        self.router_outcome_at(now_ms())
    }

    /// The windowed router read, with the clock supplied for tests.
    pub fn router_outcome_at(&self, now: i64) -> Option<ProbeOutcome> {
        let router = self.router_target_id()?;
        let outcome = self.outcome(router)?;
        (outcome.captured_at >= window_cutoff(now)).then_some(outcome)
    }

    pub fn record(&self, outcome: ProbeOutcome) {
        if outcome.success {
            self.last_success_at
                .store(outcome.captured_at, Ordering::Relaxed);
        }

        let captured_at = outcome.captured_at;
        self.latest
            .lock()
            .expect("monitor state poisoned")
            .insert(outcome.target_id, outcome.clone());

        let mut series = self.series.lock().expect("monitor state poisoned");
        series.push_back(outcome);

        // Points older than the window are no longer plotted or judged, so they
        // are dropped rather than kept until the cap trims them. This is a
        // memory bound only; correctness comes from the read path.
        let cutoff = window_cutoff(captured_at);
        while series.front().is_some_and(|old| old.captured_at < cutoff) {
            series.pop_front();
        }
        while series.len() > SERIES_MAX_POINTS {
            series.pop_front();
        }
    }

    pub fn outcome(&self, target_id: i64) -> Option<ProbeOutcome> {
        self.latest
            .lock()
            .expect("monitor state poisoned")
            .get(&target_id)
            .cloned()
    }

    /// Recent outcomes for the primary target, oldest first.
    ///
    /// The window is applied here, against the clock, rather than being left to
    /// the eviction that happens when a sample is recorded. A loop that has
    /// stopped records nothing and would therefore evict nothing, so a read
    /// that trusted eviction alone would keep serving readings that are hours
    /// old as though they were current. Correctness belongs on the read path;
    /// eviction on write is only a memory bound.
    pub fn recent_for_primary(&self) -> Vec<ProbeOutcome> {
        self.recent_for_primary_at(now_ms())
    }

    /// The windowed read, with the clock supplied.
    ///
    /// Separate from [`Self::recent_for_primary`] so a test can place samples
    /// in the past and assert on the window without waiting for it to elapse.
    pub fn recent_for_primary_at(&self, now: i64) -> Vec<ProbeOutcome> {
        match self.primary_target_id() {
            Some(primary) => self.recent_for_at(primary, now),
            None => Vec::new(),
        }
    }

    /// Recent outcomes for one target, oldest first, windowed against the clock.
    pub fn recent_for_at(&self, target_id: i64, now: i64) -> Vec<ProbeOutcome> {
        let cutoff = window_cutoff(now);

        self.series
            .lock()
            .expect("monitor state poisoned")
            .iter()
            .filter(|outcome| outcome.target_id == target_id && outcome.captured_at >= cutoff)
            .cloned()
            .collect()
    }

    /// The newest outcome for the primary target, if it is still inside the
    /// window.
    ///
    /// Windowed for the same reason the series read is: a loop that stopped
    /// keeps its last outcome in the map forever, and reporting it would leave
    /// the dashboard showing a reading from whenever monitoring happened to die.
    pub fn latest_outcome(&self) -> Option<ProbeOutcome> {
        self.latest_outcome_at(now_ms())
    }

    /// The newest windowed outcome, with the clock supplied for tests.
    pub fn latest_outcome_at(&self, now: i64) -> Option<ProbeOutcome> {
        let primary = self.primary_target_id()?;
        let outcome = self.outcome(primary)?;
        (outcome.captured_at >= window_cutoff(now)).then_some(outcome)
    }

    /// The most recent successful capture, or `None` if nothing has succeeded.
    pub fn last_success_at(&self) -> Option<i64> {
        match self.last_success_at.load(Ordering::Relaxed) {
            0 => None,
            at => Some(at),
        }
    }

    /// The newest windowed outcome's latency, and only if that outcome was a
    /// success. A failure reports no latency at all rather than an old one,
    /// because nothing yet says how old it would be.
    pub fn latest_latency(&self) -> Option<(f64, i64)> {
        let outcome = self.latest_outcome()?;
        if !outcome.success {
            return None;
        }
        Some((outcome.latency_ms.unwrap_or_default(), outcome.captured_at))
    }

    /// How many probes in the window did not connect, out of how many were made.
    ///
    /// `None` when the window holds no attempts at all. That case is not a
    /// percentage of zero: zero out of nothing is not a measurement, and
    /// reporting it as `0.0` would be the most reassuring possible lie.
    pub fn failure_summary(&self) -> Option<ProbeFailureBasis> {
        let recent = self.recent_for_primary();
        if recent.is_empty() {
            return None;
        }

        Some(ProbeFailureBasis {
            failed: recent.iter().filter(|outcome| !outcome.success).count() as u32,
            total: recent.len() as u32,
        })
    }

    /// The newest latency inside the window, and whether it is still current.
    ///
    /// The distinction is the whole point. `Fresh` means the newest outcome in
    /// the window succeeded, so the number describes this moment. `Stale` means
    /// the newest outcome failed but an earlier success is still in the window,
    /// so the number is real but is holding, and the caller must say how old it
    /// is. `None` means there is no successful reading in the window at all,
    /// which is different from a reading that has simply gone out of date.
    ///
    /// Holding the number is what 2b could not do. It blanked latency on every
    /// failure, which made a brief blip look identical to an outage.
    pub fn freshest_latency(&self) -> FreshLatency {
        let now = now_ms();
        let recent = self.recent_for_primary_at(now);

        let newest = match recent.last() {
            Some(outcome) => outcome,
            None => return FreshLatency::None,
        };

        let newest_succeeded = newest.success && newest.latency_ms.is_some();

        if newest_succeeded {
            return FreshLatency::Fresh {
                latency: newest.latency_ms.unwrap_or_default(),
                measured_at: newest.captured_at,
            };
        }

        // The newest attempt failed. An earlier success still inside the window
        // is worth holding, provided the caller reports its age.
        match recent
            .iter()
            .rev()
            .find(|outcome| outcome.success && outcome.latency_ms.is_some())
        {
            Some(held) => FreshLatency::Stale {
                latency: held.latency_ms.unwrap_or_default(),
                measured_at: held.captured_at,
            },
            None => FreshLatency::None,
        }
    }

    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn is_empty(&self) -> bool {
        self.latest
            .lock()
            .expect("monitor state poisoned")
            .is_empty()
    }
}

/// One tick: probe every enabled target once and store what came back.
///
/// Separated from the scheduling in `loop` so tests can drive a tick directly
/// instead of waiting on a real interval.
pub async fn probe_all_once<N>(network: &N, settings: &ProbeSettings, state: &MonitorState)
where
    N: Network + Sync + ?Sized,
{
    for target in settings.enabled_targets() {
        state.record(super::probe::probe_once(network, target).await);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::config::{DEFAULT_TIMEOUT_MS, MIN_INTERVAL_MS, default_settings};
    use crate::monitoring::probe::ProbeErrorKind;
    use crate::monitoring::testing::{Attempt, FakeNetwork, loopback};

    #[tokio::test]
    async fn a_successful_probe_is_stored_with_its_latency_and_time() {
        let state = MonitorState::new();
        let network = FakeNetwork::resolving(vec![loopback(443)], vec![Attempt::ok()]);

        probe_all_once(&network, &default_settings(), &state).await;

        let stored = state.outcome(1).expect("an outcome must be stored");
        assert!(stored.success);
        assert!(stored.latency_ms.is_some());
        assert!(stored.captured_at > 0);
        assert!(state.last_success_at().is_some());
    }

    #[tokio::test]
    async fn a_failed_probe_is_stored_without_a_latency() {
        let state = MonitorState::new();
        let network = FakeNetwork::resolving(
            vec![loopback(443)],
            vec![Attempt::failing(ProbeErrorKind::Unreachable)],
        );

        probe_all_once(&network, &default_settings(), &state).await;

        let stored = state.outcome(1).expect("a failure must be stored too");
        assert!(!stored.success);
        assert_eq!(stored.latency_ms, None, "a failure must not carry a number");
        assert_eq!(stored.error_kind, Some(ProbeErrorKind::Unreachable));
        assert!(state.latest_latency().is_none());
        assert!(
            state.last_success_at().is_none(),
            "a failure must not move the last successful probe"
        );
    }

    #[tokio::test]
    async fn a_failure_after_a_success_reports_no_latency_but_keeps_the_last_success() {
        let state = MonitorState::new();

        let good = FakeNetwork::resolving(vec![loopback(443)], vec![Attempt::ok()]);
        probe_all_once(&good, &default_settings(), &state).await;
        let first_success = state.last_success_at().expect("a success was recorded");

        let bad = FakeNetwork::resolving(
            vec![loopback(443)],
            vec![Attempt::failing(ProbeErrorKind::Timeout)],
        );
        probe_all_once(&bad, &default_settings(), &state).await;

        assert!(
            state.latest_latency().is_none(),
            "the newest outcome failed, so no latency is reported"
        );
        assert_eq!(
            state.last_success_at(),
            Some(first_success),
            "the last successful probe is unchanged by a later failure"
        );
    }

    #[tokio::test]
    async fn disabled_targets_are_never_probed() {
        let state = MonitorState::new();
        let network = FakeNetwork::resolving(vec![loopback(443)], vec![Attempt::ok()]);

        let mut settings = default_settings();
        settings.targets[0].enabled = false;

        probe_all_once(&network, &settings, &state).await;

        assert_eq!(
            network.attempt_count(),
            1,
            "only the enabled target is probed"
        );
        assert!(
            state.outcome(1).is_none(),
            "the disabled target was skipped"
        );
        assert!(state.outcome(2).is_some());
    }

    #[test]
    fn an_empty_store_reports_nothing_rather_than_zero() {
        let state = MonitorState::new();

        assert!(state.is_empty());
        assert!(state.last_success_at().is_none());
        assert!(state.latest_latency().is_none());
        assert!(!state.is_active());
    }

    #[test]
    fn active_flag_round_trips() {
        let state = MonitorState::new();

        state.set_active(true);
        assert!(state.is_active());

        state.set_active(false);
        assert!(!state.is_active());
    }

    /// The defect this step exists to fix. Nothing is recorded after the sample
    /// ages out, exactly as happens when the probe loop stops, and eviction on
    /// write therefore never runs. A read that trusted eviction alone would
    /// keep serving the old sample.
    #[test]
    fn a_sample_outside_the_window_is_excluded_even_when_nothing_new_was_recorded() {
        let state = MonitorState::new();
        let aged = now_ms() - SERIES_WINDOW_MS as i64 - 1_000;

        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: aged,
            success: true,
            latency_ms: Some(12.0),
            error_kind: None,
        });

        assert!(
            state.series.lock().expect("lock").len() == 1,
            "the sample is still buffered, because eviction only runs on write"
        );

        assert!(
            state.recent_for_primary().is_empty(),
            "a sample past the window must not be reported"
        );
        assert!(
            state.latest_outcome().is_none(),
            "the newest outcome must not be reported once it ages out"
        );
        assert!(
            state.latest_latency().is_none(),
            "a stale latency must never be presented as a current reading"
        );
    }

    #[test]
    fn a_sample_inside_the_window_is_still_reported() {
        let state = MonitorState::new();
        let at = now_ms() - SERIES_WINDOW_MS as i64 + 1_000;

        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: at,
            success: true,
            latency_ms: Some(12.0),
            error_kind: None,
        });

        assert_eq!(state.recent_for_primary().len(), 1);
        assert_eq!(state.latest_outcome().map(|o| o.captured_at), Some(at));
        assert_eq!(state.latest_latency().map(|(_, seen)| seen), Some(at));
    }

    #[test]
    fn recording_a_new_sample_keeps_it_readable() {
        let state = MonitorState::new();

        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: now_ms(),
            success: true,
            latency_ms: Some(9.0),
            error_kind: None,
        });
        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: now_ms(),
            success: false,
            latency_ms: None,
            error_kind: Some(ProbeErrorKind::Timeout),
        });

        assert_eq!(state.recent_for_primary().len(), 2);
        assert!(
            state.latest_latency().is_none(),
            "the newest outcome failed, so no latency is reported"
        );
    }

    #[test]
    fn the_window_boundary_keeps_the_oldest_inclusive_sample() {
        let state = MonitorState::new();
        let now = now_ms();
        let exactly_at_cutoff = now - SERIES_WINDOW_MS as i64;

        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: exactly_at_cutoff,
            success: true,
            latency_ms: Some(5.0),
            error_kind: None,
        });

        assert_eq!(
            state.recent_for_primary_at(now).len(),
            1,
            "a sample exactly on the boundary is still inside the window"
        );
        assert!(
            state.recent_for_primary_at(now + 1).is_empty(),
            "one millisecond later it is outside"
        );
    }

    // ---------------------------------------------------------------------
    // Failure counting.
    // ---------------------------------------------------------------------

    fn sample_state(results: &[bool]) -> MonitorState {
        let state = MonitorState::new();
        let base = now_ms() - (results.len() as i64 * 2_000);

        for (index, success) in results.iter().enumerate() {
            state.record(ProbeOutcome {
                target_id: 1,
                captured_at: base + (index as i64 * 2_000),
                success: *success,
                latency_ms: success.then_some(12.0),
                error_kind: (!success).then_some(ProbeErrorKind::Timeout),
            });
        }

        state
    }

    #[test]
    fn no_attempts_reports_no_summary_rather_than_zero_percent() {
        let state = MonitorState::new();

        assert_eq!(
            state.failure_summary(),
            None,
            "zero out of no attempts is not a measurement"
        );
    }

    #[test]
    fn every_attempt_succeeding_is_a_genuine_zero() {
        let state = sample_state(&[true, true, true, true]);

        let basis = state
            .failure_summary()
            .expect("a window with attempts must report one");

        assert_eq!(basis.failed, 0);
        assert_eq!(basis.total, 4);
        assert_eq!(
            basis.percent(),
            0.0,
            "zero failures out of four is real information, not an absence"
        );
    }

    #[test]
    fn every_attempt_failing_is_one_hundred_percent() {
        let basis = sample_state(&[false, false])
            .failure_summary()
            .expect("a window with attempts must report one");

        assert_eq!(basis.failed, 2);
        assert_eq!(basis.total, 2);
        assert_eq!(basis.percent(), 100.0);
    }

    #[test]
    fn a_mixed_window_reports_the_right_fraction() {
        let basis = sample_state(&[true, true, true, true, false])
            .failure_summary()
            .expect("a window with attempts must report one");

        assert_eq!(basis.failed, 1);
        assert_eq!(basis.total, 5);
        assert_eq!(basis.percent(), 20.0);
    }

    #[test]
    fn one_failure_in_three_is_not_rounded_away() {
        let basis = sample_state(&[true, false, true])
            .failure_summary()
            .expect("a window with attempts must report one");

        // Compared with a tolerance rather than against a float literal: the
        // exact last digit is not a property this app depends on, but the fact
        // that the fraction survives at all is. Rounding to a whole percent
        // here would turn one failure into 33 and, worse, a single failure in
        // thirty into 3.
        assert!(
            (basis.percent() - (100.0 / 3.0)).abs() < 1e-9,
            "got {}",
            basis.percent()
        );
    }

    #[test]
    fn attempts_outside_the_window_are_excluded_from_both_counts() {
        let state = MonitorState::new();
        let aged = now_ms() - SERIES_WINDOW_MS as i64 - 2_000;

        // Four failures, all long since outside the window.
        for step in 0..4 {
            state.record(ProbeOutcome {
                target_id: 1,
                captured_at: aged + step,
                success: false,
                latency_ms: None,
                error_kind: Some(ProbeErrorKind::Timeout),
            });
        }
        // Then one fresh success, which is the only attempt that counts.
        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: now_ms(),
            success: true,
            latency_ms: Some(11.0),
            error_kind: None,
        });

        let basis = state
            .failure_summary()
            .expect("the fresh attempt must still be counted");

        assert_eq!(
            (basis.failed, basis.total),
            (0, 1),
            "an old outage must not keep reporting as a current failure rate"
        );
        assert_eq!(basis.percent(), 0.0);
    }

    #[test]
    fn a_summary_with_no_attempts_never_divides_by_zero() {
        let basis = ProbeFailureBasis {
            failed: 0,
            total: 0,
        };

        assert_eq!(basis.percent(), 0.0);
    }

    #[test]
    fn the_summary_counts_only_the_primary_target() {
        let state = sample_state(&[false, false]);
        let base = now_ms();

        // Named explicitly. Left unnamed, the store falls back to whichever
        // target was probed most recently, which here would be target 2, and the
        // test would measure the fallback rather than the thing it means to.
        state.set_primary_target(1);

        // A second target failing repeatedly must not colour the primary's rate.
        for step in 0..10 {
            state.record(ProbeOutcome {
                target_id: 2,
                captured_at: base + step,
                success: false,
                latency_ms: None,
                error_kind: Some(ProbeErrorKind::Timeout),
            });
        }

        let basis = state.failure_summary().expect("the primary has attempts");

        assert_eq!(
            (basis.failed, basis.total),
            (2, 2),
            "another target's failures are not this figure's business until 2d defines aggregation"
        );
    }

    /// The fallback 2b introduced, pinned here because this feature depends on
    /// it: with no primary named, the store reports the target it probed most
    /// recently rather than reporting nothing.
    #[test]
    fn without_a_named_primary_the_most_recently_probed_target_is_reported() {
        let state = MonitorState::new();
        let base = now_ms();

        state.record(ProbeOutcome {
            target_id: 7,
            captured_at: base,
            success: true,
            latency_ms: Some(10.0),
            error_kind: None,
        });

        assert_eq!(state.primary_target_id(), Some(7));
    }

    // ---------------------------------------------------------------------
    // The freshness rule.
    // ---------------------------------------------------------------------

    fn latency_state(results: &[bool]) -> FreshLatency {
        let state = sample_state(results);
        state.freshest_latency()
    }

    #[test]
    fn a_newest_success_is_fresh() {
        assert_eq!(
            latency_state(&[true, true]),
            FreshLatency::Fresh {
                latency: 12.0,
                measured_at: now_ms() - 2_000,
            }
        );
    }

    #[test]
    fn a_newest_failure_holds_the_most_recent_earlier_success() {
        let base = now_ms();
        let state = MonitorState::new();

        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: base - 8_000,
            success: true,
            latency_ms: Some(30.0),
            error_kind: None,
        });
        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: base - 4_000,
            success: true,
            latency_ms: Some(20.0),
            error_kind: None,
        });
        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: base,
            success: false,
            latency_ms: None,
            error_kind: Some(ProbeErrorKind::Timeout),
        });

        assert_eq!(
            state.freshest_latency(),
            FreshLatency::Stale {
                latency: 20.0,
                measured_at: base - 4_000,
            },
            "the newest success is held, not the oldest one in the window"
        );
    }

    #[test]
    fn a_recovery_is_fresh_again() {
        let now = now_ms();
        let recovered = latency_state(&[false, false, true]);

        match recovered {
            FreshLatency::Fresh {
                latency,
                measured_at,
            } => {
                assert_eq!(latency, 12.0);
                // The helper spaces samples backwards from now, so the newest is
                // one interval old rather than this instant. What matters is
                // that it is the newest of the three, not that it is zero
                // seconds old.
                assert!(
                    measured_at > now - 10_000,
                    "a recovered reading must be the newest sample, got {measured_at}"
                );
            }
            other => panic!("expected a fresh reading after a recovery, got {other:?}"),
        }
    }

    #[test]
    fn no_success_in_the_window_reports_none() {
        assert_eq!(latency_state(&[false, false]), FreshLatency::None);
    }

    #[test]
    fn an_empty_window_reports_none() {
        assert_eq!(MonitorState::new().freshest_latency(), FreshLatency::None);
    }

    #[test]
    fn a_held_reading_is_dropped_when_it_leaves_the_window() {
        let now = now_ms();
        let state = MonitorState::new();

        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: now - SERIES_WINDOW_MS as i64 - 1_000,
            success: true,
            latency_ms: Some(40.0),
            error_kind: None,
        });
        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: now - 2_000,
            success: false,
            latency_ms: None,
            error_kind: Some(ProbeErrorKind::Timeout),
        });

        assert_eq!(
            state.freshest_latency(),
            FreshLatency::None,
            "an aged-out reading must not be held, because holding it would claim a value the window no longer covers"
        );
    }

    /// A success that carries no latency is not a usable reading, so it must not
    /// be treated as one. `ProbeOutcome` allows the combination and the type
    /// system will not stop it being constructed.
    #[test]
    fn a_success_without_a_latency_is_not_held_as_a_reading() {
        let now = now_ms();
        let state = MonitorState::new();

        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: now - 4_000,
            success: true,
            latency_ms: None,
            error_kind: None,
        });
        state.record(ProbeOutcome {
            target_id: 1,
            captured_at: now,
            success: false,
            latency_ms: None,
            error_kind: Some(ProbeErrorKind::Other),
        });

        assert_eq!(
            state.freshest_latency(),
            FreshLatency::None,
            "a success with no number cannot be rendered as 0 ms"
        );
    }

    /// A scripted double proves the bookkeeping. It cannot prove that a TCP
    /// connect to a real socket produces a real latency, which is the one thing
    /// 2b actually claims. This binds a real listener on the loopback
    /// interface and measures against it, so a regression in the socket path
    /// fails here rather than on a user's screen.
    #[tokio::test]
    async fn a_real_listener_is_measured_by_the_real_network() {
        use std::net::TcpListener;

        use crate::monitoring::probe::TcpNetwork;

        let listener = TcpListener::bind(loopback(0)).expect("loopback listener must bind");
        let port = listener.local_addr().expect("bound address").port();

        // Accepting is not required for a connect to succeed on macOS: the
        // backlog completes the handshake. Holding the listener keeps it alive.
        let state = MonitorState::new();

        let mut settings = default_settings();
        settings.targets[0].host = "127.0.0.1".to_string();
        settings.targets[0].port = port;
        settings.targets[0].interval_ms = MIN_INTERVAL_MS;
        settings.targets[0].timeout_ms = DEFAULT_TIMEOUT_MS;

        // Only the loopback target is probed.
        //
        // The default settings also carry the local router, and a probe of the
        // router would either need the device to be on the network or would
        // fail, and a failure recorded last becomes the newest outcome. That
        // would make this test depend on whether a particular router happened
        // to be reachable. A test of the socket path must not depend on
        // anything outside the machine.
        settings.targets.truncate(1);

        probe_all_once(&TcpNetwork, &settings, &state).await;

        let stored = state.outcome(1).expect("an outcome must be stored");
        assert!(
            stored.success,
            "a real connect to a listening port must succeed, got {:?}",
            stored.error_kind
        );

        let (latency, _) = state
            .latest_latency()
            .expect("a real measurement must produce a latency");
        assert!(
            latency > 0.0,
            "a measured latency must be positive, got {latency}"
        );
        // Loopback connect completes in well under a second. A generous ceiling
        // still catches a broken clock without being timing-sensitive.
        assert!(
            latency < 1_000.0,
            "a loopback connect should be fast, got {latency}"
        );

        drop(listener);
    }

    /// The other half of the same claim: a port nothing is listening on must
    /// fail rather than report a latency anyway.
    #[tokio::test]
    async fn a_closed_real_port_fails_without_inventing_a_latency() {
        use std::net::TcpListener;

        use crate::monitoring::probe::TcpNetwork;

        // Bind then drop, so the port is almost certainly free. This is the one
        // test allowed to rely on the environment; if the port is somehow taken
        // it fails to connect and the assertion still holds for the wrong
        // reason, which the error kind check below catches.
        let port = {
            let listener = TcpListener::bind(loopback(0)).expect("loopback listener must bind");
            listener.local_addr().expect("bound address").port()
        };

        let state = MonitorState::new();
        // Named explicitly. The overview falls back to the most recently probed
        // target only when no primary was set, and with a router target in the
        // defaults that fallback would pick up the router's success here.
        state.set_primary_target(1);

        let mut settings = default_settings();
        settings.targets[0].host = "127.0.0.1".to_string();
        settings.targets[0].port = port;
        settings.targets[0].timeout_ms = DEFAULT_TIMEOUT_MS;

        probe_all_once(&TcpNetwork, &settings, &state).await;

        let stored = state.outcome(1).expect("a failure must be stored too");
        assert!(!stored.success, "connecting to nothing must not succeed");
        assert_eq!(
            stored.latency_ms, None,
            "a failed connect must not carry a number"
        );
        assert!(
            state.latest_latency().is_none(),
            "no latency may be reported for a closed port"
        );
    }
}
