//! The repeating probe loop.
//!
//! Scheduling lives here and nowhere else. Everything a test cares about is in
//! `state::probe_all_once`, which can be driven directly instead of waiting on a
//! real interval.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::config::ProbeSettings;
use super::probe::Network;
use super::state::{MonitorState, probe_all_once};
use super::store::Store;
use crate::models::overview::ProbeTargetSummary;

/// Stops the loop when dropped or told to stop.
pub struct MonitorHandle {
    cancel: watch::Sender<bool>,
    /// Option so `stop` can take the handle out even though the type has a
    /// `Drop`: a field cannot be moved out of a type that implements Drop.
    task: Option<JoinHandle<()>>,
    /// Held so the flag can be cleared from `Drop`. Aborting the task stops it
    /// at an await point, which means the task never reaches its own cleanup,
    /// and without this the UI would keep reporting monitoring as running.
    state: Arc<MonitorState>,
}

impl MonitorHandle {
    /// Signals the loop to finish and waits briefly for it, so shutdown does
    /// not leave a probe in flight.
    pub async fn stop(mut self) {
        let _ = self.cancel.send(true);
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
        self.state.set_active(false);
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        // A handle dropped without an explicit stop must still not leave the
        // task running against a state nobody is reading.
        let _ = self.cancel.send(true);
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.state.set_active(false);
    }
}

/// Starts the loop on the configuration's interval.
///
/// `every` is a parameter rather than being read from settings so tests can run
/// the real scheduling logic on a ten-millisecond interval instead of a real
/// five-second one.
///
/// `store` is optional on purpose. History is a convenience, not a requirement:
/// if the database cannot be opened the app still monitors and still shows live
/// readings, it just cannot answer questions about the past. That is a far
/// better failure than refusing to start.
pub fn spawn<N>(
    network: Arc<N>,
    settings: ProbeSettings,
    state: Arc<MonitorState>,
    every: Duration,
    store: Option<Arc<Store>>,
) -> MonitorHandle
where
    N: Network + Sync + Send + 'static,
{
    let (cancel, mut cancelled) = watch::channel(false);
    let tick = Duration::from_millis(settings.default_interval_ms).min(every);

    // Cloned before the task takes ownership, so the handle can still clear the
    // active flag when the task is aborted before its own cleanup runs.
    let handle_state = state.clone();

    // Named before the first probe, so a reading that arrives has somewhere
    // authoritative to be reported from.
    if let Some(primary) = settings.primary_target() {
        state.set_primary_target(primary.id);
        state.set_primary_target_detail(&ProbeTargetSummary {
            name: primary.name.clone(),
            host: primary.host.clone(),
            port: primary.port,
            method: primary.method.label().to_string(),
            interval_ms: primary.interval_ms,
            timeout_ms: primary.timeout_ms,
        });
    }

    // The router is named too, for the same reason and so its reading can be
    // reported beside the internet reading rather than instead of it.
    if let Some(router) = settings.router_target() {
        state.set_router_target(router.id, &router.host);
    }

    let task = tokio::spawn(async move {
        state.set_active(true);
        let mut ticker = tokio::time::interval(tick);
        // A probe that overruns its slot must not queue up a backlog of probes.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    // A paused loop keeps its place and keeps its history. It
                    // simply stops probing, so a paused app does not fill the
                    // history with samples of a connection it was not asked to
                    // watch, and does not create incidents out of a pause.
                    if state.is_paused() {
                        continue;
                    }

                    probe_all_once(network.as_ref(), &settings, &state).await;

                    // History is written from the outcomes the state just
                    // accepted, so what is on disk is exactly what the live
                    // reading is showing rather than a second measurement that
                    // could disagree with it.
                    if let Some(store) = store.as_ref() {
                        persist_last_tick(store, &settings, &state);
                    }
                }
                _ = cancelled.changed() => break,
            }
        }

        state.set_active(false);
    });

    MonitorHandle {
        cancel,
        task: Some(task),
        state: handle_state,
    }
}

/// The interval the app runs on, taken from the compiled-in configuration.
pub fn interval_for(settings: &ProbeSettings) -> Duration {
    Duration::from_millis(settings.default_interval_ms)
}

/// Writes the tick's outcomes to the history database.
///
/// Every target is written, not just the primary, because the incidents screen
/// and the router's part in an outage both need the router's own samples.
///
/// A write failure is swallowed deliberately. Losing a history row is a real
/// loss but a small one, and it must never take down the loop that is producing
/// the live reading the app exists for.
fn persist_last_tick(store: &Store, settings: &ProbeSettings, state: &MonitorState) {
    // Read once per tick rather than per target: the router's reachability is a
    // single fact about this moment, and every incident opened by this tick
    // should agree on it.
    let router_reachable = state.router_outcome().map(|outcome| outcome.success);

    for target in settings.enabled_targets() {
        if let Some(outcome) = state.outcome(target.id) {
            let _ = store.record(&outcome, router_reachable);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::config::ProbeMethod;
    use crate::monitoring::config::{ProbeTargetConfig, TargetKind, default_settings};
    use crate::monitoring::probe::ProbeErrorKind;
    use crate::monitoring::probe::TcpNetwork;
    use crate::monitoring::store::{Incident, Store};
    use crate::monitoring::testing::{Attempt, FakeNetwork};

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since_epoch| since_epoch.as_millis() as i64)
            .unwrap_or(0)
    }

    const FAST: Duration = Duration::from_millis(10);

    fn settings_with_two_targets() -> ProbeSettings {
        let mut settings = default_settings();
        settings.targets.push(ProbeTargetConfig {
            id: 2,
            name: "Second".to_string(),
            kind: TargetKind::External,
            host: "127.0.0.1".to_string(),
            port: 443,
            method: ProbeMethod::Tcp,
            enabled: true,
            interval_ms: 5_000,
            timeout_ms: 2_000,
        });
        settings
    }

    #[tokio::test]
    async fn the_loop_probes_repeatedly_while_running() {
        let state = Arc::new(MonitorState::new());
        let network = Arc::new(FakeNetwork::resolving(
            vec![crate::monitoring::testing::loopback(443)],
            vec![Attempt::ok()],
        ));

        let handle = spawn(
            network.clone(),
            settings_with_two_targets(),
            state.clone(),
            FAST,
            None,
        );

        // Several intervals, not a real five seconds of waiting.
        tokio::time::sleep(Duration::from_millis(120)).await;
        handle.stop().await;

        assert!(
            network.attempt_count() >= 4,
            "expected repeated probes, got {}",
            network.attempt_count()
        );
    }

    #[tokio::test]
    async fn stopping_the_loop_marks_the_state_inactive() {
        let state = Arc::new(MonitorState::new());
        let network = Arc::new(FakeNetwork::resolving(
            vec![crate::monitoring::testing::loopback(443)],
            vec![Attempt::ok()],
        ));

        let handle = spawn(network, default_settings(), state.clone(), FAST, None);
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(state.is_active(), "the loop should report itself running");

        handle.stop().await;
        assert!(!state.is_active(), "stopping must clear the active flag");
    }

    #[tokio::test]
    async fn dropping_the_handle_stops_the_task() {
        let state = Arc::new(MonitorState::new());
        let network = Arc::new(FakeNetwork::resolving(
            vec![crate::monitoring::testing::loopback(443)],
            vec![Attempt::ok()],
        ));

        let handle = spawn(
            network.clone(),
            default_settings(),
            state.clone(),
            FAST,
            None,
        );
        tokio::time::sleep(Duration::from_millis(40)).await;

        drop(handle);
        // Aborting is asynchronous, so give the runtime a moment to notice.
        tokio::time::sleep(Duration::from_millis(40)).await;

        let after_drop = network.attempt_count();
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(
            network.attempt_count(),
            after_drop,
            "the task must not keep probing after its handle is dropped"
        );
        assert!(!state.is_active());
    }

    #[test]
    fn the_run_interval_comes_from_the_configuration() {
        let settings = default_settings();
        assert_eq!(
            interval_for(&settings),
            Duration::from_millis(settings.default_interval_ms)
        );
    }

    /// The slow end-to-end check, ignored by default because it needs a real
    /// network and two full intervals.
    ///
    /// Everything else in this file proves the loop against a scripted double,
    /// which cannot show whether the target the app actually ships with is
    /// reachable at all. This runs the production settings against the real
    /// network through two real ticks, and fails loudly if the shipped
    /// configuration cannot produce a measurement. Run it with:
    ///
    ///     cargo test -- --ignored
    #[tokio::test]
    #[ignore = "needs a real network and two full probe intervals"]
    async fn the_shipped_configuration_produces_a_real_reading() {
        let state = Arc::new(MonitorState::new());
        let settings = default_settings();

        let handle = spawn(
            Arc::new(TcpNetwork),
            settings.clone(),
            state.clone(),
            interval_for(&settings),
            None,
        );

        // Two intervals plus slack, so a slow first probe cannot be mistaken
        // for a second one.
        let two_intervals =
            Duration::from_millis(settings.default_interval_ms) * 2 + Duration::from_millis(2_000);
        tokio::time::sleep(two_intervals).await;
        handle.stop().await;

        let stored = state
            .outcome(1)
            .expect("the shipped target must have produced an outcome");
        assert!(
            stored.success,
            "the target the app ships with ({}) must be reachable, got {:?}",
            settings.targets[0].host, stored.error_kind
        );

        let (latency, captured_at) = state
            .latest_latency()
            .expect("a real reading must exist after two intervals");
        assert!(latency > 0.0, "latency must be positive, got {latency}");

        let age_ms = now_ms().saturating_sub(captured_at);
        assert!(
            age_ms < 60_000,
            "the newest reading should be recent, but it is {age_ms} ms old"
        );

        // The chart is fed from this buffer, so it is not enough that one reading
        // arrived: the series has to grow tick by tick or the line never moves.
        let series = state.recent_for_primary();
        assert!(
            series.len() >= 2,
            "the series must accumulate across ticks, got {} point(s)",
            series.len()
        );

        let mut ordered = true;
        for pair in series.windows(2) {
            if pair[0].captured_at > pair[1].captured_at {
                ordered = false;
            }
        }
        assert!(ordered, "points must be ordered oldest first");

        let plotted = crate::commands::overview::build_series(&state);
        assert_eq!(
            plotted.len(),
            series.len(),
            "every buffered sample must reach the chart, including failures"
        );
    }

    #[tokio::test]
    async fn a_paused_loop_stops_probing_and_resumes_when_asked() {
        let state = Arc::new(MonitorState::new());
        let network = Arc::new(FakeNetwork::resolving(
            vec![crate::monitoring::testing::loopback(443)],
            vec![Attempt::ok()],
        ));

        let handle = spawn(
            network.clone(),
            default_settings(),
            state.clone(),
            FAST,
            None,
        );

        // Let it take a few readings first, so there is history to keep.
        tokio::time::sleep(Duration::from_millis(60)).await;
        let before = network.attempt_count();
        assert!(before > 0, "the loop should have probed before the pause");

        state.set_paused(true);
        tokio::time::sleep(Duration::from_millis(80)).await;
        let during = network.attempt_count();
        assert_eq!(
            during, before,
            "a paused loop must not keep probing, or a pause would fill the history with samples nobody asked for"
        );

        state.set_paused(false);
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(
            network.attempt_count() > during,
            "resuming must start the probes again"
        );

        handle.stop().await;
    }

    /// A pause and a stop are different things. A paused loop is still alive,
    /// and the readings it took before the pause are still there.
    #[tokio::test]
    async fn a_pause_keeps_the_loop_alive_and_keeps_the_readings() {
        let state = Arc::new(MonitorState::new());
        let network = Arc::new(FakeNetwork::resolving(
            vec![crate::monitoring::testing::loopback(443)],
            vec![Attempt::ok()],
        ));

        let handle = spawn(
            network.clone(),
            default_settings(),
            state.clone(),
            FAST,
            None,
        );

        tokio::time::sleep(Duration::from_millis(60)).await;
        let readings_before = state.recent_for_primary().len();
        assert!(readings_before > 0);

        state.set_paused(true);
        tokio::time::sleep(Duration::from_millis(80)).await;

        assert!(
            state.is_active(),
            "a paused loop is still running, so the interface must not say it stopped"
        );
        assert!(
            state.recent_for_primary().len() >= readings_before,
            "the readings taken before the pause must still be there"
        );

        handle.stop().await;
    }

    #[tokio::test]
    async fn the_real_network_type_satisfies_the_loop_bound() {
        // A compile-time check that the production network can drive the loop,
        // without probing anything.
        let state = Arc::new(MonitorState::new());
        let handle = spawn(
            Arc::new(TcpNetwork),
            default_settings(),
            state,
            Duration::from_millis(60_000),
            None,
        );
        handle.stop().await;
    }

    // ---------------------------------------------------------------------
    // Persistence.
    // ---------------------------------------------------------------------

    /// The claim the storage wiring makes: what the loop measures is what ends
    /// up in the database. Something that only stored samples without ever
    /// running would pass every store test and fail here.
    #[tokio::test]
    async fn the_running_loop_writes_what_it_measures_to_history() {
        let state = Arc::new(MonitorState::new());
        let network = Arc::new(FakeNetwork::resolving(
            vec![crate::monitoring::testing::loopback(443)],
            vec![Attempt::ok()],
        ));
        let store = Arc::new(Store::in_memory().expect("store"));

        let handle = spawn(
            network.clone(),
            default_settings(),
            state.clone(),
            FAST,
            Some(store.clone()),
        );
        tokio::time::sleep(Duration::from_millis(120)).await;
        handle.stop().await;

        let samples = store.samples_since(1, 0).expect("read");
        assert!(
            samples.len() >= 2,
            "the loop must persist across ticks, got {} sample(s)",
            samples.len()
        );
        assert!(
            samples.iter().all(|sample| sample.latency_ms.is_some()),
            "every successful probe must be stored with its latency"
        );
    }

    /// A failed probe must reach the database as a failure with no number, so
    /// the stored history cannot later be read as a smooth connection.
    #[tokio::test]
    async fn a_failing_loop_stores_failures_without_inventing_latency() {
        let state = Arc::new(MonitorState::new());
        state.set_primary_target(1);
        let network = Arc::new(FakeNetwork::resolving(
            vec![crate::monitoring::testing::loopback(443)],
            vec![Attempt::failing(ProbeErrorKind::Timeout)],
        ));
        let store = Arc::new(Store::in_memory().expect("store"));

        let handle = spawn(
            network.clone(),
            default_settings(),
            state.clone(),
            FAST,
            Some(store.clone()),
        );
        tokio::time::sleep(Duration::from_millis(120)).await;
        handle.stop().await;

        let samples = store.samples_since(1, 0).expect("read");
        assert!(samples.len() >= 2, "failures must be stored too");
        assert!(
            samples.iter().all(|sample| sample.latency_ms.is_none()),
            "a stored failure must never carry a latency"
        );

        // The defaults probe two targets (the internet target and the router)
        // and the fake network fails both, so each records its own outage. That
        // separation is the point: an outage on one probe target is not an
        // outage on the other, and merging them would invent an outage that
        // never happened.
        let incidents = store.incidents(10).expect("read");
        assert!(
            incidents.iter().any(|incident| incident.target_id == 1),
            "the internet target's sustained outage must be recorded, got {incidents:?}"
        );
        assert!(
            incidents.iter().all(|incident| incident.probe_count >= 2),
            "each recorded outage crossed the threshold, so none is a blip"
        );
        assert!(
            incidents.iter().all(Incident::is_open),
            "none of them has recovered"
        );
    }

    /// History switched off must not stop monitoring. The app still measures,
    /// it just cannot answer questions about the past.
    #[tokio::test]
    async fn the_loop_runs_normally_with_no_history_store() {
        let state = Arc::new(MonitorState::new());
        let network = Arc::new(FakeNetwork::resolving(
            vec![crate::monitoring::testing::loopback(443)],
            vec![Attempt::ok()],
        ));

        let handle = spawn(
            network.clone(),
            default_settings(),
            state.clone(),
            FAST,
            None,
        );
        tokio::time::sleep(Duration::from_millis(80)).await;
        handle.stop().await;

        assert!(
            network.attempt_count() >= 2,
            "monitoring must continue without a database"
        );
        assert!(
            state.latest_latency().is_some(),
            "live readings must still be produced"
        );
    }
}
