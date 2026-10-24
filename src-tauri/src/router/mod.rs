//! Read-only access to the Huawei OptiXstar router.
//!
//! Everything here is discovery-driven: the endpoint list, the two-token
//! handshake, the field names and the base64-not-hashed password encoding were
//! all observed against the real device and are recorded in
//! `buildflow/designs/router-integration-findings.md`. Nothing in this module
//! guesses at a field, and nothing writes to the router.
//!
//! The module is deliberately independent of the monitoring loop. Router data is
//! opportunistic: when it is available the interface can name the WAN address
//! and the optical receive power, and when it is not, the app keeps monitoring
//! and reports those fields as unavailable rather than inventing a default.
//!
//! A few accessors below are exercised only by the tests in this module. They
//! are the readable half of the state and snapshot contracts, and the tests are
//! what pins those contracts down, so the allow is scoped and stated rather than
//! the accessors being deleted and the tests reaching into private fields.
#![allow(dead_code)]

pub mod app_config;
pub mod client;
pub mod credentials;
pub mod parse;
pub mod reader;
pub mod state;
pub mod traffic;

use serde::{Deserialize, Serialize};

use crate::models::overview::{
    MetricValue, RouterHealth, RouterNetwork, ValueState, WanConnection, WanCounters,
};

/// Where the router lives and who the app signs in as.
///
/// The credentials are held here rather than read at each call site so there is
/// exactly one place that touches them, and one place to audit for logging
/// accidents. They are never serialised: `skip` on both fields keeps a stray
/// `serde_json::to_string` on a settings struct from printing a password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterCredentials {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

impl RouterCredentials {
    /// The base the client builds every request on.
    ///
    /// HTTPS on the configured port, because that is what the observed firmware
    /// serves its real UI over. A host given as a bare address is used exactly
    /// as typed, since the router is reached by address on the local network and
    /// a name would only add a resolver that can fail.
    pub fn base_url(&self) -> String {
        format!("https://{}:{}", self.host, self.port)
    }
}

/// A settings view of the credentials with the secret removed.
///
/// This is what crosses to the interface: the address, the user name, and
/// whether a password is configured. The password itself never leaves the
/// backend, so no React state, log line or error message can carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterAuthSummary {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    /// True when a password is present. The value is never sent.
    pub has_password: bool,
}

impl From<&RouterCredentials> for RouterAuthSummary {
    fn from(credentials: &RouterCredentials) -> Self {
        Self {
            host: credentials.host.clone(),
            port: credentials.port,
            username: Some(credentials.username.clone()),
            has_password: !credentials.password.is_empty(),
        }
    }
}

/// Why the router could not be read.
///
/// A taxonomy rather than a string, so the interface can say which step failed
/// and a reader can tell "the router is off" apart from "the password is wrong".
/// The two are entirely different pieces of advice, and collapsing them into one
/// "router error" would send someone to the wrong fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RouterErrorKind {
    /// No credentials are configured, so nothing was attempted.
    NoCredentials,
    /// The router did not answer on its address.
    Unreachable,
    /// The router answered but the sign-in was refused.
    AuthRejected,
    /// A page was fetched but did not contain the fields it was read for.
    UnexpectedResponse,
    /// The request took longer than the budget.
    Timeout,
}

/// A router read that did not produce data, kept distinct from an empty one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterError {
    pub kind: RouterErrorKind,
    /// A sentence for a reader. Never contains a credential, a token or a
    /// response body.
    pub message: String,
}

impl RouterError {
    pub fn new(kind: RouterErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Everything one successful router read produced.
///
/// The two halves are separate because they come from different pages and can
/// fail independently: the WAN page can be readable while the optical figures
/// are not reported, and an optical figure that vanishes must not take the WAN
/// address down with it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterSnapshot {
    /// When this read completed, UTC epoch milliseconds.
    pub read_at: i64,
    pub auth: RouterAuthSummary,
    pub wan: WanConnection,
    pub health: RouterHealth,
    /// The router's cumulative traffic counters, when the statistics page
    /// answered.
    ///
    /// These are totals since the WAN session began. A rate is derived from two
    /// of them, and that derivation lives in the state rather than here,
    /// because one reading cannot produce a rate.
    pub counters: Option<WanCounters>,
    /// The router's own view of the network around it: its Wi-Fi radios, the
    /// access type of the WAN, and its IPv6 state.
    pub network: RouterNetwork,
    /// Which pages answered, so the screen can be specific about what it could
    /// not read instead of showing one blanket failure.
    pub sources: Vec<RouterSourceStatus>,
}

/// Whether one page was read, and if not, why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterSourceStatus {
    pub name: String,
    pub path: String,
    pub ok: bool,
    /// Null when `ok` is true, because a successful read has no error to report.
    pub error: Option<RouterError>,
}

impl RouterSourceStatus {
    pub fn ok(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            ok: true,
            error: None,
        }
    }

    pub fn failed(name: impl Into<String>, path: impl Into<String>, error: RouterError) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            ok: false,
            error: Some(error),
        }
    }
}

impl RouterSnapshot {
    /// True when nothing about the router was actually read.
    ///
    /// A snapshot that names no source at all has also read nothing, which is
    /// the state before the first attempt completes.
    pub fn is_empty(&self) -> bool {
        !self.sources.iter().any(|source| source.ok)
    }
}

/// A reading whose provenance is that it came from the router, stated once.
///
/// Every router-sourced number travels through here so the `ValueState` is set
/// in one place. A parse failure becomes `Unavailable` with no value, never a
/// value of zero, which is the rule this project holds everywhere.
pub fn router_reading<T>(value: Option<T>, read_at: i64) -> MetricValue<T> {
    match value {
        Some(value) => MetricValue {
            value: Some(value),
            state: ValueState::Measured,
            measured_at: Some(read_at),
        },
        None => MetricValue::unavailable(),
    }
}
