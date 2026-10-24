//! Test doubles shared by the monitoring modules.
//!
//! A real network cannot be made to produce a timeout or a DNS failure on
//! demand, and this app is used on exactly the machines where the network
//! misbehaves. One scripted double serves every module that needs it, rather
//! than each growing its own.

use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Duration;

use crate::monitoring::probe::{Network, ProbeErrorKind};

/// One scripted connection attempt.
#[derive(Clone, Copy)]
pub struct Attempt {
    pub delay: Duration,
    pub result: Result<(), ProbeErrorKind>,
}

impl Attempt {
    pub fn ok() -> Self {
        Attempt {
            delay: Duration::ZERO,
            result: Ok(()),
        }
    }

    pub fn failing(kind: ProbeErrorKind) -> Self {
        Attempt {
            delay: Duration::ZERO,
            result: Err(kind),
        }
    }

    pub fn after(delay: Duration, result: Result<(), ProbeErrorKind>) -> Self {
        Attempt { delay, result }
    }
}

pub struct FakeNetwork {
    resolve_result: Result<Vec<SocketAddr>, ProbeErrorKind>,
    attempts: Vec<Attempt>,
    tried: Mutex<Vec<SocketAddr>>,
}

impl FakeNetwork {
    /// Resolves to the given addresses and answers attempts in order. The last
    /// scripted attempt repeats, so a test can say "every address fails this
    /// way" without counting addresses first.
    pub fn resolving(addresses: Vec<SocketAddr>, attempts: Vec<Attempt>) -> Self {
        FakeNetwork {
            resolve_result: Ok(addresses),
            attempts,
            tried: Mutex::new(Vec::new()),
        }
    }

    pub fn failing_to_resolve(kind: ProbeErrorKind) -> Self {
        FakeNetwork {
            resolve_result: Err(kind),
            attempts: Vec::new(),
            tried: Mutex::new(Vec::new()),
        }
    }

    /// The addresses actually attempted, in order.
    pub fn tried(&self) -> Vec<SocketAddr> {
        self.tried.lock().expect("tried lock").clone()
    }

    pub fn attempt_count(&self) -> usize {
        self.tried.lock().expect("tried lock").len()
    }
}

impl Network for FakeNetwork {
    async fn resolve(&self, _host: &str, _port: u16) -> Result<Vec<SocketAddr>, ProbeErrorKind> {
        self.resolve_result.clone()
    }

    async fn attempt(&self, address: SocketAddr, _budget: Duration) -> Result<(), ProbeErrorKind> {
        let index = {
            let mut tried = self.tried.lock().expect("tried lock");
            tried.push(address);
            tried.len() - 1
        };

        let scripted = self
            .attempts
            .get(index)
            .or_else(|| self.attempts.last())
            .copied()
            .unwrap_or_else(|| Attempt::failing(ProbeErrorKind::Other));

        tokio::time::sleep(scripted.delay).await;
        scripted.result
    }
}

pub fn loopback(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}
