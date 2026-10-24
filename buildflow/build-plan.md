# Build Plan

> One of the two planning docs for FibreX Monitor. Keep the items high-level even when `project-plan.md` is detailed; later `/feature` specs hold the depth for each build item.
>
> The features that make up this project, high level and in rough build order, one line each, no detail (that comes per feature). Before `/overview` runs, this file should be shaped into a checkbox list the build loop can track.
>
> Keep it as a checklist. Run `/feature` with no number to spec the **next unchecked** item, or `/feature 3` / `/feature "WAN traffic monitoring"` to pick a specific one.
>
> Completed features get checked off here, so the build plan doubles as the progress tracker. A big item gets split into sub-items (4a, 4b, etc.) when you spec it.
>
> **Continuing after the initial build**
>
> This is a living roadmap, not a plan that freezes when the first release is done. Keep completed items checked, then append new unchecked features as the project grows. Optional milestone headings such as `## MVP` and `## Post-MVP` keep a longer plan readable without changing how `/feature` finds the next unchecked item.
>
> Scaffolding the app and setting up development infrastructure are pre-build steps, not features. The build starts with the first new product feature.
>
> **What is real today (audited 2026-10-20)**
>
> The app measures latency over TCP connects, probe failures across a rolling five-minute window, a connection verdict, monitoring active, and the last successful probe time. Those are the only live figures from the connectivity side. Router reachability is verifiable now: the router answers on HTTP port 80 at `192.168.100.1`, and a signed-in read-only session returns the WAN connection, the optical receive power, the CPU and memory use, and the WAN byte counters. Download and upload rates have a source but no producer yet, so they must read as unavailable rather than zero.
>
> **Screens.** Five, not seven: Overview, Fibre and Router, Devices, Incidents, Settings. The standalone Connectivity screen is gone (its chart lives in Fibre and Router), and Traffic is folded into Fibre and Router as a WAN section.

## MVP

- [x] 1. **Application shell, design system, and connection overview** - the surface every other feature renders into, and the headline screen over measured data.
  - [x] 1a. Data pipeline and first surface - the overview command, a live latency reading, probe failures over the rolling window, a connection verdict, and the first token layer, showing only what monitoring actually measured. _Note: shipped as feature 1 in the old numbering. The pipeline is real and stays; the surface is replaced by 1b._
  - [x] 1b. Dark bento adoption - port the approved token layer from `prototypes-new/theme.css` into `src/styles/global.css`, adopt the topbar wordmark as the app logo, and build the rail, topbar, and five-screen shell (Overview, Fibre and Router, Devices, Incidents, Settings) that the rest of the MVP renders into. Porting the token layer is the first build step of any feature that changes the look. _Note: shipped. `global.css` is now the dark bento token layer (Outfit, Figtree, JetBrains Mono; `@theme static` tokens; a light override block), `App.tsx` is the rail plus topbar plus wordmark plus five-screen switch, and the old decorative vignette and glass effects were dropped. Screens cut from seven to five._
  - [x] 1c. Honest overview - every tile shows a measured, calculated, stale, or explicitly unavailable value, the fabricated "median of last 150 probes" caption and the five-minute range stepper are removed, and unsupported metrics (download, upload) are removed from the interface rather than left as permanent empty cards. _Note: shipped. The hero names its subject via `primaryTarget`, probe failures read `2 of 25` from backend counts, the router card has three states including "No reading", uptime states the period it covers, and the WAN and optical cards say what is missing instead of showing a zero._

- [ ] 2. **Connectivity monitoring** - measure real network reachability and latency using configurable probe targets, intervals, and timeouts
  - [x] 2a. Probe configuration and probe primitives - typed configuration matching the locked probe target shape with compiled-in defaults, and a single-shot TCP connect probe returning reachability, latency, and a failure kind, unit tested
  - [x] 2b. Monitoring loop and the first real reading - a background task probing on each target's interval, the latest sample held in memory, started on app setup and stopped on shutdown, with the overview showing a real latency and refreshing _Note: grew beyond its spec by user decision during the build. Health classification (feature 4, now narrowed to multi-target verdicts), a live in-memory chart series (feature 6 keeps persistence), a polling menu-bar panel, and a 2s probe interval all landed here. Two defects were found and fixed on the way: a startup panic from spawning outside Tauri's runtime, and an aborted loop leaving `monitoringActive` stuck true._
  - [x] 2c. Packet loss and freshness - packet loss across a rolling probe window and the staleness rule that marks a reading stale once it is older than that window _Note: shipped as **probe failures**, not packet loss. The figure is the percentage of probe attempts in a five minute window that did not connect, shown as `3 of 25 failed` on both surfaces. macOS denies unprivileged ICMP sockets, so a failed TCP connect is not a lost packet and calling it packet loss would be a measurement this app never took; genuinely measuring it needs raw sockets and a privilege escalation and stays out of scope unless you decide otherwise. A failed probe now holds the last good reading and marks it stale with its age, replacing 2b's blanking. The wire field keeps the name `packetLoss` for features 6, 7, 17 and 25, so do not read that name as permission to display it._
  - [ ] 2d. Multiple targets and target status - several enabled targets, a defined rule for aggregating them into the one headline latency, and a per-target status list

- [x] 3. **Router reachability monitoring** - probe the local router separately from external internet targets so a local fault and an upstream fault can be told apart. _Note: shipped. The router is a target of kind `Router` in the same probe loop as the internet target, and it is reported in its own section of the overview with three states: reachable, unreachable, and never probed. A router that was never probed is not a router that is down. The router's part in an outage is recorded with the incident, so a local fault and an upstream fault can be told apart in the timeline._

- [ ] 4. **Connection health states** - classify the connection as Connected, Degraded, Offline, Unknown, or Paused from validated measurements and configurable thresholds. _Note: 2b shipped a basic classifier over recent probe outcomes; this feature adds thresholds, the router-versus-internet split from feature 3, and the sentence vocabulary both surfaces share._

- [x] 5. **Local monitoring persistence** - establish the SQLite data model and persist connectivity samples, traffic measurements, router snapshots, settings, and incidents locally, so history, "uptime today", and incident records survive a restart. _Note: shipped for connectivity samples and incidents, which is what has a producer today. `monitoring/store.rs` uses `rusqlite` with the bundled feature, one connection behind one mutex, WAL, `foreign_keys ON`, and `user_version` migrations. A failed probe stores NULL latency, never 0. Raw samples are kept for 14 days and pruned on write; nothing is aggregated. Traffic measurements and router snapshots stay unbuilt because no verified session produces them. History is optional: if the database cannot be opened the app logs and keeps monitoring with history switched off._

- [ ] 6. **Latency and probe-failure history** - display persisted latency and probe-failure measurements with selectable time ranges and clear handling of missing or stale data. _Note: **shipped.** The stored history is now selectable: a range stepper in the hero chooses between 5 minutes, 15 minutes, 1 hour, 6 hours and 24 hours, and both the hero's background line and the detailed chart read the stored series for the range in force. A long range is thinned into equal slices of time before it crosses the IPC boundary, and a slice containing a failed probe stays a gap, so thinning can never draw a smooth line over an outage. The card states how many stored probes the range holds rather than implying the range is full. The probe-failure history is the same stored data, counted in the backend._

- [x] 7. **Incident detection and timeline** - detect sustained connectivity failures, record incident start and recovery times, calculate outage duration, and preserve incident history across restarts. _Note: shipped. Two consecutive failed probes open an incident; the next success closes it; a single failure that recovers is deleted so a flaky moment leaves no trace. Incidents are per target and at most one is open per target by construction. The router's state is captured when the outage begins and never overwritten at recovery. The Incidents screen has three distinct empty states (history off, nothing stored, no outages) and names no cause._

- [x] 8. **Native macOS menu-bar interface** - provide a compact native menu-bar popover with connection status, key metrics, monitoring controls, and access to the main dashboard. _Note: shipped. 8a and 8b are in place: the tray icon, the right-click menu, the floating panel under the icon, the Escape and focus-loss dismissal, the compact status inside it, and the dashboard link. The popover's monitoring controls arrived with feature 9, so the panel now carries a real pause and resume. Closing a window hides it instead of quitting, and the app runs as an accessory with no Dock icon._
  - [x] 8a. Menu-bar panel - a menu-bar icon with no Dock icon, a right-click menu for the dashboard and for quitting, a floating panel toggled from the tray and positioned under it that hides on Escape or when it loses focus, compact honest status inside it, and closing any window hiding instead of quitting
  - [x] 8b. Panel visual treatment - the panel drawn on the dark bento token layer from `prototypes-new/panel.html` with its own card surface, border, spacing and typography. _Note: shipped and rebuilt to the prototype. The panel has the app mark and the live dot in its header, a `LATENCY` label over the figure, a health pill, a foot line with when it was probed and which target it was against, a sparkline of the same latency series the dashboard draws, the five rows the design has, a primary `Open dashboard` action beside a `Pause` action, and a foot line saying the interval and that everything stays local. Every row has a real producer, and the one that does not, the device count, says so rather than showing a count of zero._

- [ ] 9. **Monitoring lifecycle controls** - support pausing and resuming monitoring, reliable background operation while the main window is closed, and clean shutdown when the application quits. _Note: **pause and resume shipped.** The paused flag lives in the monitoring state and the loop skips its probe while it is set, so the loop stays alive, a resume is immediate, and the readings taken before the pause are still in the history. The interface reports `paused` separately from `monitoringActive`, because a paused app is not a stopped app. The control is in the menu-bar panel, in the header and the footer, and on the Settings screen. Background operation and clean shutdown are partly there: the app is an accessory, closing a window hides it rather than quitting, and quitting is clean, but neither has been verified as a lifecycle test._

- [x] 10. **Router connection integration** - implement safe, read-only communication with the Huawei OptiXstar HG8145X7-10 router at `192.168.100.1`, validate observed endpoints, and expose router data-source status independently of internet monitoring. _Note: **shipped.** The protocol was verified against the live device and is recorded in `buildflow/designs/router-integration-findings.md`. The confirmed flow is two-token (`getRandString.asp` to sign in, `GetRandToken.asp` per authenticated request, single-use, BOM-prefixed) **plus a `CookieHttps` session cookie set by `/login.cgi` that every authenticated request must carry**. The UI is HTTPS on port 80 with a self-signed certificate. Credentials come from the environment only, are never hardcoded, never logged, and never cross the IPC boundary; a missing pair leaves the router unread rather than producing a sign-in failure that looks like a wrong password. The app keeps working while the router is absent._

- [x] 11. **WAN traffic monitoring** - retrieve verified router WAN byte counters and calculate upload and download rates from successive readings, accounting for counter resets, stale values, and unavailable data. _Note: **shipped.** The counters come from `/html/bbsp/common/get_wan_list_pppwanstat.asp`, which returns a `WaninfoStats(domain, BytesSent, BytesReceived, PacketsSent, PacketsReceived, ...)` object. The rate is calculated from two readings, and the three ways it can go wrong are handled as distinct outcomes rather than as a number: a counter that went down means the router reset it and produces no rate, two readings with no time between them divide by nothing, and two readings too far apart are marked stale rather than presented as current. A counter that did not move is a rate of zero, which is a real reading of an idle line and different from having no rate at all. The cumulative counters are shown as totals and never labelled as rates._

- [ ] 12. **Traffic history and charts** - display historical upload and download rates, cumulative traffic counters, and peak observed rates using locally stored measurements. _Note: **gated on 5 and 11**, and 11 is blocked._

- [ ] 13. **Connected-device overview** - display verified router-reported device names, connection types, online states, and supported wireless details without inventing unavailable device information. _Note: **removed rather than deferred.** The router's device list is behind `html/bbsp/userdevinfo/getuserdevinfo.asp`, which answers `"NONE"`: the router builds the table lazily from an ARP scan, and the file does not exist until that scan has run. The screen could only ever have said so, so it and the panel's device row were taken out rather than left as a permanent apology. The Wi-Fi radios the router does report are shown on the Fibre & Router screen instead._

- [x] 14. **Router health monitoring** - display verified ONT registration, WAN connection state, and other supported router health metrics where reliable data sources are available. _Note: **shipped and wired.** The session reports WAN state, address and its classification, gateway, DNS, MTU, BRAS, access type and session uptime. Optical receive power is real and shown. No temperature, voltage, or bias current: `temp1`/`temp2` are `0`, so those fields are absent from the wire type entirely rather than being rendered as zero. CPU and memory are shown only when the firmware's own `showCPUnMemUsed` flag enables them. Four more pages are read and shown on the same screen: the Wi-Fi radios, which this router reports as `ath0` and `ath4` with both switched on; the access type of the WAN, which it reports as `GPON`; the router's word for its IPv6 state, which on the observed connection is `Invalid`; and whether it says it supports IPv6, which it does. Seven pages are read in total._

- [ ] 15. **Optical signal monitoring** - moved to Post-MVP (46). No verified router source or field semantics have been identified, and optical readings must not block MVP completion. _Note: receive power **is** verified on this firmware as `opticInfo` (observed -18.83 dBm) and is shown under feature 14. Transmit power, temperature, voltage, and bias current remain unverified and stay here._

- [ ] 16. **Monitoring settings** - allow users to configure probe targets, intervals, timeouts, health thresholds, router address, history retention, and monitoring behaviour, with the settings that exist today (appearance, and later pause/resume from feature 9) surfaced honestly rather than as dead controls.

- [ ] 17. **Native notifications** - notify users about sustained connectivity loss and recovery with configurable preferences and duplicate-alert prevention. _Note: wording describes what the app measured and never names a cause._

- [x] 18. **Light and dark appearance** - support system, light, and dark themes on the dark bento design system from feature 1b, with one appearance control whose icon reflects the current mode. _Note: shipped. The topbar carries one appearance button whose glyph shows the mode that was chosen, so System never silently reads as Dark. The clock format is a setting beside it: System, 24-hour or 12-hour, applied everywhere a time appears, including the incident times and the range the latency history covers._

- [ ] 19. **Diagnostic export** - generate a reviewable diagnostic summary containing relevant connectivity history, incident details, and available router metrics without exposing credentials or session tokens

- [ ] 20. **Error handling and recovery** - handle failed probes, unavailable router endpoints, expired sessions, malformed responses, network changes, sleep and wake transitions, and interrupted monitoring without crashing or emitting a fabricated value

- [ ] 21. **Privacy and security hardening** - keep monitoring local by default, protect router authentication material, redact sensitive data, and enforce read-only router access

- [ ] 22. **MVP testing and verification** - verify connectivity calculations, connection-state transitions, incident detection, traffic-rate calculations, SQLite persistence, menu-bar behaviour, background lifecycle, and router response parsing

- [ ] 23. **macOS packaging and deployment readiness** - package the React, TypeScript, and Vite interface with Tauri 2 and its Rust backend for local macOS installation, then verify the packaged application outside development mode

- [x] 47. **Internet speed test** - measure the line to the internet on demand, from inside the app, without going to a website first. _Note: **shipped.** Its own screen and its own rail icon, because a speed test is a measurement you ask for and wait for rather than a reading that is always there. It measures three things against the same host: the round trip, the download rate, and the upload rate, each from a real transfer. The transfers are measured the way the speed tests people compare it against measure: six connections at once, a ten-second window, the opening second thrown away, and a continuous streamed upload so no request carries its own ramp into the figure. The download runs first so the latency is the round trip over pooled connections and not the cost of opening one. The three figures are independent, so a line that uploads at nothing but downloads at 200 Mbps still shows the download it managed. The round trip is to Cloudflare's edge, and the screen names the host, because a website test's ping is usually a measurement of a different path. The screen shows a speedometer dial, logarithmic between named marks, with the fresh figure in the middle of the dial; while the test runs the needle scans the scale, as speed tests have always shown, and the figure and the filled arc are the real rate the backend publishes as the transfer progresses. The needle settles on the measured value when the test ends._

## Post-MVP

- [ ] 24. **Advanced connectivity diagnostics** - add optional diagnostic tools such as traceroute and route-change inspection where practical and supported by macOS permissions

- [ ] 25. **Extended connectivity target management** - allow users to manage multiple external targets and compare their reachability, latency, and packet failure

- [ ] 26. **Advanced performance analysis** - provide configurable time ranges, latency percentiles, peak values, historical comparisons, and longer-term connectivity trends

- [ ] 27. **Network change detection** - detect relevant network-interface, gateway, or connectivity changes and annotate the monitoring timeline

- [ ] 28. **Sleep and wake recovery improvements** - improve monitoring recovery after Mac sleep, wake, network transitions, and changes in available network interfaces

- [ ] 29. **Advanced incident analysis** - correlate router reachability, external probes, probe failures, and available WAN statistics to provide evidence-based incident summaries without asserting unsupported causes

- [ ] 30. **Router endpoint compatibility layer** - improve support for firmware differences, endpoint variations, response validation, authentication-session recovery, and changes in router response formats

- [ ] 31. **Additional router metrics** - add further verified router information as compatible endpoints and field semantics are discovered and validated

- [ ] 32. **Advanced device information** - improve device labelling, online history, Wi-Fi signal detail, and device organisation where supported by the router

- [ ] 33. **History retention and storage management** - add configurable retention policies, historical aggregation, storage usage information, and cleanup controls for the local SQLite database

- [ ] 34. **Data portability and history export** - support exporting selected monitoring history, traffic measurements, and incidents in practical formats such as CSV or JSON

- [ ] 35. **Expanded notification controls** - add notification schedules, quiet periods, threshold customisation, and per-event preferences

- [ ] 36. **Launch at login** - allow users to optionally start FibreX Monitor when they sign in to macOS using an appropriate native integration

- [ ] 37. **Menu-bar customisation** - allow users to choose which live metric or connection indicator appears in the menu bar where supported by the native integration

- [x] 38. **Optional active speed testing** - add user-initiated download and upload tests with clear bandwidth, duration, and data-usage implications, separate from passive traffic monitoring. _Note: shipped as feature 47. The screen states what a test costs in time and data (ten seconds each way, tens of megabytes depending on the line) and runs on demand only, never in the background, so it stays separate from the passive monitoring loop. Its transfer phases open several connections at once, the way the speed tests people compare it against do, so a user's figures from this screen and from a website describe the same measurement._

- [ ] 39. **Additional router and connection support** - make the monitoring architecture adaptable to other compatible routers and internet connections without compromising the FibreX-first experience

- [ ] 40. **Accessibility and usability refinement** - improve keyboard navigation, VoiceOver support, chart accessibility, reduced-motion behaviour, and small-window layouts

- [ ] 41. **Performance and resource optimisation** - reduce unnecessary polling, memory use, database growth, and background CPU consumption during long-running monitoring

- [ ] 42. **Operational diagnostics and logging** - add privacy-conscious application logs and troubleshooting information for monitoring failures, native integration issues, and router connectivity problems

- [ ] 43. **Backup and recovery for local history** - provide safe options for backing up and restoring monitoring history where useful

- [ ] 44. **Production security and compatibility review** - review local permissions, credential handling, dependency security, macOS compatibility, Tauri capabilities, Rust commands, and router integration safety

- [ ] 45. **Release and update workflow** - establish a repeatable application release process, validate packaged builds, and investigate code signing, notarisation, and signed updates if the project becomes a distributed product

- [ ] 46. **Optical signal monitoring** - retrieve and display optical transmit power, receive power, temperature, voltage, and bias current only once a reliable router data source and its field semantics have been verified. _Note: moved here from MVP 15, which stays as a placeholder for the number. Receive power **is** verified on this firmware as `opticInfo` (observed -18.83 dBm) and is shown under feature 14. Transmit power is **not**: the router's own dashboard calls `/html/ssmp/common/getOpticTxRx.asp` for a `TxPower` value, but that path answers 404 on this firmware, so the transmit figure the router asks for does not exist. Temperature, voltage, and bias current remain unverified._

The MVP succeeds if a user can check FibreX health from the macOS menu bar, monitor real latency and probe failures, tell router reachability apart from external connectivity, review outages and recovery times, and view verified WAN traffic statistics **when a verified router session is available**, without a cloud service and without changing router configuration. Unavailable router data is never shown as a number, and every reading is labelled measured, calculated, stale, or unavailable.

## Implementation constraints

- **Frontend:** React, TypeScript, and Vite. Use React for the interface and shared application state. Do not introduce Angular.
- **Desktop runtime:** Tauri 2 with Rust for native macOS integration, background monitoring, and privileged system operations where required.
- **Local storage:** SQLite for monitoring history, traffic samples, incidents, and persisted settings.
- **Monitoring architecture:** one backend monitoring service shared by the menu-bar popover and main window. Closing the main window must not create a second monitoring loop or automatically stop monitoring.
- **Router integration:** read-only by design. Router authentication must follow the router's supported login/session mechanism. Never hardcode credentials or log authentication tokens.
- **Data integrity:** distinguish measured values, calculated values, stale values, and unavailable values. Do not display fabricated metrics or treat missing data as zero.
- **Graceful degradation:** connectivity monitoring must continue when router integration is unavailable, and the app must remain useful when the router does not expose optical or advanced health metrics.
- **Local-first operation:** core monitoring, history, incident tracking, and dashboard functionality must work without a cloud account or hosted backend.
- **Scope discipline:** scaffolding, dependency installation, initial Tauri setup, development scripts, and repository configuration are pre-build tasks, not checklist features. The build loop starts at feature 1.
