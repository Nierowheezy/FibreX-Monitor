# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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