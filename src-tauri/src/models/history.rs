use serde::Serialize;

/// One recorded outage, as the interface reads it.
///
/// `recoveredAt` is null while the outage is still running, which is a real and
/// different state from a recovered one: an open incident has no duration yet,
/// only a duration so far.
///
/// There is deliberately no `cause` field. A failed TCP connect says a
/// connection did not open. It does not say why, and any field here would be
/// filled in with a guess.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentSummary {
    pub id: i64,
    pub target_id: i64,
    /// UTC epoch milliseconds of the first failed probe.
    pub started_at: i64,
    /// UTC epoch milliseconds of the first successful probe after the outage,
    /// or null while it is still running. When `closedBy` is `"unobserved"`,
    /// this is the last probe that was seen instead, and the duration is the
    /// part of the outage the app actually watched.
    pub recovered_at: Option<i64>,
    /// How many failed probes were recorded.
    pub probe_count: u32,
    /// Whether the router answered while this outage was running. Null when no
    /// router reading covers the window, which is not the same as the router
    /// being down and must never be rendered as if it were.
    pub router_reachable: Option<bool>,
    /// How the incident closed: `"probe"` when a successful probe was seen,
    /// `"unobserved"` when probing stopped first (pause, quit, sleep), null
    /// while it is still open.
    ///
    /// This is what stops an unobserved gap reading as confirmed downtime.
    pub closed_by: Option<String>,
}

/// What the history database can currently answer.
///
/// `stored` false means nothing is being written, either because the database
/// could not be opened or because no probe has run yet. The interface uses this
/// to say what is missing instead of showing an empty list that would read as
/// "no outages ever happened".
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryInfo {
    /// False when history is switched off entirely.
    pub enabled: bool,
    /// UTC epoch milliseconds of the oldest stored sample, or null.
    pub earliest_sample_at: Option<i64>,
    pub sample_count: u32,
    /// How long raw samples are kept before the oldest are dropped.
    pub retention_ms: i64,
}

/// How much of a period the connection was up, over stored probes.
///
/// "Uptime" here is the share of probes that connected. It is a count of
/// observations, not a stopwatch: the app knows what it measured and when, not
/// what the connection was doing between measurements. Saying so is the
/// difference between a defensible figure and a made-up one.
///
/// `window_start` and `window_end` are sent so the interface states the period
/// the figure covers instead of attaching it to "today", which the stored data
/// may not span at all.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UptimeSummary {
    /// Share of probes that connected, 0 to 100. Null when nothing is stored.
    pub percent: Option<f64>,
    pub connected: u32,
    pub total: u32,
    /// UTC epoch milliseconds of the oldest sample in the window, or null.
    pub window_start: Option<i64>,
    /// UTC epoch milliseconds of the newest sample in the window, or null.
    pub window_end: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn an_open_incident_serialises_its_recovery_as_null() {
        let incident = IncidentSummary {
            id: 1,
            target_id: 1,
            started_at: 1_700_000_000_000,
            recovered_at: None,
            probe_count: 4,
            router_reachable: Some(true),
            closed_by: None,
        };

        assert_eq!(
            serde_json::to_value(&incident).expect("serialise"),
            json!({
                "id": 1,
                "targetId": 1,
                "startedAt": 1_700_000_000_000i64,
                "recoveredAt": Value::Null,
                "probeCount": 4,
                "routerReachable": true,
                "closedBy": Value::Null,
            })
        );
    }

    /// The distinction the whole feature turns on: an incident where the router
    /// was never read must not serialise as one where the router was down.
    #[test]
    fn an_unread_router_serialises_as_null_not_false() {
        let incident = IncidentSummary {
            id: 2,
            target_id: 1,
            started_at: 1_000,
            recovered_at: Some(9_000),
            probe_count: 3,
            router_reachable: None,
            closed_by: Some("probe".to_string()),
        };

        let value = serde_json::to_value(&incident).expect("serialise");
        assert_eq!(value.get("routerReachable"), Some(&Value::Null));
        assert_ne!(
            value.get("routerReachable"),
            Some(&Value::Bool(false)),
            "no reading is not a reading of false"
        );
    }

    #[test]
    fn an_unobserved_close_names_itself_on_the_wire() {
        let incident = IncidentSummary {
            id: 3,
            target_id: 1,
            started_at: 1_000,
            recovered_at: Some(5_000),
            probe_count: 2,
            router_reachable: None,
            closed_by: Some("unobserved".to_string()),
        };

        let value = serde_json::to_value(&incident).expect("serialise");
        assert_eq!(value["closedBy"], json!("unobserved"));
        assert_ne!(
            value["closedBy"],
            json!("probe"),
            "a closed-by-pause incident must not claim a measured recovery"
        );
    }

    #[test]
    fn history_info_serialises_its_fields_for_the_settings_screen() {
        let info = HistoryInfo {
            enabled: true,
            earliest_sample_at: Some(1_700_000_000_000),
            sample_count: 512,
            retention_ms: 1_209_600_000,
        };

        assert_eq!(
            serde_json::to_value(&info).expect("serialise"),
            json!({
                "enabled": true,
                "earliestSampleAt": 1_700_000_000_000i64,
                "sampleCount": 512,
                "retentionMs": 1_209_600_000i64,
            })
        );
    }
}
