//! One probe: is this target reachable, and how long did it take?
//!
//! Latency comes from a TCP connect rather than ICMP because macOS does not
//! permit unprivileged ICMP sockets. A connect measures slightly more than a
//! bare round trip, which is the honest cost of measuring without privileges.

use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::{TcpStream, lookup_host};
use tokio::time::timeout;

use super::config::ProbeTargetConfig;
use super::now_ms;

/// Why a probe failed. This is the cause of a failure, not the freshness of a
/// reading, which is what `ValueState` describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeErrorKind {
    Timeout,
    Unreachable,
    Dns,
    Other,
}

/// Exactly a `probe_sample` row from project-overview.md, minus storage and
/// minus the health state that feature 4 owns.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeOutcome {
    pub target_id: i64,
    /// UTC epoch milliseconds.
    pub captured_at: i64,
    pub success: bool,
    /// Milliseconds. `None` whenever `success` is false.
    pub latency_ms: Option<f64>,
    /// `None` whenever `success` is true.
    pub error_kind: Option<ProbeErrorKind>,
}

/// The only capability a probe needs from the outside world.
///
/// A real network cannot be made to produce a timeout or a DNS failure on
/// demand, so tests implement this instead. That matters here more than usual:
/// this app runs on exactly the machines where the network misbehaves.
pub trait Network {
    /// The addresses to try, or the reason the host could not be resolved.
    fn resolve(
        &self,
        host: &str,
        port: u16,
    ) -> impl Future<Output = Result<Vec<SocketAddr>, ProbeErrorKind>> + Send;

    /// Try one address under a time budget.
    fn attempt(
        &self,
        address: SocketAddr,
        budget: Duration,
    ) -> impl Future<Output = Result<(), ProbeErrorKind>> + Send;
}

/// The real network.
pub struct TcpNetwork;

impl Network for TcpNetwork {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ProbeErrorKind> {
        // An address literal needs no resolver. Skipping the lookup keeps a
        // numeric target working even while name resolution is broken.
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Ok(vec![SocketAddr::new(ip, port)]);
        }

        lookup_host((host, port))
            .await
            .map(|addresses| addresses.collect())
            .map_err(|_| ProbeErrorKind::Dns)
    }

    async fn attempt(&self, address: SocketAddr, budget: Duration) -> Result<(), ProbeErrorKind> {
        match timeout(budget, TcpStream::connect(address)).await {
            // The timer firing is the authority. Whatever the socket was doing
            // when the budget ran out, the probe is a timeout.
            Err(_elapsed) => Err(ProbeErrorKind::Timeout),
            Ok(Err(error)) => Err(classify_io_error(&error)),
            Ok(Ok(_stream)) => Ok(()),
        }
    }
}

/// Map an IO error onto the failure taxonomy. Anything unrecognised stays
/// `Other` rather than being guessed into a more specific kind, because a wrong
/// specific kind would read as a diagnosis the app never made.
pub fn classify_io_error(error: &io::Error) -> ProbeErrorKind {
    match error.kind() {
        io::ErrorKind::TimedOut => ProbeErrorKind::Timeout,
        io::ErrorKind::ConnectionRefused
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::HostUnreachable
        | io::ErrorKind::NetworkUnreachable
        | io::ErrorKind::AddrNotAvailable => ProbeErrorKind::Unreachable,
        _ => ProbeErrorKind::Other,
    }
}

/// Probe a target over the real network.
pub async fn probe(target: &ProbeTargetConfig) -> ProbeOutcome {
    probe_once(&TcpNetwork, target).await
}

/// Probe a target using an injected network, so every outcome is reachable in a
/// test without touching a socket.
pub async fn probe_once<N>(network: &N, target: &ProbeTargetConfig) -> ProbeOutcome
where
    N: Network + Sync + ?Sized,
{
    let started = Instant::now();
    let deadline = Duration::from_millis(target.timeout_ms);

    let addresses = match network.resolve(&target.host, target.port).await {
        // Resolving to nothing is a name problem, not an unreachable host.
        Ok(addresses) if addresses.is_empty() => {
            return failure(target, ProbeErrorKind::Dns);
        }
        Ok(addresses) => addresses,
        Err(kind) => return failure(target, kind),
    };

    // A host can resolve to several addresses. The first one that connects
    // wins, and the reported latency covers the whole attempt rather than just
    // the winning attempt, so a target with a slow first address cannot look
    // fast.
    let mut last_error = ProbeErrorKind::Unreachable;

    for address in addresses {
        let remaining = deadline.saturating_sub(started.elapsed());

        match network.attempt(address, remaining).await {
            Ok(()) => return success(target, started),
            Err(kind) => {
                // The deadline outranks whatever the socket reported. Once the
                // budget is spent, the probe timed out, whatever the refusal or
                // reset said on the way out.
                if started.elapsed() >= deadline {
                    return failure(target, ProbeErrorKind::Timeout);
                }
                last_error = kind;
            }
        }
    }

    failure(target, last_error)
}

fn success(target: &ProbeTargetConfig, started: Instant) -> ProbeOutcome {
    ProbeOutcome {
        target_id: target.id,
        captured_at: now_ms(),
        success: true,
        latency_ms: Some(started.elapsed().as_secs_f64() * 1_000.0),
        error_kind: None,
    }
}

/// A failed probe carries no latency at all. A partial number would be read as
/// a measurement, which is the one thing this app must never do.
fn failure(target: &ProbeTargetConfig, kind: ProbeErrorKind) -> ProbeOutcome {
    ProbeOutcome {
        target_id: target.id,
        captured_at: now_ms(),
        success: false,
        latency_ms: None,
        error_kind: Some(kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::config::{DEFAULT_INTERVAL_MS, ProbeMethod, TargetKind};
    use std::sync::Mutex;
    use tokio::net::TcpListener;

    /// A target on loopback. Tests never aim at a real host: this app exists to
    /// diagnose bad connections, so a test that depends on the internet fails
    /// most often for the user and least predictably for us.
    fn loopback_target(port: u16) -> ProbeTargetConfig {
        ProbeTargetConfig {
            id: 7,
            name: "Loopback".to_string(),
            kind: TargetKind::External,
            host: "127.0.0.1".to_string(),
            port,
            method: ProbeMethod::Tcp,
            enabled: true,
            interval_ms: DEFAULT_INTERVAL_MS,
            timeout_ms: 2_000,
        }
    }

    #[tokio::test]
    async fn a_listening_target_is_reachable() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let port = listener.local_addr().expect("local address").port();

        let outcome = probe(&loopback_target(port)).await;

        assert!(outcome.success);
        assert_eq!(outcome.error_kind, None);
        assert_eq!(outcome.target_id, 7);

        let latency = outcome
            .latency_ms
            .expect("a successful probe carries a latency");
        assert!(latency >= 0.0);
        assert!(
            latency < 5_000.0,
            "loopback should not take seconds, got {latency}"
        );
    }

    #[tokio::test]
    async fn a_captured_at_is_a_real_recent_timestamp() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let port = listener.local_addr().expect("local address").port();

        let before = now_ms();
        let outcome = probe(&loopback_target(port)).await;
        let after = now_ms();

        assert!(
            outcome.captured_at >= before && outcome.captured_at <= after,
            "captured_at {} should sit inside [{before}, {after}]",
            outcome.captured_at
        );
    }

    #[tokio::test]
    async fn a_closed_port_is_unreachable_and_carries_no_latency() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let port = listener.local_addr().expect("local address").port();
        // Dropping the listener frees the port, so connecting to it is refused.
        drop(listener);

        let outcome = probe(&loopback_target(port)).await;

        assert!(!outcome.success);
        assert_eq!(outcome.error_kind, Some(ProbeErrorKind::Unreachable));
        assert_eq!(
            outcome.latency_ms, None,
            "a failed probe must never carry a number"
        );
    }

    /// One scripted connection attempt.
    #[derive(Clone, Copy)]
    struct Attempt {
        delay: Duration,
        result: Result<(), ProbeErrorKind>,
    }

    fn attempt(result: Result<(), ProbeErrorKind>) -> Attempt {
        Attempt {
            delay: Duration::ZERO,
            result,
        }
    }

    /// A network that does whatever the test says, so outcomes a real socket
    /// will not produce on demand stay reachable in a test.
    struct FakeNetwork {
        resolve_result: Result<Vec<SocketAddr>, ProbeErrorKind>,
        attempts: Vec<Attempt>,
        tried: Mutex<Vec<SocketAddr>>,
    }

    impl FakeNetwork {
        fn resolving(addresses: Vec<SocketAddr>, attempts: Vec<Attempt>) -> Self {
            FakeNetwork {
                resolve_result: Ok(addresses),
                attempts,
                tried: Mutex::new(Vec::new()),
            }
        }

        fn failing_to_resolve(kind: ProbeErrorKind) -> Self {
            FakeNetwork {
                resolve_result: Err(kind),
                attempts: Vec::new(),
                tried: Mutex::new(Vec::new()),
            }
        }

        fn tried(&self) -> Vec<SocketAddr> {
            self.tried.lock().expect("tried lock").clone()
        }
    }

    impl Network for FakeNetwork {
        async fn resolve(
            &self,
            _host: &str,
            _port: u16,
        ) -> Result<Vec<SocketAddr>, ProbeErrorKind> {
            self.resolve_result.clone()
        }

        async fn attempt(
            &self,
            address: SocketAddr,
            _budget: Duration,
        ) -> Result<(), ProbeErrorKind> {
            let index = {
                let mut tried = self.tried.lock().expect("tried lock");
                tried.push(address);
                tried.len() - 1
            };

            // The last scripted attempt repeats, so a test can script "every
            // address fails this way" without counting addresses first.
            let scripted = self
                .attempts
                .get(index)
                .or_else(|| self.attempts.last())
                .copied()
                .unwrap_or(attempt(Err(ProbeErrorKind::Other)));

            tokio::time::sleep(scripted.delay).await;
            scripted.result
        }
    }

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    #[test]
    fn refused_reset_and_unreachable_sockets_are_unreachable() {
        let unreachable = [
            io::ErrorKind::ConnectionRefused,
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::ConnectionAborted,
            io::ErrorKind::HostUnreachable,
            io::ErrorKind::NetworkUnreachable,
            io::ErrorKind::AddrNotAvailable,
        ];

        for kind in unreachable {
            let error = io::Error::from(kind);
            assert_eq!(
                classify_io_error(&error),
                ProbeErrorKind::Unreachable,
                "{kind:?} should read as unreachable"
            );
        }
    }

    #[test]
    fn an_os_reported_timeout_is_a_timeout() {
        let error = io::Error::from(io::ErrorKind::TimedOut);

        assert_eq!(classify_io_error(&error), ProbeErrorKind::Timeout);
    }

    #[test]
    fn an_unrecognised_error_stays_other_rather_than_being_guessed() {
        let error = io::Error::from(io::ErrorKind::PermissionDenied);

        assert_eq!(classify_io_error(&error), ProbeErrorKind::Other);
    }

    #[tokio::test]
    async fn a_host_that_cannot_be_resolved_is_a_dns_failure() {
        let network = FakeNetwork::failing_to_resolve(ProbeErrorKind::Dns);

        let outcome = probe_once(&network, &loopback_target(1)).await;

        assert_eq!(outcome.error_kind, Some(ProbeErrorKind::Dns));
        assert_eq!(outcome.latency_ms, None);
        assert!(network.tried().is_empty(), "nothing should be attempted");
    }

    #[tokio::test]
    async fn a_host_that_resolves_to_no_addresses_is_a_dns_failure() {
        let network = FakeNetwork::resolving(Vec::new(), Vec::new());

        let outcome = probe_once(&network, &loopback_target(1)).await;

        assert_eq!(
            outcome.error_kind,
            Some(ProbeErrorKind::Dns),
            "an empty answer is a name problem, not an unreachable host"
        );
    }

    #[tokio::test]
    async fn every_failure_kind_produces_a_failed_outcome_with_no_latency() {
        let kinds = [
            ProbeErrorKind::Timeout,
            ProbeErrorKind::Unreachable,
            ProbeErrorKind::Other,
        ];

        for kind in kinds {
            let network = FakeNetwork::resolving(vec![addr(1)], vec![attempt(Err(kind))]);

            let outcome = probe_once(&network, &loopback_target(1)).await;

            assert!(!outcome.success);
            assert_eq!(outcome.error_kind, Some(kind));
            assert_eq!(outcome.latency_ms, None, "{kind:?} carried a number");
        }

        // Dns is covered separately above because it fails before any attempt.
        let network = FakeNetwork::failing_to_resolve(ProbeErrorKind::Dns);
        let outcome = probe_once(&network, &loopback_target(1)).await;
        assert_eq!(outcome.error_kind, Some(ProbeErrorKind::Dns));
        assert_eq!(outcome.latency_ms, None);
    }

    #[tokio::test]
    async fn the_deadline_outranks_the_error_the_socket_reported() {
        // Refused, but only after the budget is already gone.
        let network = FakeNetwork::resolving(
            vec![addr(1)],
            vec![Attempt {
                delay: Duration::from_millis(40),
                result: Err(ProbeErrorKind::Unreachable),
            }],
        );
        let mut target = loopback_target(1);
        target.timeout_ms = 1;

        let outcome = probe_once(&network, &target).await;

        assert_eq!(
            outcome.error_kind,
            Some(ProbeErrorKind::Timeout),
            "a refusal that arrives after the deadline is still a timeout"
        );
        assert_eq!(outcome.latency_ms, None);
    }

    #[tokio::test]
    async fn the_first_reachable_address_wins_and_latency_covers_every_attempt() {
        let network = FakeNetwork::resolving(
            vec![addr(1), addr(2), addr(3)],
            vec![
                Attempt {
                    delay: Duration::from_millis(30),
                    result: Err(ProbeErrorKind::Unreachable),
                },
                Attempt {
                    delay: Duration::from_millis(30),
                    result: Err(ProbeErrorKind::Unreachable),
                },
                attempt(Ok(())),
            ],
        );

        let outcome = probe_once(&network, &loopback_target(3)).await;

        assert!(outcome.success);
        assert_eq!(network.tried(), vec![addr(1), addr(2), addr(3)]);

        let latency = outcome
            .latency_ms
            .expect("a successful probe carries a latency");
        assert!(
            latency >= 60.0,
            "latency must cover the failed attempts, not hide them: got {latency}"
        );
    }

    #[tokio::test]
    async fn the_last_failure_is_reported_when_no_address_answers() {
        let network = FakeNetwork::resolving(
            vec![addr(1), addr(2)],
            vec![
                attempt(Err(ProbeErrorKind::Unreachable)),
                attempt(Err(ProbeErrorKind::Other)),
            ],
        );

        let outcome = probe_once(&network, &loopback_target(2)).await;

        assert_eq!(outcome.error_kind, Some(ProbeErrorKind::Other));
        assert_eq!(outcome.latency_ms, None);
    }
}
