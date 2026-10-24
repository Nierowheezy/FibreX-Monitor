# Router connection integration and router health

**Type:** Feature

**Status:** verified

**Build plan:** 10 (Router connection integration), 14 (Router health monitoring)

> One work item delivered both build-plan items. Feature 10 is the integration
> itself; feature 14 is the router health it made readable. Both are checked off
> in the build plan from this single archive.

## What this is

Read-only communication with the Huawei OptiXstar HG8145X7-10 router at
`192.168.100.1`, and the Fibre & Router screen it feeds. The app signs in with
credentials from its own environment, reads the WAN connection and the router's
own health figures, and reports them with the same honesty rules the rest of the
app uses. It never writes to the router and never blocks when the router is
absent.

The protocol is not the obvious one, and three details each cost a debugging
session to find. They are recorded in
`buildflow/designs/router-integration-findings.md` and asserted in tests so a
firmware change shows up as a failing test rather than a blank screen.

## Backend

New in `src-tauri/src/router/`:

- `mod.rs` holds the wire types: `RouterCredentials`, `RouterAuthSummary`,
  `RouterSnapshot`, `RouterSourceStatus`, `RouterError`, `RouterErrorKind`, and
  the `RouterState`/`RouterReport` pair the IPC command returns.
- `parse.rs` decodes the firmware's escaped JavaScript string literals and
  reads the WAN `WanPPP` argument list by position. Every position
  (`status` 5, `accessType` 13, `ip` 14, `gateway` 15, `dns` 18, `mac` 4,
  `uptime` 23, `mtu` 37, `bras` 38 of 58) was verified against a verbatim copy
  of the live page before it was written. `classify_wan_address` separates an
  RFC1918 private address from a public one and from the `100.64/10` CGNAT
  range.
- `client.rs` is the blocking client. It signs in with `getRandString.asp`
  then `POST /login.cgi` (password base64 encoded, not hashed), and reads pages
  with a fresh `GetRandToken.asp` token per request. It carries the
  `CookieHttps` session cookie set by `/login.cgi`, because every authenticated
  page returns a 602-byte stub with status `200` without it.
- `credentials.rs` loads host, port, username and password from the process
  environment only. There is no compiled-in credential and no settings field
  that writes a password to disk. Half a pair is treated as no pair.
- `state.rs` holds the three-state router cell (reachable, unreachable, no
  reading) and keeps the last good snapshot across a failure rather than
  blanking it.
- `reader.rs` is the background thread. It reads on a 30 second interval after
  a short first delay, logs `router read ok: N pages answered of M` or the
  failure, and never logs a credential or a token.
- `commands/router.rs` exposes `get_router_report` and has an IPC-boundary test
  proving no credential crosses the wire.

## Frontend

- `src/types/overview.ts` carries the router contract: `RouterSnapshot`,
  `RouterReport`, `WanConnection`, `RouterHealth`, `RouterSourceStatus`, and
  `RouterError`.
- `src/lib/tauri/router.ts` and `src/hooks/useRouterReport.ts` poll every 30
  seconds, guard on `isTauri()`, and keep the last good report.
- `src/features/router/format.ts` holds the formatters, with tests.
- `RouterScreen.tsx` renders the real WAN, health, session and system cards.
- `DevicesScreen.tsx` states specifically that a signed-in session found no
  client list, rather than showing an empty table that would read as "no
  devices".

## What is deliberately not here

- No temperature, voltage, or bias current. The firmware reports `temp1` and
  `temp2` as bare `0`, so those fields are absent from the wire type entirely
  rather than being rendered as a real zero.
- CPU and memory are shown only where the firmware's own `showCPUnMemUsed`
  flag enables them.
- No WAN byte counters (feature 11) and no device list (feature 13). Neither
  has a source on this firmware; both stay unbuilt rather than guessed.
- Optical receive power is shown. Transmit power stays unverified.

## Done when

- `npm run verify` passes (typecheck, then unit tests).
- `cargo test` and `cargo clippy --all-targets` are clean.
- A live read from the packaged app reports the observed WAN and health values,
  and the log line proves it.
- No credential or its encoding exists in any tracked file or in `dist/`.
- The screen shows "No reading" rather than a number whenever the router is
  absent.
