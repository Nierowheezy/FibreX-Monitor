# Feature: Monitoring loop and the first real reading

**From build-plan:** feature 2b
**Status:** verified

## Amendments made during implementation

The user made four scope decisions after this spec was written, while watching
the first build. They are recorded here because the delivered work contradicts
the original scope in five places, and a commit should not claim to be smaller
or narrower than it is.

1. **Health classification is no longer out of scope.** `monitoring/health.rs`
   classifies from the recent probe window. Feature 4 keeps whatever it was
   going to add beyond this: multi-target verdicts, and any weighting of several
   targets into one answer.
2. **The chart is no longer empty.** A rolling in-memory series buffer feeds the
   existing SVG chart. Feature 6 keeps persistence, so history survives a
   restart; until then the window is this launch only and the chart says so.
3. **The panel polls.** The original spec deliberately kept it on focus refresh
   only, on the grounds that a second poller was pointless. The user asked for
   the popover to be live too, so it polls on the same interval and keeps the
   focus refresh so it shows a reading immediately rather than after one tick.
4. **The probe interval dropped from 5s to 2s**, chosen because a number that
   moves every five seconds reads as a stale sample rather than a live one.

The interval change forced the default timeout from 2s to 1.5s, because
`validate()` rejects a timeout that reaches the interval, and equal values would
have made every probe queue behind the previous one instead of measuring.

**Testing amendment.** The spec said no test probes the internet, loopback only.
That remains true of the default gate: `cargo test` is still loopback only. A
single `#[ignore]`d test additionally drives the real loop against the shipped
target, because a scripted double cannot show whether the configuration the app
actually ships with is reachable at all. It is opt-in via `cargo test -- --ignored`.

## Goal

Make the dashboard show a real measurement. A background task probes the
configured target on its interval, holds the latest outcome in memory, and the
overview command reports it. This is the feature where the app stops being an
empty shell.

## Design reference

None. The look is already built; this feature changes what the existing
components render, not how they look.

## Decisions already taken with the user

- **TCP connect**, not ICMP, because macOS does not permit unprivileged ICMP
  sockets. Already built in 2a.
- **The frontend polls** `get_connection_overview` on a timer rather than
  subscribing to Tauri events. One code path, and a response can never show a
  half-updated state such as a new latency beside an old timestamp.
- **A failed probe blanks the latency** back to `No reading` rather than
  holding the last value, because until 2c there is no way to say how old a
  held number is. This is visibly jumpy and 2c replaces it with a proper stale
  marker.

## In scope

- `monitoring/state.rs`: the latest outcome per target behind a mutex, plus
  `probe_all_once` which performs one tick across enabled targets.
- `monitoring/loop.rs`: the tokio task that calls a tick on each target's
  interval, started during app setup and stopped on exit.
- `get_connection_overview` reading that state: real `latency`, real
  `lastProbeAt`, and `monitoringActive` true while the loop runs.
- The frontend polling the command so the dashboard and panel refresh.
- The panel stopping to poll, since it already refreshes on focus and a second
  poller would be pointless.

## Out of scope

Superseded by the amendments above where they conflict; the rest still holds.

- **Packet loss and staleness.** 2c. `packetLossWindowMs` stays `null` and
  `ValueState::Stale` is still never constructed.
- **Multiple targets and aggregation.** 2d. The loop probes the one enabled
  target from the compiled-in config, and the overview names that target as
  primary rather than combining targets.
- **Pause, resume, and running while the window is closed.** Feature 9. The loop
  runs for the life of the process and nothing here changes that.
- **Persistence.** Feature 5. Samples live in memory only, so the series is
  cleared on restart.
- **Health verdicts across several targets.** Feature 4, narrowed by the first
  amendment to what this feature does not attempt.
- **Stored chart history.** Feature 6.
- **Router targets.** Feature 3.

## Build steps

- [x] **Step 1 - Latest-sample store and one tick** - `state.rs` holding the
  latest `ProbeOutcome` per target, and `probe_all_once` probing every enabled
  target once through the injected `Network`. *Done when:* `cargo test` passes
  with tests proving a successful probe is stored with its latency and capture
  time, a failed probe is stored with no latency and its error kind, and a
  target with a disabled flag is never probed.
  *Also:* two tests against a real loopback listener and a real closed port,
  because a scripted double cannot show that the socket path works.

- [x] **Step 2 - The loop task** - `runner.rs` ticking on the configured
  interval, started in `setup` and stopped on exit. *Done when:* the app starts,
  probes without blocking startup, stops cleanly on quit rather than leaking the
  task, and `cargo test` passes for the tick-scheduling logic with a fake clock
  rather than a real interval wait.
  *Two defects found and fixed here.* `loop` is a Rust keyword, so the module is
  `runner`. And `abort()` kills the task before its own cleanup runs, leaving
  `monitoringActive` stuck true, so the handle clears the flag itself; a test
  caught it and it would have made a dead loop look alive.

- [x] **Step 3 - The overview command reports real values** -
  `get_connection_overview` reading the store for `latency`, `lastProbeAt`,
  `monitoringActive` and `healthState`, plus `get_latency_series` for the chart.
  *Done when:* the six existing Rust tests are updated or extended rather than
  deleted, a test proves a stored success produces a `measured` latency with its
  `measuredAt`, a stored failure produces an `unavailable` latency with no
  number, and an empty store still produces the honest first-run payload.
  *One startup crash fixed here:* `tokio::spawn` in `setup` panicked with "no
  reactor running", because Tauri's setup hook runs outside the async runtime.

- [x] **Step 4 - The frontend polls** - `useConnectionOverview` taking a poll
  interval, the dashboard polling while visible, and the panel polling too per
  the third amendment. *Done when:* `npm run build` passes, and the dashboard
  shows a real latency within one poll of the app starting.

- [x] **Step 5 - Live chart and stable axes** - added by the amendments, since
  polling a value nothing displays is not a live monitor. *Done when:* the chart
  accumulates across ticks, gaps stay gaps, the reveal animation does not re-run
  on every appended point, the axis does not rescale on every poll, and a lone
  reading shows the empty state rather than a bare axis.

## Files / areas

| Area | Files |
| --- | --- |
| Engine | `monitoring/state.rs`, `monitoring/runner.rs`, `monitoring/health.rs`, `monitoring/testing.rs` (new), `monitoring/mod.rs`, `monitoring/config.rs` |
| Models | `models/latency.rs` (new), `models/mod.rs` |
| Command | `src-tauri/src/commands/overview.rs`, `src-tauri/src/lib.rs` |
| Frontend | `hooks/useConnectionOverview.ts`, `hooks/useLatencySeries.ts` (new), `features/overview/OverviewScreen.tsx`, `panel/PanelScreen.tsx`, `components/chart/LatencyChart.tsx`, `lib/tauri/overview.ts` |
| Unchanged | `src/styles/global.css`, `prototypes/` |

`monitoring/testing.rs` holds the scripted network double, extracted because both
`state.rs` and `runner.rs` needed it and each would otherwise grow its own.

## Data / contracts

`ConnectionOverview` does not change shape. What changes is that its fields stop
being constant:

| Field | First run | With a successful probe | With a failed probe |
| --- | --- | --- | --- |
| `monitoringActive` | `true` while the loop runs | same | same |
| `lastProbeAt` | `null` | capture time of the last **successful** probe | unchanged |
| `latency` | `unavailable`, null | `measured`, value, `measuredAt` | `unavailable`, null |
| `healthState` | `paused` | `connected` | `offline`, or `degraded` when the window is mixed |

`healthState` moved out of the "deliberately stays unknown" position the original
spec held, by the first amendment. Two rules constrain what it may say:

- **`paused` overrides everything.** If the loop is not running the app has no
  current opinion, and reporting `connected` from a reading taken before the loop
  died would present a stale claim as a live one. This is also the state that
  makes a crashed loop visible instead of silent.
- **It describes our probes and nothing else.** No rule here inspects the
  router, knows an ISP exists, or names a cause. The verdict is what this app's
  own probes saw.

`ConnectionOverview` does not change shape. The series is a **separate command**,
`get_latency_series`, rather than another field on the overview, so a growing
array of points never rides along on the snapshot every poll reads.

**Schema amendment for feature 5.** `probe_sample.monitoring_state` is filled at
probe time now that health is classified, so the column can be non-null. If 2c
later proves a sample can be stored without a verdict, it becomes nullable then;
feature 5 should not create it nullable for a case that does not exist yet.

## Testing

- `cargo test` is the gate. Steps 1 and 2 are pure logic and ship their tests in
  the same diff.
- **The tick is separated from the scheduling on purpose.** `probe_all_once` is
  a plain function the tests drive directly through the injected `Network`; the
  loop only decides when to call it. A test that waits a real interval to prove a
  tick happened would be slow and flaky, and this app is used precisely on the
  machines where timing is least reliable. The loop takes its interval as a
  parameter so the scheduling runs on a ten-millisecond clock in tests.
- **`cargo test` probes no network beyond loopback.** One opt-in test reaches the
  real internet, per the testing amendment above.
- `npm run build` after Step 4.
- `cargo clippy --all-targets` clean at every step.
- **The native window has not been verified by eye.** `screencapture` returns
  wallpaper only, because Screen Recording permission is not granted on this
  machine. The backend was proven against a real socket and a real network, and
  the dashboard was driven in a browser, but the two were never observed together
  in the running app.

## Notes for the AI

- Never turn a failed probe into a number. The store keeps the outcome, the
  command decides what to report, and a failure reports no latency at all.
- `lastProbeAt` tracks the last **successful** probe. Reporting the time of a
  failure as the last probe would make a broken connection look freshly checked.
- The loop must not keep the process alive or block shutdown. Abort or signal it
  on exit and prove it in a test.
- The shared state is read by a synchronous command, so a plain mutex is right.
  The critical section is a map lookup; do not reach for an async lock.
- Reuse the `Network` seam from 2a rather than calling the real network directly,
  or the tick stops being testable.
- Comment the why, not the what. No em dashes, en dashes, or ellipsis characters
  in code, comments, or docs.
## Verification

| Claim | How it was proven |
| --- | --- |
| 57 tests pass | `cargo test` |
| Clippy clean | `cargo clippy --all-targets`, 0 warnings |
| Builds | `npm run build` |
| The probe works against a real socket | Two tests bind a real loopback listener and a real closed port, asserting a positive latency for the first and no number for the second |
| The shipped config produces real readings | `cargo test -- --ignored` drives the real loop and real network against `1.1.1.1:443` for two intervals, asserting the series grows and stays ordered |
| The app starts without crashing | 75 seconds running, no panic. This check is what caught the "no reactor running" panic |
| The dashboard updates live | Driven in a browser: readings climbed 4 to 14, gaps broke the line, the reveal animation did not restart (`strokeDashoffset` stayed 0), health reached Degraded, and the pulse dot correctly disappeared when the newest probe failed |
| No colour outside the token layer | Grep for hex literals across `src`, none outside `src/styles/global.css` |

**Not verified.** The native window has never been observed by eye. `screencapture`
returns wallpaper only on this machine because Screen Recording permission is not
granted, so the tray icon, the absent Dock icon, the menu rows, and the panel's
placement under the icon remain unchecked. The backend was proven against a real
network and the dashboard was proven in a browser, but never observed together in
the running app. Granting Screen Recording permission is the only way to close
this, and `/check` is the command for it.

**Still out of scope.** The series is in memory, so the chart clears on restart
until feature 5 persists it. Probing `1.1.1.1:443` every two seconds is roughly
43,000 connections a day, which is a real cost paid to someone else's server and
is flagged in the code for feature 2d to revisit.
