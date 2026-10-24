# Feature: Probe configuration and probe primitives

**From build-plan:** feature 2a
**Status:** verified

## Goal

Give the monitoring engine the two things every later piece of connectivity
monitoring depends on: a validated description of what to probe, and a single
probe that answers "is this reachable, and how long did it take".

This ships no user-visible change. The dashboard keeps showing its honest
first-run state, because a probe result that nothing displays yet is worth less
than a probe result with a defined contract and tests around it.

## In scope

- A probe configuration type shaped to match the locked `probe_target` and
  `settings` rows in `project-overview.md`, so feature 5 persists it unchanged.
- Compiled-in default configuration plus validation that rejects configuration
  that would be harmful rather than merely wrong.
- A single-shot TCP connect probe returning reachability, latency in
  milliseconds, and a failure kind.
- Rust unit tests covering every success and failure path without depending on
  the user's internet connection.

## Out of scope

- **The monitoring loop.** Nothing schedules or repeats probes. That is 2b,
  along with exposing a real latency through `get_connection_overview`.
- **Any new Tauri command.** 2a is engine-internal, so the public IPC surface
  stays owned by 2b rather than growing a second shape to undo.
- **Packet loss and staleness.** Both are 2c. `packet_loss_window_ms` stays
  `null` and `ValueState::Stale` stays unconstructed.
- **Multiple targets and aggregation.** 2a defines the fields but enables
  exactly one external target. 2d decides how several combine.
- **ICMP.** macOS does not permit unprivileged ICMP sockets, so `ProbeMethod` has
  a single `Tcp` variant in this feature. Adding a method later must not change
  `ProbeOutcome`.
- **Router targets.** `TargetKind::Router` exists in the shape so feature 5 can
  store it, but nothing constructs one. Feature 3 owns router probing.
- **Persisting or editing configuration.** Feature 5 owns storage, feature 16
  owns the settings UI. Configuration is compiled in here.
- **Any frontend change.** No component, token, or command changes.

## Build loop

Build one step at a time, never the whole feature at once.

1. Plan mode lays out the step before any code.
2. The AI implements just that step.
3. It shows the diff (not full files); you read it and understand it.
4. You approve, then choose whether to commit a checkpoint or roll straight on.
   Checkpoints are optional; `/complete` makes the real feature-level commit.

Never accept a step you have not read. If a diff is too big to review, the step
was too big, so split it.

## Build steps

- [x] **Step 1 - Probe configuration types, defaults, and validation** -
  `src-tauri/src/monitoring/config.rs` with `ProbeTargetConfig`,
  `ProbeSettings`, `TargetKind`, `ProbeMethod`, a `validate` that returns the
  reasons configuration is unusable, and the compiled-in default settings (one
  enabled external TCP target, 5s interval, 2s timeout). *Done when:*
  `cargo check` and `cargo clippy` pass, and `cargo test config` passes with one
  test per validation rule plus a test asserting the compiled-in defaults are
  themselves valid, so a bad default can never ship.

- [x] **Step 2 - TCP connect probe** - `src-tauri/src/monitoring/probe.rs` with
  `probe_once` resolving the host, timing a TCP connect to the configured port
  under the configured timeout, and returning `ProbeOutcome`. The connector is
  reached through a small injected seam so failure paths are testable without a
  network. *Done when:* `cargo test probe` passes with a test that binds a
  `TcpListener` on loopback, probes it, and asserts `success` with a latency and
  no error kind, and a test that probes a closed loopback port and asserts
  failure with no latency and `Unreachable`.

- [x] **Step 3 - Failure classification** - the taxonomy that turns an IO
  error into `Timeout`, `Unreachable`, `Dns`, or `Other`, including the rule
  that a connect which outlives its deadline is a timeout regardless of the
  error the socket reports. *Done when:* `cargo test` passes with a test per
  failure kind, a test that an elapsed deadline yields `Timeout` even when the
  socket error would otherwise read as unreachable, and a test that a failed
  probe always leaves `latency_ms` as `None` so no partial number can be shown
  as a measurement.

## Files / areas

| Area | Files |
| --- | --- |
| Engine | `src-tauri/src/monitoring/mod.rs`, `src-tauri/src/monitoring/config.rs`, `src-tauri/src/monitoring/probe.rs` |
| Manifest | `src-tauri/Cargo.toml` (direct `tokio` dependency with `net`, `time`, `rt`, `macros`) |
| Wiring | `src-tauri/src/lib.rs` gains `mod monitoring;` only, and registers no new command |
| Unchanged | nothing under `src/` changes |

`models/` stays the home of wire types. `monitoring/` is engine-internal and
gains no serde derives until 2b decides what crosses the IPC boundary.

## Data / contracts

```rust
pub enum TargetKind { Router, External }

pub enum ProbeMethod { Tcp }

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

pub struct ProbeSettings {
    pub targets: Vec<ProbeTargetConfig>,
    pub default_interval_ms: u64,
    pub default_timeout_ms: u64,
}

pub enum ProbeErrorKind { Timeout, Unreachable, Dns, Other }

pub struct ProbeOutcome {
    pub target_id: i64,
    pub captured_at: i64,             // UTC epoch ms
    pub success: bool,
    pub latency_ms: Option<f64>,      // milliseconds, None whenever success is false
    pub error_kind: Option<ProbeErrorKind>, // None whenever success is true
}
```

`ProbeOutcome` is exactly the `probe_sample` row from `project-overview.md`
minus storage, minus `monitoring_state`. That is deliberate: feature 5 persists
these outcomes without reshaping them.

**Load-bearing.** Features 2b, 2c, 2d, 5, 6, and 7 all build on these types.
`latency_ms` is `f64` to match the existing `MetricValue<f64>` contract, so
nothing has to convert units later.

**Contract note for 2b, not for this feature.** `probe_sample` also carries
`monitoring_state`, the health state at capture time. 2a does not stamp it,
because health classification is feature 4. 2b must decide where that stamp is
captured so feature 5 does not have to retrofit it onto stored rows.

### Validation rules

Each rule has its own test.

| Rule | Reason |
| --- | --- |
| `host` is non-empty after trimming | A blank host cannot be probed |
| `port` is 1 to 65535 | Anything else is not a port |
| `interval_ms >= 1000` | Sub-second probing is abusive and measures nothing useful |
| `timeout_ms >= 100` | A timeout below this reports timeouts rather than results |
| `timeout_ms < interval_ms` | Otherwise a probe is still running when the next one starts |
| At least one enabled target | Otherwise monitoring runs and can never measure anything |

### Default configuration

One external TCP target, `1.1.1.1:443`, named "Internet", enabled, with a 5s
interval and a 2s timeout. Feature 2d replaces this with a considered list.

## Testing

- **`cargo test` is the gate** for this feature, declared in `AGENTS.md`. This
  feature is pure logic with no UI, so every step ships its tests in the same
  diff and the suite must be green before the step is approved.
- **No test may touch the user's internet.** Tests use loopback only, and DNS,
  timeout, and generic failure paths are driven through the injected connector
  seam rather than by aiming at an address that happens to be unreachable. A
  test that depends on a black-hole IP is slow on a good connection and flaky on
  a bad one, which is exactly the environment this app exists to diagnose.
- **Resolution rule to test:** when a host resolves to several addresses, each is
  tried in turn and the first success wins, with the elapsed time covering the
  whole attempt. No test may resolve a real hostname.
- **Gate order per step:** `npm run build` (unchanged frontend must stay green),
  then `cargo check`, then `cargo test`, then `cargo clippy`. There is no
  `Verify` command declared in `AGENTS.md` yet; `/ci` would define one later.
- **Regression check:** because the frontend does not change, confirm
  `npm run tauri dev` still shows the unchanged first-run overview. A dashboard
  that quietly starts showing fabricated numbers would be the worst possible
  outcome here.
- **No browser test harness** is configured and none is added. Nothing in this
  feature is user-visible.

## Notes for the AI

- Rust is the single source of truth for measurement. React never probes, and
  2a adds no command, so there is nothing for the frontend to call yet.
- Never turn a failed probe into a number. A failed probe has no latency, and
  `0` is never a stand-in for missing. This is the same rule feature 1 enforced
  in the UI, applied at the source.
- Add `tokio` as a direct dependency with `net`, `time`, `rt`, and `macros`.
  Tauri already ships a tokio runtime, so this adds no second runtime to the
  binary; it only makes the types nameable from our own code.
- The injected connector seam is deliberate and small. Without it the timeout and
  DNS paths can only be tested against the network, which is untestable in CI and
  unreliable on a laptop.
- Do not add `Icmp` or `Http` variants to `ProbeMethod` "for completeness".
  Dead variants in a contract are a promise nobody has verified.
- Do not read or write any settings file. Compiled-in defaults are the whole
  configuration story until feature 5.
- The compiled-in defaults are a placeholder, not a considered target list.
  Say so in the commit message so feature 2d does not inherit them as a decision.
- Comment the why, not the what. No em dashes, en dashes, or ellipsis characters
  in code, comments, or docs.