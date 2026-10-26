# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.2

- **An outage that was not watched is no longer reported as an outage that was measured.** An incident was opened by two failed probes and closed by the next success, with nothing in between checked. If probing stopped — monitoring paused, the app quit, the Mac slept — the first success afterwards closed the incident and the whole silent stretch was stored as confirmed downtime. Probing that stops now closes the incident at the last probe it actually saw, and so does any gap wider than ten seconds between probes; the gap starts a fresh observation instead of extending the old one. A gap followed by another failure begins a new incident rather than drawing a line across time the app did not see.
- Incidents carry two new fields, `observed_until` and `closed_by`, so the record says how far it was watched and why it closed. Existing history migrates in place and keeps every row; incidents recorded before this version were all closed by a successful probe.
- An incident closed because observation stopped is labelled **Unobserved** in the app, its duration reads "at least" the watched part, and its end time reads "monitoring stopped" rather than "recovered". The incidents screen explains the count in one line.

## v0.1.1

- The menu-bar panel now appears over whichever app and desktop Space is in front. A window that this app created at launch, while it was still a regular app, ignored macOS's "join all Spaces" behaviour for the rest of its life and stayed on the Space it was born on, so clicking the menu-bar icon while another app was frontmost looked like nothing happened. The panel is now created only after the app has become a menu-bar app, which is when macOS honours that behaviour, and it lands on the active Space from the first click.
- The panel is dismissed by a click in any other app. It used to hide when it lost keyboard focus, but an accessory app can be given activation and then have it taken back without any input from anyone; that rule read the hand-back as a click elsewhere and took the panel away about two seconds after opening it.
- The panel is placed against the display the menu-bar icon is actually on, rather than the display it last appeared on, so its position stays correct with more than one screen.
- The menu-bar icon is the app's own mark — the three ember chevrons — instead of a placeholder signal-bars icon. It is drawn as a macOS template image, so it follows the menu bar's light and dark appearance.

## v0.1.0

The first release on GitHub. The project was carried through several internal
pre-release iterations before this, so what ships here already includes work a
first release would otherwise grow across a v0.1.x series:

- **Connection monitoring** — the connection probed every two seconds in the
  background; every reading (online, offline, failing) stored locally.
- **Incident timeline** — every outage recorded with its exact start, end, and
  duration, newest first, in a contained 320 px scroll viewport; the
  "Recorded outages" card beside it hugs its own content.
- **Honest speed test** — a real measurement against `speed.cloudflare.com`
  with 6 parallel download streams, warm-up discard, and streaming upload,
  shown in three named phases (Ping / Download / Upload) on the gauge.
- **Read-only router integration** — live Huawei OptiXstar figures (WAN
  traffic, link state, uptime, firmware details) via a sign-in and page reads
  only; router reachability and internet reachability kept strictly separate.
- **In-app router configuration** — the router connection is set under
  Settings: address, port, user name in a settings file, the password in the
  macOS Keychain only, a write-only password field that never echoes back,
  saves that keep the stored password, an explicit two-click removal, and a
  status badge that names its source (Reading / Password missing /
  From environment / Not configured). The `FIBREX_ROUTER_*` environment
  variables remain as a developer override that wins while present.
- **Local-first history** — a local SQLite database; no cloud service, no
  account, no telemetry.
- **macOS menu-bar integration** — live state in the menu bar panel, a full
  dashboard window, menus, and a real light/dark pair of themes.

##### Build

`npm install` then `npm run tauri dev` to run, or `npm run tauri build` for the .app bundle.