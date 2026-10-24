use serde::Serialize;

/// One point on the latency chart.
///
/// `value` is null when a probe ran and failed. That is not the same as the
/// point being absent: the gap is the observation, and drawing a line straight
/// through it would claim the connection was fine at a moment when it was not.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyPoint {
    /// UTC epoch milliseconds.
    pub at: i64,
    /// Milliseconds, or null when the probe at this time measured nothing.
    pub value: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::LatencyPoint;
    use serde_json::{Value, json};

    #[test]
    fn points_serialise_as_the_frontend_type_expects() {
        let measured = serde_json::to_value(LatencyPoint {
            at: 1_700_000_000_000,
            value: Some(14.25),
        })
        .expect("point must serialise");

        assert_eq!(
            measured,
            json!({"at": 1_700_000_000_000i64, "value": 14.25})
        );

        let failed = serde_json::to_value(LatencyPoint {
            at: 1_700_000_002_000,
            value: None,
        })
        .expect("point must serialise");

        assert_eq!(
            failed,
            json!({"at": 1_700_000_002_000i64, "value": Value::Null}),
            "a failed probe must serialise as a present point with no value"
        );
    }
}
