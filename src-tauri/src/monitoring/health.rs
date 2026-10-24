//! Deciding what to call the connection.
//!
//! The rule this app must never break: a verdict describes what our own probes
//! saw, and nothing more. Nothing here inspects the router, knows an ISP
//! exists, or explains a cause. A user seeing "Offline" learns that their probes
//! failed, which is a fact; anything past that would be the app guessing.

use crate::models::overview::HealthState;
use crate::monitoring::probe::ProbeOutcome;

/// Classifies the connection from recent probe outcomes.
///
/// `active` comes first and overrides everything. If nothing is probing, the
/// app has no current opinion at all, and claiming "Connected" from a reading
/// taken before the loop died would be the most misleading thing it could do.
/// Reporting Paused points at the real problem instead.
pub fn classify(active: bool, recent: &[ProbeOutcome]) -> HealthState {
    if !active {
        return HealthState::Paused;
    }

    if recent.is_empty() {
        return HealthState::Unknown;
    }

    let succeeded = recent.iter().filter(|outcome| outcome.success).count();

    match succeeded {
        0 => HealthState::Offline,
        n if n == recent.len() => HealthState::Connected,
        // Some probes worked and some did not. That is a partial loss of
        // reachability, and calling it anything stronger would overstate it.
        _ => HealthState::Degraded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::probe::{ProbeErrorKind, ProbeOutcome};

    fn sample(success: bool, at: i64) -> ProbeOutcome {
        ProbeOutcome {
            target_id: 1,
            captured_at: at,
            success,
            latency_ms: success.then_some(12.0),
            error_kind: (!success).then_some(ProbeErrorKind::Timeout),
        }
    }

    fn samples(results: &[bool]) -> Vec<ProbeOutcome> {
        results
            .iter()
            .enumerate()
            .map(|(index, &success)| sample(success, index as i64))
            .collect()
    }

    #[test]
    fn a_loop_that_is_not_running_reports_paused_whatever_it_last_saw() {
        // The important case: probes succeeded a moment ago, then the loop died.
        // "Connected" here would be a stale claim presented as a live one.
        let recent = samples(&[true, true, true]);

        assert_eq!(classify(false, &recent), HealthState::Paused);
    }

    #[test]
    fn a_running_loop_with_no_readings_yet_reports_unknown() {
        assert_eq!(classify(true, &[]), HealthState::Unknown);
    }

    #[test]
    fn every_probe_succeeding_is_connected() {
        assert_eq!(
            classify(true, &samples(&[true, true, true])),
            HealthState::Connected
        );
    }

    #[test]
    fn a_single_success_in_the_window_is_connected() {
        // One success out of one is not evidence of a pattern. It is all the
        // evidence there is, so it is not degraded.
        assert_eq!(classify(true, &samples(&[true])), HealthState::Connected);
    }

    #[test]
    fn every_probe_failing_is_offline() {
        assert_eq!(
            classify(true, &samples(&[false, false, false])),
            HealthState::Offline
        );
    }

    #[test]
    fn mixed_results_are_degraded() {
        assert_eq!(
            classify(true, &samples(&[true, false, true])),
            HealthState::Degraded
        );
    }

    #[test]
    fn the_oldest_result_ages_out_of_the_verdict() {
        // The recent samples are all good, but a failure sits just behind them.
        // Recent is already windowed by the caller, so the verdict follows the
        // window rather than all of history.
        let mut recent = samples(&[false]);
        recent.extend(samples(&[true, true]));

        assert_eq!(classify(true, &recent), HealthState::Degraded);
        assert_eq!(
            classify(true, &recent[1..]),
            HealthState::Connected,
            "an old failure must not keep the connection degraded forever"
        );
    }
}
