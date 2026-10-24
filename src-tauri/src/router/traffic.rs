//! Turning the router's byte counters into a rate.
//!
//! The router reports totals. A rate is the app's own arithmetic on two of
//! those totals, which makes it a calculated figure rather than a measured one,
//! and the interface says so.
//!
//! Every way this can go wrong is handled as a distinct outcome rather than as
//! a number:
//!
//! - One reading and no second one yet: no rate, because there is nothing to
//!   divide.
//! - A counter that went down: the router reset it, so the delta is unknown.
//!   Rendering the difference as a huge spike would be the worst possible
//!   answer, because it would look like the fastest moment in the history.
//! - Two readings that are too far apart: the rate describes a period the
//!   reader is not looking at, so it is marked stale instead of presented as
//!   current.
//! - Two readings at the same moment: no time passed, so no rate.

use crate::models::overview::{TrafficRate, ValueState, WanCounters};

/// The longest gap between two readings that still describes the present.
///
/// The router is read every thirty seconds, so a gap much wider than that means
/// the reader missed some reads. A rate averaged over a longer stretch is
/// still true of that stretch, but it is not true of now, and showing it as
/// current would be a claim the number does not make.
pub const MAX_RATE_AGE_MS: i64 = 5 * 60 * 1_000;

/// The shortest gap that can produce a rate.
///
/// Two readings in the same millisecond have no time between them, and
/// dividing by zero is not a thing to attempt.
pub const MIN_RATE_GAP_MS: i64 = 1;

/// A rate derived from two counter readings.
///
/// `previous` is the older reading and `current` the newer one. They must be
/// two readings of the same session: a router that rebooted between them
/// restarts its counters, and the reset case below catches that.
pub fn rate_between(previous: &WanCounters, current: &WanCounters) -> TrafficRate {
    let gap = current.measured_at - previous.measured_at;

    if gap < MIN_RATE_GAP_MS {
        return TrafficRate::unavailable("Two readings with no time between them.");
    }

    if gap > MAX_RATE_AGE_MS {
        let mut rate =
            TrafficRate::unavailable("The two readings are too far apart to describe the present.");
        rate.state = ValueState::Stale;
        rate.measured_at = Some(current.measured_at);
        return rate;
    }

    let seconds = gap as f64 / 1000.0;

    // A counter that went down was reset. The delta across a reset is not a
    // rate, it is an artefact, so the direction is reported as unavailable
    // rather than as a spike.
    let sent = current
        .bytes_sent
        .checked_sub(previous.bytes_sent)
        .map(|delta| delta as f64 / seconds);
    let received = current
        .bytes_received
        .checked_sub(previous.bytes_received)
        .map(|delta| delta as f64 / seconds);

    if sent.is_none() || received.is_none() {
        return TrafficRate::unavailable("The router reset its traffic counters.");
    }

    TrafficRate {
        sent_bytes_per_second: sent,
        received_bytes_per_second: received,
        state: ValueState::Calculated,
        measured_at: Some(current.measured_at),
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::overview::ValueState;

    fn counters(at: i64, sent: u64, received: u64) -> WanCounters {
        WanCounters {
            bytes_sent: sent,
            bytes_received: received,
            packets_sent: 0,
            packets_received: 0,
            measured_at: at,
        }
    }

    #[test]
    fn a_rate_is_the_difference_over_the_time_between() {
        // 30 seconds, 30000 bytes sent and 60000 received.
        let previous = counters(0, 1_000, 2_000);
        let current = counters(30_000, 31_000, 62_000);

        let rate = rate_between(&previous, &current);

        assert_eq!(rate.sent_bytes_per_second, Some(1_000.0));
        assert_eq!(rate.received_bytes_per_second, Some(2_000.0));
        assert_eq!(
            rate.state,
            ValueState::Calculated,
            "a rate is the app's arithmetic, not something the router said"
        );
        assert_eq!(rate.measured_at, Some(30_000));
        assert!(rate.note.is_none());
    }

    /// The rule that matters most: a reset must never become a spike.
    #[test]
    fn a_counter_that_went_down_produces_no_rate() {
        let previous = counters(0, 5_000_000, 9_000_000);
        let current = counters(30_000, 12, 40);

        let rate = rate_between(&previous, &current);

        assert_eq!(rate.sent_bytes_per_second, None);
        assert_eq!(rate.received_bytes_per_second, None);
        assert_eq!(rate.state, ValueState::Unavailable);
        let note = rate.note.expect("a reset must say so");
        assert!(
            note.contains("reset"),
            "the reason has to be said, because a reset is a different fact from a slow link: {note:?}"
        );
    }

    #[test]
    fn one_counter_resetting_leaves_the_other_unavailable_too() {
        // A rate is a pair. Half of one is not a rate, and showing the half
        // that worked would imply the reset never happened.
        let previous = counters(0, 5_000_000, 9_000_000);
        let current = counters(30_000, 5_100_000, 40);

        let rate = rate_between(&previous, &current);

        assert_eq!(rate.received_bytes_per_second, None);
        assert_eq!(rate.state, ValueState::Unavailable);
    }

    #[test]
    fn two_readings_at_the_same_moment_produce_no_rate() {
        let previous = counters(1_000, 1_000, 2_000);
        let current = counters(1_000, 2_000, 4_000);

        let rate = rate_between(&previous, &current);

        assert_eq!(rate.sent_bytes_per_second, None);
        assert_eq!(rate.state, ValueState::Unavailable);
    }

    #[test]
    fn two_readings_too_far_apart_are_stale_rather_than_current() {
        let previous = counters(0, 1_000, 2_000);
        let current = counters(MAX_RATE_AGE_MS + 1_000, 31_000, 62_000);

        let rate = rate_between(&previous, &current);

        assert_eq!(rate.state, ValueState::Stale);
        assert_eq!(rate.sent_bytes_per_second, None);
        assert_eq!(
            rate.measured_at,
            Some(MAX_RATE_AGE_MS + 1_000),
            "a stale rate still says when its readings were taken"
        );
    }

    /// A counter that has not moved at all is a real reading of zero traffic,
    /// and must not be confused with the absence of a reading.
    #[test]
    fn a_counter_that_did_not_move_is_a_rate_of_zero() {
        let previous = counters(0, 1_000, 2_000);
        let current = counters(30_000, 1_000, 2_000);

        let rate = rate_between(&previous, &current);

        assert_eq!(rate.sent_bytes_per_second, Some(0.0));
        assert_eq!(rate.received_bytes_per_second, Some(0.0));
        assert_eq!(rate.state, ValueState::Calculated);
    }
}
