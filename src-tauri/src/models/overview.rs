use serde::Serialize;

/// How a displayed value came to be, so the UI never presents a reading as more
/// certain than it actually is.
///
/// Every variant belongs to the wire contract that the frontend union mirrors,
/// so the ones the monitoring engine will construct later are not dead code.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueState {
    Measured,
    Calculated,
    Stale,
    Unavailable,
}

/// Connection health as described in the project plan. Classification arrives
/// with the monitoring engine; this type only carries the result.
///
/// Every variant belongs to the wire contract that the frontend union mirrors,
/// so the ones the health engine will construct later are not dead code.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthState {
    Connected,
    Degraded,
    Offline,
    Unknown,
    Paused,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricValue<T> {
    pub value: Option<T>,
    pub state: ValueState,
    pub measured_at: Option<i64>,
}

impl<T> MetricValue<T> {
    /// The honest shape for a metric that nothing has measured yet.
    pub fn unavailable() -> Self {
        Self {
            value: None,
            state: ValueState::Unavailable,
            measured_at: None,
        }
    }
}

/// How many probe attempts in the window did not connect, out of how many were
/// made.
///
/// On the wire beside the percentage so the UI can say "3 of 25" instead of a
/// bare figure. That phrasing is the point: macOS denies unprivileged ICMP
/// sockets, so a failed probe is a connection that did not open, **not** a lost
/// packet. Showing the counts keeps the number from being read as a packet-loss
/// claim, and it lets a reader tell a genuine zero failures from the absence of
/// any data at all.
///
/// The wire field is still named `packetLoss` because features 6, 7 and 17
/// consume it, but that name is historical and describes none of this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeFailureBasis {
    pub failed: u32,
    pub total: u32,
}

impl ProbeFailureBasis {
    /// Percentage from 0 to 100.
    ///
    /// Sent unrounded and formatted at the edge, matching how latency is
    /// handled, so one rounding decision lives in one place.
    ///
    /// Not called until the overview command reports this figure, which is the
    /// next build step. Allowed rather than left warning, for the same reason
    /// the unused enum variants above are: it belongs to a locked wire contract
    /// and is not dead weight.
    #[allow(dead_code)]
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.failed as f64 / self.total as f64) * 100.0
    }
}

/// The local router, probed separately from the internet.
///
/// Three fields rather than one, because the three states are genuinely
/// different: never probed, probed and answered, probed and did not answer.
/// Collapsing them into "a latency of null" would make a router that was never
/// checked indistinguishable from a router that is down.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterReachability {
    /// The address that was probed, so the reading has a named subject.
    pub host: Option<String>,
    /// True when the most recent probe connected, false when it did not, and
    /// null when no probe has completed inside the window.
    pub reachable: Option<bool>,
    /// Connect time in milliseconds when the router answered.
    pub latency: MetricValue<f64>,
    /// When the most recent probe ran, whether it succeeded or not.
    pub checked_at: Option<i64>,
}

impl RouterReachability {
    /// The honest shape before any router probe has completed.
    pub fn not_measured() -> Self {
        Self {
            host: None,
            reachable: None,
            latency: MetricValue::unavailable(),
            checked_at: None,
        }
    }
}

/// The WAN connection as the router reports it.
///
/// Every field is separately optional because the router's own reporting is
/// patchy: it can name its address while reporting no MTU, and a screen that
/// demanded all of them would show nothing at all.
///
/// `address_kind` is a classification rather than a boolean `is_public` because
/// "not public" covers several different situations a reader needs told apart.
/// On the observed connection it is `private`, which on a WAN interface means
/// the carrier is NATing as well as the router, and that is a materially
/// different situation from a directly assigned public address.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WanConnection {
    /// The router's own word for the connection state, such as `Connected`.
    /// Passed through rather than mapped, so the app never invents a state the
    /// router did not report.
    pub status: Option<String>,
    pub ip_address: Option<String>,
    /// "public", "carrierNat", "private", "local" or "unknown". Null when there
    /// is no address to classify.
    pub address_kind: Option<String>,
    pub gateway: Option<String>,
    pub dns_servers: Vec<String>,
    pub mac: Option<String>,
    pub mtu: Option<u32>,
    /// The access server identity, where the router reports one.
    pub bras: Option<String>,
    pub access_type: Option<String>,
    pub service_list: Option<String>,
    /// How long the connection has been up, in seconds, where reported.
    pub uptime_seconds: Option<u64>,
}

impl WanConnection {
    /// Nothing read from the router, so nothing shown.
    pub fn unavailable() -> Self {
        Self {
            status: None,
            ip_address: None,
            address_kind: None,
            gateway: None,
            dns_servers: Vec::new(),
            mac: None,
            mtu: None,
            bras: None,
            access_type: None,
            service_list: None,
            uptime_seconds: None,
        }
    }
}

/// What the router says about its own health, where it says anything.
///
/// Deliberately narrow. The build plan is explicit that CPU, memory, optical
/// temperature, voltage and bias current are **not** requirements, and that each
/// is added only when a verified endpoint exposes it with known semantics. The
/// fields here are the ones the observed firmware actually reports.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterHealth {
    /// The optical receive power in dBm, when the router reports one.
    pub optical_rx_dbm: MetricValue<f64>,
    /// Router CPU utilisation as a percentage, when reported and enabled.
    pub cpu_percent: MetricValue<f64>,
    /// Router memory utilisation as a percentage, when reported and enabled.
    pub memory_percent: MetricValue<f64>,
    /// The PON mode the router named, such as `gpon`.
    pub pon_mode: Option<String>,
    /// Whether the firmware reports an optical figure at all. False lets the
    /// screen say "this router does not report it" rather than "we could not
    /// read it", which are different claims.
    pub reports_optical: Option<bool>,
    /// Whether the CPU and memory figures are switched on in this firmware
    /// profile.
    pub reports_utilisation: Option<bool>,
}

impl RouterHealth {
    pub fn unavailable() -> Self {
        Self {
            optical_rx_dbm: MetricValue::unavailable(),
            cpu_percent: MetricValue::unavailable(),
            memory_percent: MetricValue::unavailable(),
            pon_mode: None,
            reports_optical: None,
            reports_utilisation: None,
        }
    }
}

/// One of the router's Wi-Fi radios.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiRadio {
    /// The radio's own name, such as `ath0`.
    pub name: String,
    /// Whether the radio is switched on.
    pub enabled: bool,
    /// Whether the radio is bound to a service, which the firmware reports
    /// separately from whether it is on.
    pub bound: bool,
}

/// The router's own view of the network around it.
///
/// Everything here is something the router reported, and nothing here is
/// inferred. Where the router reports that a thing is not present, that is
/// reported as the absence of the thing rather than as a zero.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterNetwork {
    /// The router's Wi-Fi radios, when it reports them.
    pub wifi_radios: Vec<WifiRadio>,
    /// The access type of the WAN interface, such as `GPON`. This is the
    /// router's own word for how the line reaches it.
    pub wan_access_type: Option<String>,
    /// The router's word for the IPv6 connection state on the WAN, such as
    /// `Invalid`. It is passed through rather than interpreted, because the
    /// router's vocabulary for it is the one that matters.
    pub ipv6_status: Option<String>,
    /// Whether the router says it supports IPv6 at all.
    pub supports_ipv6: Option<bool>,
    /// The IPv6 prefixes delegated to the router, when it reports any.
    pub ipv6_prefixes: Vec<String>,
}

/// The router's cumulative WAN byte counters, as read from the router.
///
/// These are totals since the WAN session began, not rates. The two are
/// different facts and the interface labels them differently: a total is a
/// number the router reports, a rate is derived from two readings of a total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WanCounters {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    /// When the counters were read, UTC epoch milliseconds.
    pub measured_at: i64,
}

/// A throughput rate derived from two counter readings.
///
/// A rate is a calculation, never a measurement: the router reports totals and
/// the app does the division. The state says so, so the interface can present
/// it as calculated rather than as something the router said.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficRate {
    /// Bytes per second towards the internet, or `None` when no rate can be
    /// derived.
    pub sent_bytes_per_second: Option<f64>,
    /// Bytes per second from the internet, or `None` when no rate can be
    /// derived.
    pub received_bytes_per_second: Option<f64>,
    pub state: ValueState,
    /// When the two readings were taken, so a held rate can state its age.
    pub measured_at: Option<i64>,
    /// Why no rate could be derived, in words the interface can show.
    ///
    /// The reasons are different facts: no second reading yet, a counter the
    /// router reset, and two readings too far apart to describe the present.
    /// A reader needs them told apart, because they need different actions.
    pub note: Option<String>,
}

impl TrafficRate {
    /// The honest shape before there is anything to derive a rate from.
    pub fn unavailable(note: impl Into<String>) -> Self {
        Self {
            sent_bytes_per_second: None,
            received_bytes_per_second: None,
            state: ValueState::Unavailable,
            measured_at: None,
            note: Some(note.into()),
        }
    }
}

/// The probe target behind the headline reading, named for display.
///
/// Sent with the overview so the interface can say what a latency is a
/// measurement of. A bare "17 ms" with no subject is the kind of number a reader
/// has to guess at.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeTargetSummary {
    pub name: String,
    pub host: String,
    pub port: u16,
    /// "TCP" today. A label rather than a boolean, so a second method later
    /// cannot be rendered as if it were a TCP connect.
    pub method: String,
    pub interval_ms: u64,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionOverview {
    pub health_state: HealthState,
    /// Which target the headline latency belongs to. Null before the loop names
    /// one, so nothing has to invent a subject for the number.
    pub primary_target: Option<ProbeTargetSummary>,
    pub monitoring_active: bool,
    /// Whether the loop is paused.
    ///
    /// Separate from `monitoring_active` on purpose. A paused app is not a
    /// stopped app: the loop is still alive and will resume, and the readings
    /// taken before the pause are still stored. Collapsing the two would make a
    /// paused app look like a crashed one.
    pub paused: bool,
    pub last_probe_at: Option<i64>,
    pub packet_loss_window_ms: Option<u64>,
    pub latency: MetricValue<f64>,
    pub packet_loss: MetricValue<f64>,
    pub download_rate: MetricValue<f64>,
    pub upload_rate: MetricValue<f64>,
    /// The counts behind `packet_loss`, so the UI can say "3 of 25" rather than
    /// a bare percentage that would read as a packet-loss claim this app never
    /// made. Null exactly when `packet_loss` is unavailable.
    pub probe_failure_basis: Option<ProbeFailureBasis>,
    /// The local router, kept separate from the internet reading so a local
    /// fault and an upstream fault are never merged into one number.
    pub router: RouterReachability,
}

impl ConnectionOverview {
    /// Before monitoring exists no reading is known, so every measurement is
    /// reported as unavailable rather than filled with a plausible zero.
    pub fn not_measured() -> Self {
        Self {
            health_state: HealthState::Unknown,
            primary_target: None,
            monitoring_active: false,
            paused: false,
            last_probe_at: None,
            packet_loss_window_ms: None,
            latency: MetricValue::unavailable(),
            packet_loss: MetricValue::unavailable(),
            download_rate: MetricValue::unavailable(),
            upload_rate: MetricValue::unavailable(),
            probe_failure_basis: None,
            router: RouterReachability::not_measured(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HealthState, ValueState};
    use serde_json::{Value, json};

    fn wire<T: serde::Serialize>(value: T) -> Value {
        serde_json::to_value(value).expect("contract types must serialise")
    }

    #[test]
    fn value_states_serialise_as_the_frontend_union_expects() {
        assert_eq!(wire(ValueState::Measured), json!("measured"));
        assert_eq!(wire(ValueState::Calculated), json!("calculated"));
        assert_eq!(wire(ValueState::Stale), json!("stale"));
        assert_eq!(wire(ValueState::Unavailable), json!("unavailable"));
    }

    #[test]
    fn health_states_serialise_as_the_frontend_union_expects() {
        assert_eq!(wire(HealthState::Connected), json!("connected"));
        assert_eq!(wire(HealthState::Degraded), json!("degraded"));
        assert_eq!(wire(HealthState::Offline), json!("offline"));
        assert_eq!(wire(HealthState::Unknown), json!("unknown"));
        assert_eq!(wire(HealthState::Paused), json!("paused"));
    }
}
