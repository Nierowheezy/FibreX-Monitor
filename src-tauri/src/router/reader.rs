//! The background router reader.
//!
//! A router read is a sequence of blocking HTTPS requests to a small embedded
//! device, so it runs on its own thread rather than inside the probe loop's
//! async runtime. It is also far slower and far less urgent than a probe: the
//! probe loop answers "is the connection up right now" every two seconds, while
//! the router answers "what does the line look like" every so often.
//!
//! The reader never panics the app. A failure is recorded and the loop continues,
//! because a router that is off, on another network, or behind a changed password
//! must not stop connectivity monitoring, which is the thing this app is for.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::client::read_router;
use super::state::RouterState;
use super::{RouterError, RouterErrorKind};
use crate::monitoring::now_ms;

/// How long between router reads.
///
/// Deliberately much slower than the probe loop. These are full page fetches
/// from a device with a modest web server, and nothing about a WAN address, an
/// optical figure or a CPU percentage changes on a two-second cadence. Reading
/// faster would cost the router real work and tell the reader nothing more.
pub const READ_INTERVAL: Duration = Duration::from_secs(30);

/// How long to wait before the first read.
///
/// Short, so the screen fills in promptly, but not immediate: the app is already
/// starting the probe loop and opening the history database, and three things
/// hitting the disk and the network at once on launch is how a first paint ends
/// up looking broken.
pub const FIRST_READ_DELAY: Duration = Duration::from_secs(2);

/// Performs one read and records the outcome.
///
/// Separated from the loop so it can be driven directly in a test and so the
/// failure handling is in exactly one place.
pub fn read_once(state: &RouterState) -> Result<(), RouterError> {
    let Some(credentials) = state.credentials() else {
        // Not configured is not a failure. Nothing is recorded, which leaves the
        // state in "never attempted" and lets the interface explain the absence
        // rather than showing an error that never happened.
        return Ok(());
    };

    match read_router(&credentials, now_ms()) {
        Ok(snapshot) => {
            // One line per read, so an operator can confirm from outside the app
            // that the integration is actually working. It names only the host,
            // how many pages answered, and nothing else: no credential, no token,
            // and no response body is ever reachable from here.
            let answered = snapshot.sources.iter().filter(|source| source.ok).count();
            eprintln!(
                "router read ok: {} pages answered of {}",
                answered,
                snapshot.sources.len()
            );
            state.record_success(snapshot);
            Ok(())
        }
        Err(error) => {
            eprintln!("router read failed: {:?} {}", error.kind, error.message);
            state.record_failure(error.clone());
            Err(error)
        }
    }
}

/// Starts the reader on its own thread.
///
/// The returned handle keeps the thread alive; dropping it does not stop the
/// thread, which is why it is managed by the app rather than being a local.
pub fn spawn(state: Arc<RouterState>) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("router-reader".to_string())
        .spawn(move || {
            thread::sleep(FIRST_READ_DELAY);

            loop {
                let _ = read_once(&state);
                thread::sleep(READ_INTERVAL);
            }
        })
        .unwrap_or_else(|error| {
            // A thread that cannot start is a serious environment problem, but
            // it must not take the app with it. Connectivity monitoring is the
            // feature that matters and it is already running by this point.
            eprintln!("router reader disabled, could not start its thread: {error}");
            // A finished thread with no work keeps the handle shape without
            // pretending the reader is running.
            thread::spawn(|| {})
        })
}

/// Records that no credentials are configured, without a network attempt.
///
/// Called at startup so the interface can tell "the operator has not configured
/// a router" from "the router has not been read yet". Both show nothing, but only
/// one of them is something a person needs to act on.
pub fn record_unconfigured(state: &RouterState) {
    state.record_failure(RouterError::new(
        RouterErrorKind::NoCredentials,
        "No router credentials are configured.".to_string(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::RouterCredentials;

    fn credentials() -> RouterCredentials {
        RouterCredentials {
            host: "192.168.100.1".to_string(),
            port: 80,
            username: "root".to_string(),
            password: "a-password".to_string(),
        }
    }

    #[test]
    fn read_once_does_nothing_when_the_router_is_not_configured() {
        let state = RouterState::new();

        let result = read_once(&state);

        assert!(result.is_ok(), "an unconfigured router is not a failure");
        let report = state.report();
        assert!(
            !report.attempted,
            "nothing was attempted, so the state must still read as never tried"
        );
        assert!(report.error.is_none());
    }

    #[test]
    fn read_once_reports_rather_than_panicking_when_the_router_is_unreachable() {
        let state = RouterState::new();
        // A port reserved for the discard service on loopback, so the connect
        // fails quickly without depending on an external network.
        state.set_credentials(Some(RouterCredentials {
            host: "127.0.0.1".to_string(),
            port: 9,
            ..credentials()
        }));

        let result = read_once(&state);

        assert!(
            result.is_err(),
            "an unreachable router must report a failure, not succeed silently"
        );
        let report = state.report();
        assert!(report.attempted);
        assert!(
            report.error.is_some(),
            "the failure must be recorded so the screen can explain it"
        );
    }

    #[test]
    fn an_unreachable_router_never_reports_a_fabricated_snapshot() {
        let state = RouterState::new();
        state.set_credentials(Some(RouterCredentials {
            host: "127.0.0.1".to_string(),
            port: 9,
            ..credentials()
        }));

        let _ = read_once(&state);

        assert!(
            state.snapshot().is_none(),
            "a failed read must not leave behind a snapshot with invented values"
        );
    }

    #[test]
    fn recording_an_unconfigured_router_is_visible_to_the_interface() {
        let state = RouterState::new();
        record_unconfigured(&state);

        let report = state.report();
        assert!(report.attempted);
        assert_eq!(
            report.error.map(|error| error.kind),
            Some(RouterErrorKind::NoCredentials),
            "the screen needs to tell 'not configured' apart from 'could not reach it'"
        );
        assert!(report.snapshot.is_none());
    }

    #[test]
    fn a_reader_error_never_carries_the_password() {
        let state = RouterState::new();
        state.set_credentials(Some(RouterCredentials {
            password: "must-not-appear".to_string(),
            host: "127.0.0.1".to_string(),
            port: 9,
            username: "root".to_string(),
        }));

        let _ = read_once(&state);

        let json = serde_json::to_string(&state.report()).expect("report must serialise");
        assert!(!json.contains("must-not-appear"));
    }

    /// The reader's own log line is a place a secret could leak, so it is pinned.
    ///
    /// The message that reaches `eprintln!` on a failure is the one the state
    /// holds, and it is built in `client.rs` without a credential. This asserts
    /// the whole recorded failure, which is the same text.
    #[test]
    fn a_reader_failure_message_names_the_host_and_nothing_secret() {
        let state = RouterState::new();
        state.set_credentials(Some(RouterCredentials {
            password: "must-not-appear".to_string(),
            host: "127.0.0.1".to_string(),
            port: 9,
            username: "root".to_string(),
        }));

        let error = read_once(&state).expect_err("the router is not listening on port 9");

        assert!(
            !error.message.contains("must-not-appear"),
            "a failure message must never carry the password"
        );
        assert!(
            !error.message.contains("a-password"),
            "a failure message must never carry any password"
        );
    }
}
