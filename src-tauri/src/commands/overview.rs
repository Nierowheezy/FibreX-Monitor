use std::sync::Arc;

use tauri::State;

use crate::models::latency::LatencyPoint;
use crate::models::overview::{ConnectionOverview, MetricValue, RouterReachability, ValueState};
use crate::monitoring::health;
use crate::monitoring::state::{FreshLatency, MonitorState, SERIES_WINDOW_MS};
use crate::HistoryStore;

/// Builds the overview payload from whatever monitoring has actually seen.
///
/// Kept separate from the command so the logic can be tested without standing up
/// an app, and so the command stays a thin wiring layer.
pub fn build_overview(state: &MonitorState) -> ConnectionOverview {
    let mut overview = ConnectionOverview::not_measured();

    overview.monitoring_active = state.is_active();
    overview.paused = state.is_paused();
    overview.last_probe_at = state.last_success_at();

    // What the headline latency is a measurement of. Null until the loop names
    // a target, so nothing invents a subject for the number.
    overview.primary_target = state.primary_target_detail();

    overview.latency = match state.freshest_latency() {
        FreshLatency::Fresh {
            latency,
            measured_at,
        } => MetricValue {
            value: Some(latency),
            state: ValueState::Measured,
            measured_at: Some(measured_at),
        },
        // Held, and flagged as holding. The capture time travels with it so the
        // UI can say how old the number is. A stale value with no age is the
        // same unreadable blank 2b had, just spelled differently.
        FreshLatency::Stale {
            latency,
            measured_at,
        } => MetricValue {
            value: Some(latency),
            state: ValueState::Stale,
            measured_at: Some(measured_at),
        },
        // Nothing succeeded inside the window, so there is no number to hold.
        FreshLatency::None => MetricValue::unavailable(),
    };

    // The failure figure and the counts behind it. A genuine zero is measured,
    // because zero failed probes is real information; only an empty window is
    // unavailable.
    match state.failure_summary() {
        Some(basis) => {
            overview.packet_loss = MetricValue {
                value: Some(basis.percent()),
                state: ValueState::Measured,
                measured_at: state.last_success_at(),
            };
            overview.probe_failure_basis = Some(basis);
            overview.packet_loss_window_ms = Some(SERIES_WINDOW_MS);
        }
        None => {
            // No attempts in the window, so there is no figure and no window to
            // describe. Reporting the window anyway would imply a measurement
            // covering a stretch of time in which nothing was probed.
            overview.packet_loss_window_ms = None;
        }
    }

    // The verdict reads the recent window, so a failure ages out of it instead
    // of marking the connection degraded forever.
    overview.health_state = health::classify(state.is_active(), &state.recent_for_primary());

    // The router, reported separately. A router that was probed and did not
    // answer is `reachable: false`; a router that was never probed is
    // `reachable: null`. The verdict is never taken from this and never merged
    // with the internet reading, because a reachable router and an unreachable
    // internet are different facts and only one of them is about the provider.
    overview.router = match state.router_outcome() {
        Some(outcome) => RouterReachability {
            host: state.router_host(),
            reachable: Some(outcome.success),
            latency: if outcome.success {
                MetricValue {
                    value: outcome.latency_ms,
                    state: ValueState::Measured,
                    measured_at: Some(outcome.captured_at),
                }
            } else {
                MetricValue::unavailable()
            },
            checked_at: Some(outcome.captured_at),
        },
        None => RouterReachability {
            host: state.router_host(),
            ..RouterReachability::not_measured()
        },
    };

    overview
}

/// The recent latency series behind the chart, oldest first.
///
/// A separate command rather than another field on the overview, because the
/// overview payload's shape is a locked contract and a growing list of points
/// does not belong in a snapshot that is read on every poll.
pub fn build_series(state: &MonitorState) -> Vec<LatencyPoint> {
    state
        .recent_for_primary()
        .into_iter()
        .map(|outcome| LatencyPoint {
            at: outcome.captured_at,
            value: outcome.latency_ms,
        })
        .collect()
}

/// Everything the overview screen shows, in one payload.
#[tauri::command]
pub fn get_connection_overview(state: State<'_, Arc<MonitorState>>) -> ConnectionOverview {
    build_overview(state.inner())
}

/// Recent latency points for the chart.
#[tauri::command]
pub fn get_latency_series(state: State<'_, Arc<MonitorState>>) -> Vec<LatencyPoint> {
    build_series(state.inner())
}

/// Pauses or resumes the probe loop, and reports the state it ended in.
///
/// The state is returned rather than assumed, so a caller that asked to pause
/// can tell whether it actually did. The loop keeps running either way, which
/// is what makes a resume immediate and keeps the readings taken before the
/// pause in the history.
///
/// Pausing also closes any outage that is still open, at the last probe it
/// actually saw. Otherwise the pause would sit inside the incident and be
/// reported as downtime the app never measured.
#[tauri::command]
pub fn set_monitoring_paused(
    paused: bool,
    state: State<'_, Arc<MonitorState>>,
    history: State<'_, HistoryStore>,
) -> bool {
    state.set_paused(paused);

    if paused
        && let Some(store) = history.get()
    {
        // A failed close is not fatal: the gap check in `Store::record`
        // closes the same incident on the next probe after a resume, so the
        // record still cannot claim the pause as downtime.
        if let Err(error) = store.close_open_incidents() {
            eprintln!("could not close incidents for the pause: {error}");
        }
    }

    state.is_paused()
}

/// Whether the probe loop is paused right now.
#[tauri::command]
pub fn is_monitoring_paused(state: State<'_, Arc<MonitorState>>) -> bool {
    state.is_paused()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::probe::{ProbeErrorKind, ProbeOutcome};
    use crate::monitoring::store::Store;

    const METRICS: [&str; 4] = ["latency", "packetLoss", "downloadRate", "uploadRate"];

    /// Stamps outcomes relative to now.
    ///
    /// An absolute timestamp here would quietly age out of the five minute
    /// window as the calendar moved, and the test would keep passing its
    /// assertions for the wrong reason, or start failing for no visible cause.
    fn recent(offset_ms: i64) -> i64 {
        crate::monitoring::now_ms() - offset_ms
    }

    fn outcome(success: bool, latency: Option<f64>, at: i64) -> ProbeOutcome {
        ProbeOutcome {
            target_id: 1,
            captured_at: at,
            success,
            latency_ms: latency,
            error_kind: if success {
                None
            } else {
                Some(ProbeErrorKind::Unreachable)
            },
        }
    }

    fn payload(state: &MonitorState) -> serde_json::Value {
        serde_json::to_value(build_overview(state)).expect("overview must serialise")
    }

    #[test]
    fn an_empty_store_reports_no_connection_facts() {
        let state = MonitorState::new();
        let value = payload(&state);

        // Nothing is probing, so the honest verdict is that monitoring is not
        // running. "Unknown" would imply the app has an opinion it does not.
        assert_eq!(value["healthState"], serde_json::json!("paused"));
        assert_eq!(value["lastProbeAt"], serde_json::json!(null));
        assert_eq!(value["packetLossWindowMs"], serde_json::json!(null));
    }

    #[test]
    fn an_empty_store_reports_metrics_as_null_rather_than_zero() {
        let state = MonitorState::new();
        let value = payload(&state);

        for metric in METRICS {
            assert_eq!(
                value[metric]["value"],
                serde_json::json!(null),
                "{metric} value"
            );
            assert_eq!(
                value[metric]["state"],
                serde_json::json!("unavailable"),
                "{metric} state"
            );
            assert_eq!(
                value[metric]["measuredAt"],
                serde_json::json!(null),
                "{metric} measuredAt"
            );
        }
    }

    #[test]
    fn payload_shape_is_locked() {
        let state = MonitorState::new();
        let value = payload(&state);
        let object = value.as_object().expect("overview must be an object");

        // Twelve, not nine. Feature 2c added `probeFailureBasis` deliberately, so
        // that the UI can show the counts behind the percentage rather than a
        // figure that reads as a packet-loss claim this app never made. Feature
        // 3 added `router`, the local router read kept separate from the internet
        // reading so a local fault and an upstream fault never merge, and
        // `primaryTarget`, so a latency is never shown without its subject.
        // Feature 9 added `paused`, so a paused loop is not reported as a
        // stopped one.
        assert_eq!(object.len(), 12, "top-level field count changed");

        for key in [
            "healthState",
            "primaryTarget",
            "monitoringActive",
            "paused",
            "lastProbeAt",
            "packetLossWindowMs",
            "latency",
            "packetLoss",
            "downloadRate",
            "uploadRate",
            "probeFailureBasis",
            "router",
        ] {
            assert!(object.contains_key(key), "missing {key}");
        }
    }

    #[test]
    fn a_successful_probe_becomes_a_measured_latency() {
        let state = MonitorState::new();
        let at = recent(0);
        state.record(outcome(true, Some(14.2), at));

        let value = payload(&state);

        assert_eq!(value["latency"]["value"], serde_json::json!(14.2));
        assert_eq!(value["latency"]["state"], serde_json::json!("measured"));
        assert_eq!(value["latency"]["measuredAt"], serde_json::json!(at));
        assert_eq!(value["lastProbeAt"], serde_json::json!(at));
    }

    /// This is the behaviour 2b could not offer and 2b's spec promised here.
    /// A held reading carries its own capture time, so the UI can say how old it
    /// is; that time is when the number was taken, never when it went stale.
    #[test]
    fn a_failed_probe_holds_the_last_good_latency_as_stale() {
        let state = MonitorState::new();
        let success_at = recent(6_000);
        let failure_at = recent(2_000);

        state.record(outcome(true, Some(14.2), success_at));
        state.record(outcome(false, None, failure_at));

        let value = payload(&state);

        assert_eq!(
            value["latency"]["value"],
            serde_json::json!(14.2),
            "a real reading inside the window should be held, not thrown away"
        );
        assert_eq!(value["latency"]["state"], serde_json::json!("stale"));
        assert_eq!(
            value["latency"]["measuredAt"],
            serde_json::json!(success_at),
            "the age must be the age of the number, not of the failure"
        );
        assert_eq!(
            value["lastProbeAt"],
            serde_json::json!(success_at),
            "the last successful probe is unchanged by a later failure"
        );
    }

    #[test]
    fn a_window_with_no_success_reports_latency_as_unavailable() {
        let state = MonitorState::new();

        state.record(outcome(false, None, recent(6_000)));
        state.record(outcome(false, None, recent(2_000)));

        let value = payload(&state);

        assert_eq!(
            value["latency"]["value"],
            serde_json::json!(null),
            "there is nothing to hold, so nothing is shown"
        );
        assert_eq!(value["latency"]["state"], serde_json::json!("unavailable"));
        assert_eq!(value["latency"]["measuredAt"], serde_json::json!(null));
    }

    #[test]
    fn a_held_latency_is_dropped_once_it_leaves_the_window() {
        let state = MonitorState::new();
        let now = crate::monitoring::now_ms();

        // A success older than the window, then a fresh failure.
        state.record(outcome(
            true,
            Some(14.2),
            now - SERIES_WINDOW_MS as i64 - 1_000,
        ));
        state.record(outcome(false, None, recent(2_000)));

        let value = payload(&state);

        assert_eq!(
            value["latency"]["value"],
            serde_json::json!(null),
            "a reading that has aged out must not be presented as the current one"
        );
        assert_eq!(value["latency"]["state"], serde_json::json!("unavailable"));
    }

    #[test]
    fn the_newest_success_is_measured_even_when_earlier_probes_failed() {
        let state = MonitorState::new();
        // Marked running, or the verdict is Paused regardless of the samples,
        // which is correct behaviour and would make this assertion meaningless.
        state.set_active(true);
        let recovered_at = recent(2_000);

        state.record(outcome(false, None, recent(8_000)));
        state.record(outcome(false, None, recent(5_000)));
        state.record(outcome(true, Some(11.0), recovered_at));

        let value = payload(&state);

        assert_eq!(value["latency"]["state"], serde_json::json!("measured"));
        assert_eq!(value["latency"]["value"], serde_json::json!(11.0));
        assert_eq!(
            value["latency"]["measuredAt"],
            serde_json::json!(recovered_at)
        );
        assert_eq!(value["healthState"], serde_json::json!("degraded"));
    }

    // ---------------------------------------------------------------------
    // The failure figure on the wire.
    // ---------------------------------------------------------------------

    #[test]
    fn a_measured_failure_rate_travels_with_its_counts_and_window() {
        let state = MonitorState::new();
        state.set_active(true);

        state.record(outcome(true, Some(12.0), recent(6_000)));
        state.record(outcome(true, Some(13.0), recent(4_000)));
        state.record(outcome(true, Some(11.0), recent(2_000)));
        state.record(outcome(false, None, recent(0)));

        let value = payload(&state);

        assert_eq!(value["packetLoss"]["value"], serde_json::json!(25.0));
        assert_eq!(value["packetLoss"]["state"], serde_json::json!("measured"));
        assert_eq!(
            value["probeFailureBasis"],
            serde_json::json!({"failed": 1, "total": 4})
        );
        assert_eq!(
            value["packetLossWindowMs"],
            serde_json::json!(SERIES_WINDOW_MS),
            "the window must be the real one, not a guess at display time"
        );
    }

    #[test]
    fn a_genuine_zero_failure_rate_is_measured_not_unavailable() {
        let state = MonitorState::new();
        state.set_active(true);

        state.record(outcome(true, Some(12.0), recent(4_000)));
        state.record(outcome(true, Some(13.0), recent(0)));

        let value = payload(&state);

        assert_eq!(
            value["packetLoss"]["value"],
            serde_json::json!(0.0),
            "zero failed probes is a real measurement"
        );
        assert_eq!(value["packetLoss"]["state"], serde_json::json!("measured"));
        assert_eq!(
            value["probeFailureBasis"],
            serde_json::json!({"failed": 0, "total": 2})
        );
    }

    #[test]
    fn an_empty_window_reports_no_rate_no_counts_and_no_window() {
        let state = MonitorState::new();
        let value = payload(&state);

        assert_eq!(value["packetLoss"]["value"], serde_json::json!(null));
        assert_eq!(
            value["packetLoss"]["state"],
            serde_json::json!("unavailable")
        );
        assert_eq!(
            value["probeFailureBasis"],
            serde_json::json!(null),
            "no attempts means no counts"
        );
        assert_eq!(
            value["packetLossWindowMs"],
            serde_json::json!(null),
            "naming a window in which nothing was probed would imply a measurement"
        );
    }

    #[test]
    fn the_basis_is_present_exactly_when_the_rate_is() {
        // The two travel together by construction. Asserted so a future change
        // that populates one without the other is caught here rather than
        // rendering "12.5%" with nothing to explain it.
        let measured = MonitorState::new();
        measured.record(outcome(false, None, recent(0)));
        let measured_value = payload(&measured);
        assert!(measured_value["probeFailureBasis"].is_object());
        assert!(measured_value["packetLoss"]["value"].is_number());

        let empty = payload(&MonitorState::new());
        assert!(empty["probeFailureBasis"].is_null());
        assert!(empty["packetLoss"]["value"].is_null());
    }

    #[test]
    fn a_running_loop_reports_monitoring_active() {
        let state = MonitorState::new();
        state.set_active(true);

        assert_eq!(payload(&state)["monitoringActive"], serde_json::json!(true));

        state.set_active(false);
        assert_eq!(
            payload(&state)["monitoringActive"],
            serde_json::json!(false)
        );
    }

    #[test]
    fn a_running_loop_with_working_probes_reports_connected() {
        let state = MonitorState::new();
        state.set_active(true);
        state.record(outcome(true, Some(3.0), recent(0)));

        assert_eq!(
            payload(&state)["healthState"],
            serde_json::json!("connected")
        );
    }

    #[test]
    fn a_running_loop_whose_probes_all_fail_reports_offline() {
        let state = MonitorState::new();
        state.set_active(true);
        state.record(outcome(false, None, recent(0)));
        state.record(outcome(false, None, recent(2_000)));

        assert_eq!(payload(&state)["healthState"], serde_json::json!("offline"));
    }

    #[test]
    fn a_running_loop_with_partly_working_probes_reports_degraded() {
        let state = MonitorState::new();
        state.set_active(true);
        state.record(outcome(true, Some(3.0), recent(0)));
        state.record(outcome(false, None, recent(2_000)));

        assert_eq!(
            payload(&state)["healthState"],
            serde_json::json!("degraded")
        );
    }

    #[test]
    fn a_loop_that_stopped_after_success_reports_paused_not_connected() {
        let state = MonitorState::new();
        state.record(outcome(true, Some(3.0), recent(0)));
        state.set_active(true);
        state.set_active(false);

        let value = payload(&state);

        assert_eq!(
            value["healthState"],
            serde_json::json!("paused"),
            "a good reading must not be presented as a current verdict once nothing is probing"
        );
        assert_eq!(
            value["latency"]["value"],
            serde_json::json!(3.0),
            "the last real reading is still worth showing"
        );
    }

    #[test]
    fn the_series_keeps_failed_probes_as_points_with_no_value() {
        let state = MonitorState::new();
        state.set_active(true);
        state.record(outcome(true, Some(12.0), recent(6_000)));
        state.record(outcome(false, None, recent(4_000)));
        state.record(outcome(true, Some(14.0), recent(2_000)));

        let series = build_series(&state);
        let value = serde_json::to_value(&series).expect("series must serialise");

        assert_eq!(value.as_array().expect("array").len(), 3);
        assert_eq!(value[0]["value"], serde_json::json!(12.0));
        assert_eq!(
            value[1]["value"],
            serde_json::json!(null),
            "a failed probe must stay on the chart as a gap, not vanish from it"
        );
        assert_eq!(value[2]["value"], serde_json::json!(14.0));
    }

    #[test]
    fn the_series_is_empty_before_anything_is_probed() {
        assert!(build_series(&MonitorState::new()).is_empty());
    }

    /// Registration and serialisation are the two things a unit call cannot
    /// prove, so this drives the commands through the real IPC boundary.
    #[test]
    fn commands_answer_over_ipc_with_the_contract() {
        use tauri::ipc::CallbackFn;
        use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
        use tauri::webview::InvokeRequest;

        let app = mock_builder()
            .manage(Arc::new(MonitorState::new()))
            .invoke_handler(tauri::generate_handler![
                get_connection_overview,
                get_latency_series
            ])
            .build(mock_context(noop_assets()))
            .expect("mock app must build");

        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview must build");

        let call = |cmd: &str| {
            let url = "tauri://localhost".parse().expect("valid invoke url");
            get_ipc_response(
                &webview,
                InvokeRequest {
                    cmd: cmd.into(),
                    callback: CallbackFn(0),
                    error: CallbackFn(1),
                    url,
                    body: tauri::ipc::InvokeBody::default(),
                    headers: Default::default(),
                    invoke_key: INVOKE_KEY.to_string(),
                },
            )
            .expect("command must succeed")
            .deserialize::<serde_json::Value>()
            .expect("response must be JSON")
        };

        let overview = call("get_connection_overview");
        assert_eq!(overview["healthState"], serde_json::json!("paused"));
        assert_eq!(overview["latency"]["value"], serde_json::json!(null));
        assert_eq!(
            overview["latency"]["state"],
            serde_json::json!("unavailable")
        );

        let series = call("get_latency_series");
        assert_eq!(series, serde_json::json!([]));
    }

    /// The history commands, through the real IPC boundary. Registration is the
    /// part a unit call cannot prove, and a command that is written but never
    /// registered fails silently with an unknown-command error at runtime.
    #[test]
    fn history_commands_answer_over_ipc_with_the_contract() {
        use crate::HistoryStore;
        use tauri::ipc::CallbackFn;
        use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
        use tauri::webview::InvokeRequest;

        let store = Arc::new(Store::in_memory().expect("store"));

        // Two failures and a recovery, so there is one closed incident to read
        // back and the test proves the whole path rather than an empty list.
        //
        // Stamped relative to now, because `get_stored_series` is windowed: an
        // absolute timestamp here would silently age out of the requested range
        // as the calendar moved and the test would start passing for the wrong
        // reason.
        let base = crate::monitoring::now_ms() - 10_000;
        store
            .record(&probe_outcome(1, base, false), None)
            .expect("write");
        store
            .record(&probe_outcome(1, base + 2_000, false), None)
            .expect("write");
        store
            .record(&probe_outcome(1, base + 4_000, true), None)
            .expect("write");

        let state = Arc::new(MonitorState::new());
        state.set_primary_target(1);

        let app = mock_builder()
            .manage(state)
            .manage(HistoryStore(Some(store)))
            .invoke_handler(tauri::generate_handler![
                crate::commands::history::get_incidents,
                crate::commands::history::get_history_info,
                crate::commands::history::get_stored_series,
                crate::commands::history::get_uptime
            ])
            .build(mock_context(noop_assets()))
            .expect("mock app must build");

        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview must build");

        let call_with = |cmd: &str, body: tauri::ipc::InvokeBody| {
            let url = "tauri://localhost".parse().expect("valid invoke url");
            get_ipc_response(
                &webview,
                InvokeRequest {
                    cmd: cmd.into(),
                    callback: CallbackFn(0),
                    error: CallbackFn(1),
                    url,
                    body,
                    headers: Default::default(),
                    invoke_key: INVOKE_KEY.to_string(),
                },
            )
            .expect("command must succeed")
            .deserialize::<serde_json::Value>()
            .expect("response must be JSON")
        };

        let call = |cmd: &str| call_with(cmd, tauri::ipc::InvokeBody::default());

        let incidents = call("get_incidents");
        let list = incidents.as_array().expect("an array");
        assert_eq!(list.len(), 1, "one recorded outage, got {incidents}");
        assert_eq!(list[0]["startedAt"], serde_json::json!(base));
        assert_eq!(list[0]["recoveredAt"], serde_json::json!(base + 4_000));
        assert_eq!(list[0]["probeCount"], serde_json::json!(2));
        assert_eq!(
            list[0]["routerReachable"],
            serde_json::json!(null),
            "no router reading is not a reading of false"
        );

        let info = call("get_history_info");
        assert_eq!(info["enabled"], serde_json::json!(true));
        assert_eq!(info["sampleCount"], serde_json::json!(3));
        assert_eq!(info["earliestSampleAt"], serde_json::json!(base));

        let series = call_with(
            "get_stored_series",
            tauri::ipc::InvokeBody::Json(serde_json::json!({ "sinceMs": 60_000 })),
        );
        let points = series.as_array().expect("an array");
        assert_eq!(points.len(), 3);
        assert_eq!(
            points[1]["value"],
            serde_json::json!(null),
            "a stored failure comes back as a gap"
        );
    }

    /// A probe outcome with a single success flag, for the tests below.
    fn probe_outcome(target_id: i64, at: i64, success: bool) -> ProbeOutcome {
        ProbeOutcome {
            target_id,
            captured_at: at,
            success,
            latency_ms: success.then_some(14.0),
            error_kind: (!success).then_some(ProbeErrorKind::Timeout),
        }
    }

    /// An outcome for the router target, which the defaults number as 2.
    fn router_probe(success: bool, latency: Option<f64>, at: i64) -> ProbeOutcome {
        ProbeOutcome {
            target_id: 2,
            captured_at: at,
            success,
            latency_ms: latency,
            error_kind: if success {
                None
            } else {
                Some(ProbeErrorKind::Timeout)
            },
        }
    }

    #[test]
    fn the_router_is_unreported_until_it_is_probed() {
        let state = MonitorState::new();
        state.set_router_target(2, "192.168.100.1");

        let value = payload(&state);
        assert_eq!(value["router"]["host"], serde_json::json!("192.168.100.1"));
        assert_eq!(
            value["router"]["reachable"],
            serde_json::json!(null),
            "a router that was never probed is not the same as one that is down"
        );
        assert_eq!(value["router"]["latency"]["value"], serde_json::json!(null));
        assert_eq!(value["router"]["checkedAt"], serde_json::json!(null));
    }

    #[test]
    fn a_reachable_router_reports_its_address_and_connect_time() {
        let state = MonitorState::new();
        state.set_router_target(2, "192.168.100.1");
        state.record(router_probe(true, Some(1.4), recent(1_000)));

        let value = payload(&state);
        assert_eq!(value["router"]["reachable"], serde_json::json!(true));
        assert_eq!(value["router"]["latency"]["value"], serde_json::json!(1.4));
        assert_eq!(
            value["router"]["latency"]["state"],
            serde_json::json!("measured")
        );
    }

    #[test]
    fn an_unreachable_router_reports_false_without_a_latency() {
        let state = MonitorState::new();
        state.set_router_target(2, "192.168.100.1");
        state.record(router_probe(false, None, recent(1_000)));

        let value = payload(&state);
        assert_eq!(
            value["router"]["reachable"],
            serde_json::json!(false),
            "a probed router that did not answer is an answer in itself"
        );
        assert_eq!(
            value["router"]["latency"]["value"],
            serde_json::json!(null),
            "a failed connect must not carry a number"
        );
    }

    #[test]
    fn an_old_router_probe_ages_out_rather_than_reporting_stale_reachability() {
        let state = MonitorState::new();
        state.set_router_target(2, "192.168.100.1");
        // Older than the window, so it is no longer evidence about now.
        state.record(router_probe(
            true,
            Some(1.4),
            recent(SERIES_WINDOW_MS as i64 + 1_000),
        ));

        let value = payload(&state);
        assert_eq!(value["router"]["reachable"], serde_json::json!(null));
    }
}
