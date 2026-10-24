# FibreX Monitor - Project Overview

<!-- buildflow:source-hash dde65b05f15e6c34206023fca6e0f5487b1d3a11af388dbdc33be3b0b65742b7 -->

> A local-first macOS menu-bar app that monitors an MTN FibreX connection: real connectivity, latency, probe failures, recorded incidents with recovery times, and verified Huawei OptiXstar router reachability, with no cloud service and no router configuration changes.

## Problem

Diagnosing FibreX problems today means watching the router admin dashboard and running `ping` by hand, then trying to remember when the connection dropped. It is hard to tell router reachability from internet reachability, to see whether latency is abnormal, to know how long an outage lasted, or how much data moved. FibreX Monitor watches continuously, keeps the evidence locally, and answers those questions from the menu bar.

Core principles: monitor continuously, understand the connection, keep the evidence locally. Core monitoring must keep working when router integration is unavailable, and the app must never present unverified or fabricated readings.

Measurement note: probes are TCP connects to `1.1.1.1:443` and, separately, to the router at `192.168.100.1:80`, because macOS does not permit unprivileged ICMP sockets. A failed connect is a probe failure, not packet loss, and the app says so. Router reachability and internet reachability are always reported separately.

## Users

| User | Needs |
| --- | --- |
| FibreX owner (primary) | Better visibility into intermittent internet performance |
| Mac users running `ping` manually | A menu-bar health summary without the router dashboard |
| People with intermittent disconnects | Reliable incident history with recovery times |
| Home users, developers (secondary) | Lightweight local monitoring, historical latency and traffic |

Not targeted: ISPs, enterprises, packet-capture/security suites, per-device bandwidth accounting without a router source, automatic router repair. Personal-first, local-first, read-only. No accounts, no tiers.

## Features

MVP, in `build-plan.md` order. The list below mirrors that checklist; the build plan is the source of truth when they disagree.

1. **Application shell, design system, and connection overview** - rail, topbar, wordmark, tokens, the five screens, and the overview on real measurements.
2. **Connectivity monitoring** - real reachability and latency via configurable targets, intervals, timeouts.
3. **Router reachability monitoring** - probe the router separately from the internet to separate local from upstream faults. Real today: a TCP connect to `192.168.100.1:80`, no login.
4. **Connection health states** - Connected, Degraded, Offline, Unknown, Paused, from measured data plus thresholds.
5. **Local monitoring persistence** - SQLite storage of every probe, with retention and incident rows.
6. **Latency history** - stored history behind any range wider than this launch, with explicit gaps for failed probes.
7. **Incident detection and timeline** - sustained-failure detection, start and recovery times, duration, history across restarts.
8. **Native macOS menu-bar interface** - compact native popover: status, key metrics, dashboard access.
9. **Monitoring lifecycle controls** - pause/resume, background operation while the window is closed, clean shutdown on quit.
10. **Router connection integration** - a safe read-only client, validated endpoints, source status independent of internet monitoring. Session-gated, so it stays off until a real login exists.
11. **WAN traffic monitoring** - verified router byte counters to upload/download rates. Session-gated.
12. **Traffic history and charts** - historical rates from stored data. Session-gated.
13. **Connected-device overview** - verified device names, connection types, online states. Session-gated.
14. **Router health monitoring** - verified ONT registration, WAN state, and other fields a real endpoint exposes. Session-gated.
15. **Monitoring settings** - interval, timeouts, thresholds, retention, behaviour.
16. **Native notifications** - loss, recovery, sustained latency, with duplicate suppression.
17. **Light and dark appearance** - system, light, dark themes on the shared design tokens.
18. **Diagnostic export** - a reviewable summary of connectivity and incidents without credentials or tokens.
19. **Error handling and recovery** - failed probes, unavailable endpoints, expired sessions, malformed responses, sleep/wake.
20. **Privacy and security hardening** - local by default, protected auth material, redaction, read-only router access.
21. **MVP testing and verification** - calculations, state transitions, incident detection, persistence, menu-bar, lifecycle, router parsing.
22. **macOS packaging and deployment readiness** - a packaged local build verified outside development mode.

Headline feature: **the connection overview** (1), which the rest of the MVP feeds.

Post-MVP: traceroute diagnostics, multi-target management, percentile and long-range analysis, sleep/wake improvements, router compatibility, optical signal monitoring (transmit power, receive power, temperature, voltage, bias current), CPU and memory, retention management, history export, expanded notifications, launch at login, menu-bar customisation, active speed testing, other-router support, accessibility refinement, performance work, operational logging, backup and restore, production security review, release workflow.

## Data model

Concrete shapes derived from `project-plan.md` and the features that use them. `NULL` means unavailable or not yet measured. Never coerce unavailable to `0`. Store timestamps as UTC epoch milliseconds.

The shipped implementation is narrower than the original plan. What exists today is `probe_sample` and `incident`; the router tables below are the intended shapes for the session-gated features and are not created yet. `monitoring/store.rs` is the authority on what is actually in the database.

### probe_sample (implemented)

- `id` (integer) - primary key
- `target_id` (integer) - the compiled-in target the probe ran against
- `captured_at` (integer, UTC ms), indexed
- `success` (integer) - 0 or 1
- `latency_ms` (real, nullable) - NULL for a failure, never 0
- `error_kind` (text, nullable) - `timeout` | `unreachable` | `dns` | `other`

### incident (implemented)

- `id` (integer) - primary key
- `target_id` (integer)
- `started_at` (integer, UTC ms), indexed
- `recovered_at` (integer, nullable) - NULL while the outage is open
- `probe_count` (integer) - failed probes recorded
- `router_reachable` (integer, nullable) - captured when the outage began; NULL when no router reading covers it

An incident is opened by two consecutive failed probes (`INCIDENT_THRESHOLD`) and closed by the next success. A single failure that recovers is deleted rather than reported. There is deliberately no cause, severity, or classification column: a failed TCP connect is not evidence of a cause.

Raw samples are kept for `RETENTION_MS` (14 days) and pruned on write. No aggregation is stored, because an aggregate would be a derived number with no stored evidence behind it.

### Planned: traffic_sample

- `id` (integer) - primary key
- `captured_at` (integer, UTC ms), indexed
- `wan_rx_bytes` (integer, nullable) - cumulative counter
- `wan_tx_bytes` (integer, nullable) - cumulative counter
- `download_bps` (real, nullable) - derived, never shown as the counter
- `upload_bps` (real, nullable) - derived
- `counter_status` (text) - `ok` | `unavailable` | `reset` | `stale`
- `source_retrieved_at` (integer, nullable)

Rates come from counter deltas over elapsed time. On a decrease, reset, or gap, mark the sample `reset`/`stale` instead of emitting a spike.

### Planned: router_snapshot

- `id` (integer) - primary key
- `captured_at` (integer, UTC ms), indexed
- `ont_registration_state` (text, nullable)
- `wan_connection_state` (text, nullable)
- `source_status` (text) - `ok` | `partial` | `unavailable`

Optical, temperature, voltage, bias, CPU and memory are Post-MVP: no verified endpoint exposes them yet, so they are not modelled ahead of one.

### Planned: device

- `id` (integer) - primary key
- `display_name` (text, nullable) - hostname or label
- `online` (boolean, nullable)
- `connection_type` (text, nullable) - `wired` | `wifi`
- `wifi_band` (text, nullable), `signal_strength_dbm` (real, nullable)
- `last_refresh_at` (integer, UTC ms)

MAC and private IP addresses are not persisted or are redacted by default.

### Relationships

- `probe_sample.target_id` -> the compiled-in target (`config.rs`)
- `incident` rows are per target; at most one is open per target
- `router_snapshot`, `traffic_sample`, and `device` are independent series keyed by `captured_at` / `last_refresh_at`

Schema changes go through the `user_version` migration in `monitoring/store.rs`.

## Tech stack

- **React 19 + TypeScript** - all UI; functional components and hooks only.
- **Vite** - frontend dev server and production bundle. No SSR, no server routes.
- **Tailwind CSS 4 + CSS variables** - styling and shared design tokens in `src/styles/global.css`, the only file allowed literal colours.
- **Hand-built SVG charts** - latency history. No charting library: the app's own chart component draws to SVG so the look matches the design exactly and failed probes stay visible as gaps.
- **Inline SVG icons** - the app's own icon set in `src/components/ui/icons.tsx`, including the wordmark taken from the topbar design. No icon dependency.
- **Vitest** - unit tests for the formatting and calculation rules. `npm test`.
- **Tauri 2** - desktop shell, main window, menu-bar/tray, typed commands and events.
- **Rust** - the single monitoring backend: probes, health states, the incident engine, storage.
- **SQLite via `rusqlite` (bundled)** - local history in the macOS application-data directory, with `user_version` migrations. Bundled so the app has no system SQLite dependency.

Boundaries: the Rust backend owns the one monitoring loop shared by the main window and the popover. React never pings hosts, calls the router, or reads the database directly. See `coding-standards.md`.

Current repo state: the template demo is gone. The shell, design system, five screens, the menu-bar panel, TCP probes, health states, router reachability, persistence and incident detection all exist and are tested. Router session data (WAN, devices, router health) is not implemented because it needs a login this machine does not have.

## Monetization

Not in v1. FibreX Monitor is a free personal utility. No advertising, subscriptions, paid backend, accounts, or paywall. Future options (advanced reports, more router integrations, export) are uncommitted possibilities.

## UI/UX

A modern, restrained macOS utility: compact in the menu bar, detailed in the main window. Not an admin dashboard or analytics surface. No excessive gradients, glassmorphism, oversized metric cards, or decorative charts.

Screens (five, not seven):

- **Overview** - the headline latency and its subject, the router's reachability, probe failures over the five-minute window, latency history for this launch, uptime from stored probes, and an honest empty WAN card.
- **Fibre & Router** - router reachability, the internet target, probe failures, line latency history, and WAN traffic and router session status (session-gated, so they state what is missing).
- **Devices** - the connected-device list once a router session exists; until then it says so rather than inventing devices.
- **Incidents** - recorded outages with start, recovery, duration, failed-probe count, and what the router was doing at the time. Three distinct empty states.
- **Settings** - appearance, privacy, the compiled-in probe target, router address, history retention, and what is not built yet.
- **Menu-bar popover** (not a screen) - status, latency, probe failures, router reachability, and open-dashboard. No download/upload rows.

The original plan's Connectivity and Traffic screens were removed: Connectivity folded into Overview, and Traffic folded into Fibre & Router as a WAN section. A separate screen for each would have been two more places to show the same numbers.

Design tokens live in `src/styles/global.css`, ported from `prototypes-new/theme.css`. The decorative page vignette and glass effects from the prototype were dropped; the rail, topbar, wordmark, bento layout, typography, spacing, radius, surfaces and status colours were kept. The chart is hand-built SVG with no library, and it draws a gap where a probe failed rather than interpolating through it.

One appearance control, a single button whose icon reflects the current mode. Status colour always pairs with a text label. Tone is plain: "Connected", "Degraded", "Offline", "No reading", "Monitoring paused", "Router not checked". A probe failure is never described as packet loss, and an outage is never attributed to MTN or an ISP without evidence.

Closing the main window must not stop monitoring; quitting the app stops it cleanly.

## Deployment

Local macOS desktop application. No host, no cloud, no remote database.

- **Target** - Apple Silicon Macs first; packaged macOS app bundle.
- **Dev run** - `npm run tauri dev` (needs Rust).
- **Package** - `npm run tauri build`, output `src-tauri/target/release/bundle/`.
- **Frontend only** - `npm run dev` (browser), `npm run build`, `npm run preview`.
- **Verify** - `npm run verify` (typecheck, then unit tests), `cargo test` in `src-tauri/`.
- **Storage** - SQLite under the macOS application-data directory (`history.sqlite3`); never inside the bundle.
- **Env vars** - none required. No `.env` with router credentials.
- **Workers / cron** - none; monitoring is an in-process Rust task.
- **Health path** - not applicable (no HTTP service).
- **Permissions** - local network access for probes. macOS may prompt for local network access on first run.
- **Signing / notarisation** - `> TODO`: decide before any public distribution.

## Open questions

1. **Router session unavailable** - the WAN, devices, and router-health features need a signed-in router session. No credentials are stored and none will be guessed, so those features cannot be verified on this machine. Resolve by signing in to the router once and capturing the real endpoint responses, then implementing against them.
2. **Optical and hardware metrics** - no verified endpoint exposes transmit power, receive power, temperature, voltage, bias current, CPU or memory. Moved to Post-MVP rather than built on a guess.
3. **Chart range beyond this launch** - stored history exists and `get_stored_series` can read it, but the Overview chart still plots only the current launch. A range selector should be wired once the stored history is worth browsing.
4. **Native appearance never confirmed by eye** - the built app was verified in headless Chrome, but the native window, tray popover and menu-bar behaviour need a real `npm run tauri dev` run and a human look.
5. **WAN counter direction and units** - `BytesSent` and `BytesReceived` meanings must be confirmed against real traffic before rates are shown as upload and download.
6. **Signing and notarisation** - `> TODO`: decide before any public distribution.
7. **Readings between probes** - uptime is the share of probes that connected, which is a count of observations rather than a stopwatch. If a true time-weighted figure is ever wanted, it needs a different model and should say so.
