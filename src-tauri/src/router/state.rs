//! The router reading held between polls.
//!
//! A router read is a handful of HTTPS requests to a small embedded device, so
//! it runs on its own cadence, slower than the probe loop and off the async
//! runtime entirely. This cell is where the result is parked so the commands can
//! serve it without performing network work on the IPC path.
//!
//! The state is deliberately three states, not two: never attempted, succeeded,
//! and failed. Collapsing "never attempted" into "failed" would show a failure
//! before the first request had a chance to run, which is a claim the app has not
//! earned.

use std::sync::Mutex;

use super::traffic::rate_between;
use super::{RouterCredentials, RouterError, RouterSnapshot};
use crate::models::overview::TrafficRate;

/// What is currently known about the router.
#[derive(Default)]
pub struct RouterState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// The credentials, or `None` when the router is not configured.
    credentials: Option<RouterCredentials>,
    /// The most recent successful snapshot, kept when a later read fails so a
    /// transient failure does not blank the screen.
    snapshot: Option<RouterSnapshot>,
    /// The most recent failure, cleared by the next success.
    error: Option<RouterError>,
    /// Whether a read has been attempted at all.
    attempted: bool,
    /// The traffic rate derived from the last two counter readings.
    ///
    /// Held here rather than inside a snapshot because a rate needs two
    /// readings, and one snapshot is one reading.
    traffic: Option<TrafficRate>,
}

/// A read-only view of the router state, as the commands serve it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterReport {
    pub snapshot: Option<RouterSnapshot>,
    pub error: Option<RouterError>,
    /// False when no credentials are configured, so the interface can explain
    /// the absence rather than reporting a failure that never happened.
    pub configured: bool,
    /// Whether any read has been attempted, so the interface can tell "waiting
    /// for the first read" apart from "the last read failed".
    pub attempted: bool,
    /// The throughput rate, when two counter readings have produced one.
    pub traffic: Option<TrafficRate>,
}

impl RouterState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the credentials. Passing `None` disables router reads entirely.
    pub fn set_credentials(&self, credentials: Option<RouterCredentials>) {
        let mut inner = self.lock();
        inner.credentials = credentials;
        // A credential change invalidates whatever the previous account read, so
        // the held snapshot is dropped rather than served under the new name.
        inner.snapshot = None;
        inner.error = None;
        inner.attempted = false;
        // A rate across a credential change would compare two different
        // sessions' counters, so it goes with the snapshot.
        inner.traffic = None;
    }

    pub fn credentials(&self) -> Option<RouterCredentials> {
        self.lock().credentials.clone()
    }

    pub fn snapshot(&self) -> Option<RouterSnapshot> {
        self.lock().snapshot.clone()
    }

    pub fn error(&self) -> Option<RouterError> {
        self.lock().error.clone()
    }

    /// Records a successful read, replacing any held failure.
    ///
    /// The traffic rate is derived here, because this is the moment a second
    /// counter reading exists. The rate is taken from the snapshot that was
    /// held before this one and the one being stored now, so it describes the
    /// interval between two real reads.
    pub fn record_success(&self, snapshot: RouterSnapshot) {
        let mut inner = self.lock();

        // A credential change or a fresh session restarts the counters, so a
        // rate across that boundary would be meaningless. The counters are only
        // compared when both readings came from the same session.
        inner.traffic = match (
            inner.snapshot.as_ref().and_then(|s| s.counters),
            snapshot.counters,
        ) {
            (Some(previous), Some(current)) => Some(rate_between(&previous, &current)),
            // A read with no counters, after a read that had them, means the
            // statistics page stopped answering. The rate is dropped rather
            // than held, because it can no longer be shown to be current.
            _ => None,
        };

        inner.snapshot = Some(snapshot);
        inner.error = None;
        inner.attempted = true;
    }

    /// Records a failed read, keeping the last good snapshot if there was one.
    ///
    /// The snapshot is kept and reported alongside the error rather than being
    /// discarded: a router that answered a minute ago still has a real address,
    /// and throwing that away because the newest attempt timed out would make a
    /// brief blip look like a total loss of information.
    pub fn record_failure(&self, error: RouterError) {
        let mut inner = self.lock();
        inner.error = Some(error);
        inner.attempted = true;
    }

    /// Whether router reads are possible at all.
    pub fn is_configured(&self) -> bool {
        self.lock().credentials.is_some()
    }

    /// The whole state as the commands serve it.
    pub fn report(&self) -> RouterReport {
        let inner = self.lock();
        RouterReport {
            snapshot: inner.snapshot.clone(),
            error: inner.error.clone(),
            configured: inner.credentials.is_some(),
            attempted: inner.attempted,
            traffic: inner.traffic.clone(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("router state poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::overview::{RouterHealth, RouterNetwork, WanConnection, WanCounters};
    use crate::router::{RouterAuthSummary, RouterErrorKind, RouterSourceStatus};

    fn snapshot(read_at: i64) -> RouterSnapshot {
        RouterSnapshot {
            read_at,
            auth: RouterAuthSummary {
                host: "192.168.100.1".to_string(),
                port: 80,
                username: Some("root".to_string()),
                has_password: true,
            },
            wan: WanConnection::unavailable(),
            health: RouterHealth::unavailable(),
            counters: None,
            network: RouterNetwork::default(),
            sources: vec![RouterSourceStatus::ok("WAN connection", "/wan")],
        }
    }

    fn credentials() -> RouterCredentials {
        RouterCredentials {
            host: "192.168.100.1".to_string(),
            port: 80,
            username: "root".to_string(),
            password: "secret".to_string(),
        }
    }

    #[test]
    fn a_fresh_state_is_unconfigured_and_never_attempted() {
        let state = RouterState::new();
        let report = state.report();

        assert!(!report.configured);
        assert!(!report.attempted);
        assert!(report.snapshot.is_none());
        assert!(
            report.error.is_none(),
            "a router that has not been tried has not failed"
        );
    }

    #[test]
    fn configuring_credentials_marks_the_router_configured() {
        let state = RouterState::new();
        state.set_credentials(Some(credentials()));

        let report = state.report();
        assert!(report.configured);
        assert!(
            !report.attempted,
            "configured is not the same as attempted, and must not imply a result"
        );
    }

    #[test]
    fn a_successful_read_is_reported_without_an_error() {
        let state = RouterState::new();
        state.set_credentials(Some(credentials()));
        state.record_success(snapshot(1_700_000_000_000));

        let report = state.report();
        assert!(report.attempted);
        assert_eq!(
            report.snapshot.map(|snapshot| snapshot.read_at),
            Some(1_700_000_000_000)
        );
        assert!(report.error.is_none());
    }

    #[test]
    fn a_failure_keeps_the_last_good_snapshot() {
        let state = RouterState::new();
        state.set_credentials(Some(credentials()));
        state.record_success(snapshot(1_700_000_000_000));
        state.record_failure(RouterError::new(RouterErrorKind::Timeout, "did not answer"));

        let report = state.report();
        assert_eq!(
            report.snapshot.map(|snapshot| snapshot.read_at),
            Some(1_700_000_000_000),
            "a real address read a minute ago is still worth showing"
        );
        assert_eq!(
            report.error.map(|error| error.kind),
            Some(RouterErrorKind::Timeout)
        );
    }

    #[test]
    fn a_success_clears_a_held_failure() {
        let state = RouterState::new();
        state.set_credentials(Some(credentials()));
        state.record_failure(RouterError::new(RouterErrorKind::Unreachable, "down"));
        state.record_success(snapshot(1));

        assert!(
            state.report().error.is_none(),
            "a recovered router must not keep reporting the old failure"
        );
    }

    #[test]
    fn changing_credentials_drops_the_previous_read() {
        let state = RouterState::new();
        state.set_credentials(Some(credentials()));
        state.record_success(snapshot(1));
        state.record_failure(RouterError::new(RouterErrorKind::AuthRejected, "wrong"));

        state.set_credentials(Some(RouterCredentials {
            username: "other".to_string(),
            ..credentials()
        }));

        let report = state.report();
        assert!(
            report.snapshot.is_none(),
            "data read under the old account must not be shown under the new one"
        );
        assert!(report.error.is_none());
        assert!(!report.attempted);
    }

    #[test]
    fn a_report_never_carries_the_password() {
        let state = RouterState::new();
        state.set_credentials(Some(RouterCredentials {
            password: "must-not-appear".to_string(),
            ..credentials()
        }));
        state.record_success(snapshot(1));

        let json = serde_json::to_string(&state.report()).expect("report must serialise");
        assert!(!json.contains("must-not-appear"));
    }

    // ---------------------------------------------------------------------
    // The traffic rate.
    // ---------------------------------------------------------------------

    fn counters(at: i64, sent: u64, received: u64) -> WanCounters {
        WanCounters {
            bytes_sent: sent,
            bytes_received: received,
            packets_sent: 0,
            packets_received: 0,
            measured_at: at,
        }
    }

    fn snapshot_with_counters(at: i64, sent: u64, received: u64) -> RouterSnapshot {
        RouterSnapshot {
            counters: Some(counters(at, sent, received)),
            ..snapshot(at)
        }
    }

    /// One reading is not a rate. The app must say it is waiting for a second
    /// one rather than showing a number derived from nothing.
    #[test]
    fn the_first_counter_reading_produces_no_rate() {
        let state = RouterState::new();
        state.record_success(snapshot_with_counters(1_000, 1_000, 2_000));

        let report = state.report();
        assert!(report.traffic.is_none());
        assert!(report.snapshot.is_some_and(|s| s.counters.is_some()));
    }

    #[test]
    fn two_counter_readings_produce_a_rate() {
        let state = RouterState::new();
        state.record_success(snapshot_with_counters(1_000, 1_000, 2_000));
        state.record_success(snapshot_with_counters(31_000, 31_000, 62_000));

        let traffic = state.report().traffic.expect("a rate must be derived");

        assert_eq!(traffic.sent_bytes_per_second, Some(1_000.0));
        assert_eq!(traffic.received_bytes_per_second, Some(2_000.0));
    }

    /// A reset must not become a spike.
    #[test]
    fn a_reset_counter_produces_no_rate() {
        let state = RouterState::new();
        state.record_success(snapshot_with_counters(1_000, 5_000_000, 9_000_000));
        state.record_success(snapshot_with_counters(31_000, 12, 40));

        let traffic = state
            .report()
            .traffic
            .expect("the outcome must be reported");

        assert_eq!(traffic.sent_bytes_per_second, None);
        assert_eq!(traffic.received_bytes_per_second, None);
    }

    /// A read that lost the counters cannot be compared with one that had them.
    #[test]
    fn a_read_without_counters_drops_the_rate() {
        let state = RouterState::new();
        state.record_success(snapshot_with_counters(1_000, 1_000, 2_000));
        state.record_success(snapshot_with_counters(31_000, 31_000, 62_000));
        assert!(state.report().traffic.is_some());

        // The statistics page stops answering.
        state.record_success(snapshot(61_000));

        assert!(
            state.report().traffic.is_none(),
            "a rate can no longer be shown to be current"
        );
    }

    /// A credential change restarts the session, so the counters are not
    /// comparable across it.
    #[test]
    fn a_credential_change_drops_the_rate() {
        let state = RouterState::new();
        state.set_credentials(Some(credentials()));
        state.record_success(snapshot_with_counters(1_000, 1_000, 2_000));
        state.record_success(snapshot_with_counters(31_000, 31_000, 62_000));
        assert!(state.report().traffic.is_some());

        state.set_credentials(Some(RouterCredentials {
            username: "other".to_string(),
            ..credentials()
        }));

        assert!(state.report().traffic.is_none());
    }
}
