# Feature: Packet loss and freshness

**From build-plan:** feature 2c
**Status:** verified

## Goal

Two things are missing from a monitor that is now measuring for real.

- **No failure figure.** The dashboard shows latency and nothing about how often
  probes fail, which is the number a user actually wants when the connection is
  misbehaving.
- **Latency blinks.** A failed probe blanks the reading to `No reading` with no
  way to say how old the last good number was. That was accepted in 2b as
  visibly jumpy, and 2b's spec committed to replacing it here.

This computes the failure rate across the probe window and introduces the
freshness rule, so a held reading can say how old it is instead of vanishing.

## Design reference

`prototypes/dashboard.html` (the metric tile) and `prototypes/panel.html` (the
popover row) are the layout reference for both surfaces. The tokens were ported
into `src/styles/global.css` by 8b, so this feature ports nothing: it changes
what existing components render, not how they look.

Two deliberate departures from the mockups, both visible copy changes:

- The tile label becomes **Probe failures**, not "Packet loss". See the decisions.
- The tile foot becomes **Last 5 minutes**, not the mockup's "Last 60 seconds",
  because the window is 5 minutes.

## Decisions taken with the user

- **Named "Probe failures", not "packet loss".** macOS denies unprivileged ICMP
  sockets, so 2a measures a TCP connect. **A failed connect is not a lost
  packet**, and a headline reading "12% packet loss" would be a measurement this
  app never took. The tile shows `3 of 25 probes failed`, which cannot be
  misread as a packet-level claim even if the label is skimmed. The wire field
  keeps its `packetLoss` name because features 6, 7 and 17 consume it, but its
  meaning is now locked as **the percentage of probe attempts in the window that
  did not connect**. Nothing in this app may call it packet loss again.

- **The window is 5 minutes.** That is the existing `SERIES_WINDOW_MS`, already
  the window the chart draws and the health verdict judges. One window means the
  three figures on screen always describe the same stretch of time and none of
  them needs a buffer of its own. 60 seconds was considered and rejected: at a
  2 second interval that is 30 samples, so a single failure reads as 3.3% and
  the figure jitters constantly.

- **A failed probe holds the last good reading and marks it stale** instead of
  blanking it. 2b's spec committed to this: blanking was "visibly jumpy and 2c
  replaces it with a proper stale marker".

## In scope

- **Read-time window filtering** in `MonitorState`, which is a bug fix rather
  than a feature. See the note below; it is load-bearing for everything else.
- Failure counts and percentage over the window, for the primary target.
- The freshness rule resolving latency to `measured`, `stale` or `unavailable`.
- The overview command reporting all three, plus a real `packetLossWindowMs`.
- Dashboard tile and panel row copy, and the stale reading's age.
- Relabelling `lastProbeAt` in the UI, because it tracks the last **successful**
  probe and the label "Last probe" implies the last attempt.

## Out of scope

- **Thresholds, and what counts as degraded.** Feature 4. This feature computes
  a figure and displays it. It must not compare that figure against a threshold,
  because the `settings` table does not exist until feature 5 and there is no
  configured value to compare against.
- **Persistence.** Feature 5. The window is in memory and clears on restart.
- **Multiple targets.** Feature 2d. Everything here is the primary target only.
- **A chart of loss in its own right.** Feature 6, which reads it from storage.
- **Incident detection.** Feature 7.
- **ICMP.** Not available on macOS without privileges, so genuinely measuring
  packet loss would mean raw sockets and a privilege escalation. That is not a
  change this app should make to itself, and it stays out of scope permanently
  unless the user decides otherwise.

## The defect this feature must fix first

`MonitorState` evicts old samples only inside `record()`, which runs when a probe
completes. **A paused or stalled loop therefore never evicts anything**, so its
buffer keeps serving readings that are minutes or hours old as though they were
current. Read `healthy`, the app would show a stale number indefinitely.

The window must be applied when reading, against the current clock, not only when
writing. Eviction on write stays as a memory bound, but correctness comes from the
read filter. Every query that reports on the window takes the clock into account,
and this is what makes the staleness rule honest rather than decorative.

## Build steps

- [x] **Step 1 - Apply the window at read time** - extract the one shared
  `now_ms()` (2b's `probe.rs` has a private copy and this feature needs a second,
  so it becomes one shared helper rather than two) and filter the series by the
  current clock on every read. *Done when:* `cargo test` proves that a sample
  older than the window is excluded from a read even when no new probe has been
  recorded since it aged out, and that a freshly recorded sample still appears.

- [x] **Step 2 - Count the failures** - a failure summary over the window for the
  primary target, returning failed count, total count and the percentage.
  *Done when:* tests prove zero samples yields `None` rather than `0%`, all
  successes yields a genuine `0.0` percent, a mixed window yields the right
  fraction, and samples outside the window are excluded from both counts.

- [x] **Step 3 - The overview command reports both** - `packetLoss`,
  `packetLossWindowMs` and the failure basis filled for real, and the freshness
  rule resolving latency to `measured`, `stale` or `unavailable`. *Done when:*
  tests prove a newest-success reports `measured` with no age caveat, a
  newest-failure with an older success in the window reports `stale` carrying the
  **older** number and its own capture time, a window with no success at all
  reports `unavailable`, and `ValueState::Stale` is constructed for the first
  time in this codebase.

- [x] **Step 4 - Show it honestly** - the tile and the panel row render the
  failure count over the total, the tile foot names the real window, a stale
  reading shows its age, and the existing overview contract tests are extended
  rather than deleted. This step is more than copy, because the current tile
  cannot express the new figure:
  - `MetricDefinition.format` becomes
    `(value: number, basis: ProbeFailureBasis | null) => string`, and
    `MetricGrid` passes `overview.probeFailureBasis` to every tile. The latency
    and rate formatters ignore the second argument. Without this the counts
    cannot reach the formatter at all.
  - `MetricTile` gains an optional `basis` prop and passes it through. Its
    `stale` rendering is already correct from 2b's design and needs no change.
  - `formatPercent` is **replaced** by `formatProbeFailures`, not kept beside it.
    Nothing else uses it, and a dead export invites someone to reach for the one
    that renders a bare percentage.
  - A new shared `formatAge(ms)` renders "just now", "40s ago", "12m ago", used
    by both the stale tile foot and the hero card.
  *Done when:* `npm run build` passes, the dashboard shows `3 of 25` with
  `failed` beneath and `Last 5 minutes` in the foot, the panel row shows the
  same figure rather than a percentage, and both surfaces agree exactly.

## Files / areas

| Area | Files |
| --- | --- |
| Store | `src-tauri/src/monitoring/state.rs`, `src-tauri/src/monitoring/mod.rs` (shared clock) |
| Models | `src-tauri/src/models/overview.rs` |
| Command | `src-tauri/src/commands/overview.rs` |
| Frontend | `src/types/overview.ts`, `src/features/overview/format.ts`, `src/features/overview/MetricGrid.tsx`, `src/features/overview/MetricTile.tsx`, `src/features/overview/ConnectionHero.tsx`, `src/features/overview/StatusSummary.tsx`, `src/panel/PanelScreen.tsx` |

`ConnectionHero` is in scope because its foot currently reads "Measured just now"
or "Waiting for the first probe", and neither is true for a stale reading. It has
to show the age, which is the whole point of the feature. `MetricTile` itself
needs only the `basis` pass-through; its `stale` tone, note and pill were built in
2b and have been dormant until now.

## Data / contracts

`packetLoss.value` is a **percentage from 0 to 100**, not a fraction. This is
locked because `formatPercent` already formats a bare number and the panel
already appends `%` itself, and `settings.packet_loss_threshold_pct` is a
percentage. Changing it later would silently rescale every threshold.

One new top-level field, deliberately:

| Field | Type | Meaning |
| --- | --- | --- |
| `packetLoss` | `MetricValue<f64>` | Percentage of attempts in the window that did not connect. `unavailable` when there are no samples |
| `packetLossWindowMs` | `u64`, nullable | The window the figure covers. Now always `300000` while monitoring runs |
| `probeFailureBasis` | `{ failed: u32, total: u32 }`, nullable | The counts behind the percentage |

`probeFailureBasis` exists so the UI can say `3 of 25` without re-deriving the
window and the counting rule in TypeScript. A second implementation of that rule
would be free to drift from the Rust one, and the project has already been bitten
by two copies of one formatter. The backend counts; the frontend renders.

**A genuine `0.0` is not the same as unavailable.** Zero failed probes out of 25
is a real measurement and reports `measured`. This is the opposite of latency,
where `0` would be meaningless, and the distinction is why the counts are on the
wire: a reader can see `total > 0` and know the zero means something.

`ValueState::Stale` becomes reachable for the first time. 2a's spec noted it was
"still never constructed" and reserved it for exactly this.

The latency rules:

| Situation | `value` | `state` | `measuredAt` |
| --- | --- | --- | --- |
| Newest probe in window succeeded | latency | `measured` | that capture |
| Newest probe failed, an earlier success is still in the window | that older latency | `stale` | the **older** capture |
| No success in the window | `null` | `unavailable` | `null` |

`lastProbeAt` keeps tracking the last **successful** probe, unchanged from 2b.
Reporting the time of a failure as the last probe would make a broken connection
look freshly checked. The UI label changes to match what it actually holds.

**Frontend types.** One added type, mirroring the Rust one:

```ts
export type ProbeFailureBasis = { failed: number; total: number };
```

and `ConnectionOverview` gains `probeFailureBasis: ProbeFailureBasis | null`. The
top-level payload goes from 8 fields to 9, so the `payload_shape_is_locked` test's
expected count is widened deliberately rather than left to fail.

`ValueState` needs no change: `stale` already exists on both sides and the tile
already knows how to draw it.

## Testing

- `cargo test` is the gate. All of this is pure logic and ships its tests in the
  same diff.
- `cargo clippy --all-targets` clean, `npm run build` for Step 4.
- **No test reaches the internet.** 2b's opt-in test already covers that ground.
- Time is injected, never slept on. Every window test records outcomes stamped in
  the past, so the staleness and eviction rules are exercised without a test
  waiting five minutes, and they stay deterministic on a loaded machine.
- The frontend claims are visual and ride on browser evidence from `/check`, since
  no JavaScript test runner is configured.

## Notes for the AI

- **Never call this packet loss in anything a user reads.** The wire field is
  named `packetLoss` for historical reasons and must keep that name, but the
  label, the tooltip, the aria label and any comment a user could surface say
  "probe failures". A comment explaining that the name is historical is welcome;
  a comment that quietly calls it packet loss is not.
- The percentage is a **real measurement when `total > 0`**, including when it is
  zero. Do not report `unavailable` for `0%`, and do not report `0%` for "no
  samples".
- A stale reading must always carry the age of the number it is holding. A stale
  value with no age is the same unreadable state 2b had, just spelled differently.
- The panel and the dashboard must render the failure figure from the same
  formatter. This is the specific failure `format.ts` exists to prevent, so
  neither surface is allowed to format the counts itself.
- Apply the window at read time. Do not rely on eviction having happened, because
  a stalled loop evicts nothing.
- No em dashes, en dashes, or ellipsis characters in code, comments, or docs.
- Do not add a threshold comparison. There is no configured threshold to compare
  against until feature 5 creates the settings row, and inventing a default would
  be a decision feature 4 has to own.

## Verification

| Claim | How it was proven |
| --- | --- |
| 84 tests pass | `cargo test` |
| Clippy clean | `cargo clippy --all-targets`, 0 warnings |
| Frontend builds and typechecks | `npm run build` |
| The window is applied when reading, not only when writing | Step 1's tests record a sample, age it past the window with the clock alone, and assert the read excludes it |
| A genuine zero is not the same as no data | `every_attempt_succeeding_is_a_genuine_zero` reports `0.0` as measured, `no_attempts_reports_no_summary_rather_than_zero_percent` reports `None` |
| A failed probe holds the last good reading | `a_failed_probe_holds_the_last_good_latency_as_stale` asserts the **older** number carries its **own** capture time |
| A held reading is dropped once it leaves the window | `a_held_reading_is_dropped_once_it_leaves_the_window` and `a_held_latency_is_dropped_when_it_leaves_the_window` |
| One failed probe in three survives rounding | `one_failure_in_three_is_not_rounded_away` compares against `100.0/3.0` with a tolerance, so rounding cannot quietly rescale the figure |
| The payload is 9 fields and the basis travels with the figure | `payload_shape_is_locked`, `a_measured_failure_rate_travels_with_its_counts_and_window`, `the_basis_is_present_exactly_when_the_rate_is` |
| `ValueState::Stale` is constructed for the first time | `freshest_latency` returns it, and `commands::overview` maps it onto the wire |
| Both surfaces render the counts, not a percentage | The four real components server-rendered against crafted snapshots: dashboard tile reads `3 of 25 failed / Last 5 minutes`, panel row reads `3 of 25`, first run reads `No reading` on both |
| No colour outside the token layer | Grep for hex literals across `src`, none outside `src/styles/global.css` |
| No em dashes, en dashes, or ellipsis | Scanned every file this feature touched |

### Defects found and fixed during the build

- **A held reading was never surfaced with its age.** 2b blanked latency on every
  failure, which the spec promised 2c would replace. Replaced with
  `FreshLatency::Fresh` / `Stale` / `None` and a tile foot that states the age.
- **`No reading failed`.** The first version of `formatProbeFailures` returned
  the words "No reading" from inside the formatter, and the tile rendered that
  caption beneath them with a window foot above a figure that was not there. The
  formatter now returns `null` and the tile falls back to its empty rendering in
  full.
- **The hero card said "Healthy" whenever a latency existed.** Once stale
  readings became reachable, that badge would have read Healthy while probes
  were failing. It now carries the real health verdict. This was a change beyond
  the written spec and is recorded here so the history explains it.

**Not verified.** The native window and the menu-bar popover have never been
observed by eye, and no browser tooling was available while this feature was
built. The dashboard and panel were proven by server-rendered markup, which
confirms the words and not the layout. `screencapture` returns wallpaper only on
this machine because Screen Recording permission is not granted. `/check` is the
command that closes this and it is still outstanding from 2b.

**Still out of scope.** The window is in memory, so both figures clear on
restart until feature 5 persists them. Multi-target aggregation stays with 2d.
Thresholds stay with feature 4, because there is no configured value to compare
against until feature 5 creates the settings row.
