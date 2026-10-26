# Current Feature

# An outage that was not watched is not reported as an outage that was measured

**Type:** Fix

**Status:** implemented, awaiting review

## What this is

The incident log could claim downtime it never measured.

An incident is opened by two consecutive failed probes and closed by the first
successful one. Nothing in between was checked, so if probing stopped — the
user paused monitoring, quit the app, or the Mac slept — and the line came back
before the next probe, that next success closed the incident and the whole
silent stretch was stored as confirmed downtime.

That is the one thing an incident log must not do. It is also the exact
complaint that came back about the launch post: the pause exists, so a gap in
the record is expected, and the log was describing the gap as if it had watched
it.

**The fix closes the incident where the evidence ends, and says so.**

## The change

**Probing that stops closes the incident.** `set_monitoring_paused` now takes
the history store and closes every open incident at the last probe it saw,
before the loop goes quiet. A pause no longer sits inside an outage.

**A silence wider than a plausible cadence closes it too.** `Store::record`
compares each probe's timestamp with the open incident's last observed probe.
Anything wider than `UNOBSERVED_GAP_MS` (ten seconds, five missed probes at the
shipped two-second interval) means the app was not watching: the incident is
closed at the last probe it actually saw, and the incoming probe starts a fresh
observation. This covers the cases a command cannot: a sleep, a quit, a probe
that hung, a pause the close command missed.

A gap followed by a failure starts a new incident rather than continuing the old
one, because the record must not draw a straight line across time it did not
see.

**Two new columns say why it closed.** `observed_until` is the last probe folded
into the incident, which is how far the evidence reaches. `closed_by` is
`probe` when a success closed it, `unobserved` when observation stopped first,
and null while it is open. `recovered_at` for an unobserved incident is that
last observed probe, so the recorded duration is the part that was watched.

The schema moves to v2 with a migration that backfills both columns: existing
recovered rows were closed by a probe, because nothing else could close one, and
their last observation was their recovery.

**The interface stops claiming what it did not see.** An unobserved incident is
badged `Unobserved`, its duration reads `Lasted at least 3.0s · monitoring
stopped`, and its end time reads `Monitoring stopped 14:02` rather than
`Recovered`. The incidents screen also says, once, how many incidents ended that
way and what the duration means. Nothing is hidden and nothing is estimated.

Below-threshold partial runs are discarded on a pause exactly as a success
discards them: one failed probe is a blip and a pause must not promote it.

## Done when

- Pausing during an open outage closes it at the last observed probe, marked
  `unobserved`.
- A probe that arrives more than ten seconds after the last observed one closes
  the incident where the evidence ends, and does not extend it.
- A gap followed by another failure starts a new incident.
- A v1 database opens, keeps every row, and gains both columns.
- The interface never presents an unobserved incident's duration as measured.
- `npm run verify` passes.
- `cargo test` passes (serially; the Keychain tests in `router::app_config`
  race when run in parallel and fail with `os error 22`, which predates this
  fix).
- The harness shows both a measured and an unobserved incident.
