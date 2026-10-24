//! What to probe, and the rules that decide whether a configuration is usable.
//!
//! The shapes mirror the locked `probe_target` and `settings` rows in
//! project-overview.md so feature 5 can persist them without reshaping.

/// Shortest interval the app accepts. Probing faster than this is abusive to
/// the target and measures nothing a person would act on.
pub const MIN_INTERVAL_MS: u64 = 1_000;

/// Shortest timeout that can produce a result. Below this, a probe reports a
/// timeout instead of an answer.
pub const MIN_TIMEOUT_MS: u64 = 100;

/// Interval used by the compiled-in defaults. Two seconds is what makes the
/// reading on screen feel live rather than sampled. It is a real cost paid to
/// someone else's server, so feature 2d revisits it against what should
/// actually be probed rather than leaving it arbitrary.
pub const DEFAULT_INTERVAL_MS: u64 = 2_000;

/// Timeout used by the compiled-in defaults. Deliberately under the interval:
/// a timeout that reached the interval would still be running when the next
/// probe is due, and probes would queue instead of measuring.
pub const DEFAULT_TIMEOUT_MS: u64 = 1_500;

/// Which side of the router a target sits on. 2a constructs only `External`;
/// feature 3 owns router probing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Router,
    External,
}

/// How a target is measured. macOS does not permit unprivileged ICMP sockets
/// the way Linux does, so latency and reachability come from a TCP connect.
/// A second method would change what an outcome means, so it earns its own
/// feature rather than a variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeMethod {
    Tcp,
}

impl ProbeMethod {
    /// The word the interface shows for this method. Kept here rather than in
    /// the UI so a second method cannot be added without deciding how it is
    /// named to a reader.
    pub fn label(self) -> &'static str {
        match self {
            ProbeMethod::Tcp => "TCP",
        }
    }
}

/// One thing to probe, and how hard to try.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeTargetConfig {
    pub id: i64,
    pub name: String,
    pub kind: TargetKind,
    pub host: String,
    pub port: u16,
    pub method: ProbeMethod,
    pub enabled: bool,
    pub interval_ms: u64,
    pub timeout_ms: u64,
}

impl ProbeTargetConfig {
    /// Every reason this target cannot be probed, not just the first. A settings
    /// screen is more useful when it can show all of them at once.
    pub fn validate(&self) -> Vec<ConfigIssue> {
        let mut issues = Vec::new();

        if self.host.trim().is_empty() {
            issues.push(ConfigIssue::EmptyHost { target_id: self.id });
        }

        // u16 makes the upper bound unrepresentable, so zero is the only
        // unusable port.
        if self.port == 0 {
            issues.push(ConfigIssue::InvalidPort {
                target_id: self.id,
                port: self.port,
            });
        }

        if self.interval_ms < MIN_INTERVAL_MS {
            issues.push(ConfigIssue::IntervalTooShort {
                target_id: self.id,
                interval_ms: self.interval_ms,
            });
        }

        if self.timeout_ms < MIN_TIMEOUT_MS {
            issues.push(ConfigIssue::TimeoutTooShort {
                target_id: self.id,
                timeout_ms: self.timeout_ms,
            });
        }

        // A timeout at or beyond the interval means the next probe starts while
        // this one is still running, which piles up work instead of measuring.
        if self.timeout_ms >= self.interval_ms {
            issues.push(ConfigIssue::TimeoutNotBeforeInterval {
                target_id: self.id,
                timeout_ms: self.timeout_ms,
                interval_ms: self.interval_ms,
            });
        }

        issues
    }
}

/// The whole configuration: what to probe, plus the values a new target starts
/// from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeSettings {
    pub targets: Vec<ProbeTargetConfig>,
    pub default_interval_ms: u64,
    pub default_timeout_ms: u64,
}

impl ProbeSettings {
    /// Every reason this configuration cannot be used as written.
    pub fn validate(&self) -> Vec<ConfigIssue> {
        let mut issues: Vec<ConfigIssue> = self
            .targets
            .iter()
            .flat_map(|target| target.validate())
            .collect();

        // Monitoring that runs with nothing enabled would look alive forever
        // while measuring nothing, which is the failure this app exists to
        // prevent.
        if !self.targets.iter().any(|target| target.enabled) {
            issues.push(ConfigIssue::NoEnabledTarget);
        }

        issues
    }

    pub fn enabled_targets(&self) -> impl Iterator<Item = &ProbeTargetConfig> {
        self.targets.iter().filter(|target| target.enabled)
    }

    /// The target the single overview and chart report on.
    ///
    /// An external target is preferred so the router probe cannot hijack the
    /// headline reading, which is what a naive "first enabled target" would do
    /// the moment the router is listed before the internet. With several
    /// external targets this is still the first, which is a placeholder:
    /// feature 2d decides what should actually be probed, and feature 4 decides
    /// what a verdict across several targets even means.
    pub fn primary_target_id(&self) -> Option<i64> {
        self.enabled_targets()
            .find(|target| target.kind == TargetKind::External)
            .or_else(|| self.enabled_targets().next())
            .map(|target| target.id)
    }

    /// The enabled router target, if one exists. Feature 3 probes the local
    /// router separately so a local fault and an upstream fault can be told
    /// apart, and this is how the loop finds it without a second configuration
    /// path.
    pub fn router_target(&self) -> Option<&ProbeTargetConfig> {
        self.enabled_targets()
            .find(|target| target.kind == TargetKind::Router)
    }

    /// The whole target the headline reading comes from, so the interface can
    /// name what a latency is a measurement of instead of showing a bare number.
    pub fn primary_target(&self) -> Option<&ProbeTargetConfig> {
        let id = self.primary_target_id()?;
        self.targets.iter().find(|target| target.id == id)
    }
}

/// One reason a configuration is unusable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigIssue {
    EmptyHost {
        target_id: i64,
    },
    InvalidPort {
        target_id: i64,
        port: u16,
    },
    IntervalTooShort {
        target_id: i64,
        interval_ms: u64,
    },
    TimeoutTooShort {
        target_id: i64,
        timeout_ms: u64,
    },
    TimeoutNotBeforeInterval {
        target_id: i64,
        timeout_ms: u64,
        interval_ms: u64,
    },
    NoEnabledTarget,
}

/// The compiled-in configuration. A deliberately small target list: choosing
/// what FibreX Monitor should watch in full is feature 2d's decision, and this
/// keeps 2a from inheriting an arbitrary host as if it had been chosen.
///
/// Two targets, and the split is the point: an external target that reports on
/// the internet, and the local router, which feature 3 probes so a local fault
/// and an upstream fault can be told apart. The router is reached by address, so
/// the probe works whether or not the router is on its usual address and fails
/// honestly when the Mac is on a different network.
pub fn default_settings() -> ProbeSettings {
    ProbeSettings {
        targets: vec![external_target(), router_target()],
        default_interval_ms: DEFAULT_INTERVAL_MS,
        default_timeout_ms: DEFAULT_TIMEOUT_MS,
    }
}

/// The external target, used by 2a and the one the overview reports on.
fn external_target() -> ProbeTargetConfig {
    ProbeTargetConfig {
        id: 1,
        name: "Internet".to_string(),
        kind: TargetKind::External,
        host: "1.1.1.1".to_string(),
        port: 443,
        method: ProbeMethod::Tcp,
        enabled: true,
        interval_ms: DEFAULT_INTERVAL_MS,
        timeout_ms: DEFAULT_TIMEOUT_MS,
    }
}

/// The local router, probed for reachability only.
///
/// Port 80 is the one the router actually answers on. Port 443 is closed on the
/// observed firmware; the router forces HTTPS but serves it on 80, so a connect
/// check uses 80 and does not follow the redirect. This is a reachability probe
/// and needs no session, which is exactly why it can ship before router
/// authentication exists.
fn router_target() -> ProbeTargetConfig {
    ProbeTargetConfig {
        id: 2,
        name: "Router".to_string(),
        kind: TargetKind::Router,
        host: "192.168.100.1".to_string(),
        port: 80,
        method: ProbeMethod::Tcp,
        enabled: true,
        interval_ms: DEFAULT_INTERVAL_MS,
        timeout_ms: DEFAULT_TIMEOUT_MS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_in_defaults_are_valid() {
        assert_eq!(default_settings().validate(), Vec::new());
    }

    #[test]
    fn a_blank_host_is_rejected() {
        let mut target = external_target();
        target.host = "   ".to_string();

        assert_eq!(
            target.validate(),
            vec![ConfigIssue::EmptyHost { target_id: 1 }]
        );
    }

    #[test]
    fn a_zero_port_is_rejected() {
        let mut target = external_target();
        target.port = 0;

        assert_eq!(
            target.validate(),
            vec![ConfigIssue::InvalidPort {
                target_id: 1,
                port: 0
            }]
        );
    }

    #[test]
    fn an_interval_below_the_floor_is_rejected() {
        let mut target = external_target();
        // The timeout is lowered too, otherwise it would also exceed the
        // shortened interval and this test would prove two rules at once.
        target.interval_ms = MIN_INTERVAL_MS - 1;
        target.timeout_ms = MIN_TIMEOUT_MS;

        assert_eq!(
            target.validate(),
            vec![ConfigIssue::IntervalTooShort {
                target_id: 1,
                interval_ms: MIN_INTERVAL_MS - 1
            }]
        );
    }

    #[test]
    fn a_timeout_below_the_floor_is_rejected() {
        let mut target = external_target();
        target.timeout_ms = MIN_TIMEOUT_MS - 1;

        assert_eq!(
            target.validate(),
            vec![ConfigIssue::TimeoutTooShort {
                target_id: 1,
                timeout_ms: MIN_TIMEOUT_MS - 1
            }]
        );
    }

    #[test]
    fn a_timeout_beyond_its_interval_is_rejected() {
        let mut target = external_target();
        target.timeout_ms = target.interval_ms;

        assert_eq!(
            target.validate(),
            vec![ConfigIssue::TimeoutNotBeforeInterval {
                target_id: 1,
                timeout_ms: DEFAULT_INTERVAL_MS,
                interval_ms: DEFAULT_INTERVAL_MS,
            }]
        );
    }

    #[test]
    fn settings_without_an_enabled_target_are_rejected() {
        let mut settings = default_settings();
        for target in &mut settings.targets {
            target.enabled = false;
        }

        assert_eq!(settings.validate(), vec![ConfigIssue::NoEnabledTarget]);
    }

    #[test]
    fn every_problem_is_reported_not_just_the_first() {
        let mut settings = default_settings();
        settings.targets = vec![external_target()];
        settings.targets[0].host = String::new();
        settings.targets[0].port = 0;
        settings.targets[0].enabled = false;

        assert_eq!(
            settings.validate(),
            vec![
                ConfigIssue::EmptyHost { target_id: 1 },
                ConfigIssue::InvalidPort {
                    target_id: 1,
                    port: 0
                },
                ConfigIssue::NoEnabledTarget,
            ]
        );
    }

    #[test]
    fn enabled_targets_excludes_disabled_ones() {
        let mut settings = default_settings();
        let mut disabled = external_target();
        disabled.id = 3;
        disabled.enabled = false;
        settings.targets.push(disabled);

        let enabled: Vec<i64> = settings.enabled_targets().map(|t| t.id).collect();
        assert_eq!(enabled, vec![1, 2]);
    }

    #[test]
    fn the_primary_target_is_external_even_when_the_router_is_listed_first() {
        let mut settings = default_settings();
        // Router first, external second. The headline must still be the internet,
        // because a router probe only says whether the local network answered.
        settings.targets.reverse();

        assert_eq!(settings.primary_target_id(), Some(1));
    }

    #[test]
    fn the_router_target_is_named_by_kind_not_by_position() {
        let mut settings = default_settings();
        settings.targets.reverse();

        assert_eq!(
            settings.router_target().map(|target| target.host.as_str()),
            Some("192.168.100.1")
        );
    }
}
