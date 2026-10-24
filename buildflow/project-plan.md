# Project Plan

> Project plan for a modern, local-first macOS menu-bar application that monitors an MTN FibreX internet connection. The app combines live connectivity checks, latency and packet-loss tracking, WAN traffic statistics, router health, and historical incident records in a polished desktop experience.
>
> The application will be built with React, TypeScript, Tauri 2, Rust, and SQLite. It will run as a standalone macOS desktop application, with a menu-bar interface for quick access and a main window for detailed monitoring. It will not require a browser window, hosted backend, or cloud service for ordinary use.

## 1. Problem - What problem are we solving?

Internet connectivity problems are difficult to diagnose when the only available tools are the router's administration dashboard and commands such as `ping` in Terminal.

When FibreX becomes slow or unstable, it is not immediately clear:

- Whether the Mac can reach the router.
- Whether the router can reach the wider internet.
- Whether latency is unusually high.
- Whether packets are being lost.
- Whether the connection is completely offline or temporarily degraded.
- How long an interruption lasted.
- Whether the connection recovered on its own.
- How much data is being transferred.
- Whether router health information can help explain the problem.

The existing workflow requires manually running commands, checking the router interface, and trying to remember when interruptions occurred.

A better solution is a small macOS utility that continuously monitors the connection, keeps a local history, and makes useful network information accessible directly from the macOS menu bar.

The product should combine basic network diagnostics with verified router statistics without becoming an unnecessarily complicated network administration tool.

The core product becomes:

> **A personal FibreX monitoring dashboard that tells you when your internet connection is healthy, when it is struggling, and what happened when it went down.**

### Core product principle

**Monitor continuously.**

**Understand the connection.**

**Keep the evidence locally.**

The app should remain useful even if router integration is unavailable. Basic latency and connectivity monitoring must work independently of the router's authenticated administration interface.

The application is intended for personal monitoring of the user's own network. It must not change router configuration or attempt to bypass authentication.

### Desktop application principle

FibreX Monitor is a standalone macOS application, not a website that the user must open in Chrome or Safari.

- React and TypeScript implement the visible interface.
- Tauri 2 packages the interface into a desktop application and provides access to native desktop capabilities.
- Rust handles native monitoring tasks, system integration, and backend operations where appropriate.
- SQLite stores measurements, incidents, and other local history.
- The macOS menu bar provides quick access to connection status without requiring the main application window to remain open.

The main dashboard and menu-bar popover are parts of the same desktop application. They must share one monitoring service and one source of truth for application state.

---

## 2. Users - Who is this for?

### Primary users

- The owner of an MTN FibreX connection who wants better visibility into internet performance.
- Mac users who currently run `ping` or other network commands manually.
- People who experience intermittent disconnections and want a reliable incident history.
- Users who want a quick connection-health summary without opening the router dashboard.
- People who want to distinguish router reachability problems from external connectivity problems.

### Secondary users

- Home users who want a lightweight local network monitor.
- Developers who need to observe connectivity while working.
- Users who want historical latency, packet-loss, and traffic information.
- Users who want to inspect available router health readings and connected devices.

### Non-target users for the MVP

- Internet service providers needing a centralised network operations platform.
- Enterprises requiring a multi-user network administration platform and access controls.
- Users seeking a full packet-capture or security-monitoring suite.
- Users needing guaranteed per-device bandwidth accounting without router support.
- Users who need the app to automatically repair or reconfigure their router.

The product is intentionally personal-first, local-first, and read-only.

---

## 3. Features - What does the MVP need?

### Core features

- Native macOS desktop application.
- React-based main dashboard.
- macOS menu-bar icon with a compact connection-status popover.
- Real connectivity monitoring that works without router authentication.
- Router reachability checks.
- External internet reachability checks.
- Latency measurements.
- Packet-loss tracking over a defined measurement window.
- Connection states: Connected, Degraded, Offline, Unknown, and Paused.
- Configurable probe targets, intervals, timeouts, and thresholds.
- Historical latency and packet-loss charts.
- Detection and recording of connectivity incidents.
- Incident start time, recovery time, and duration.
- Local persistence of measurements and incident history.
- WAN download and upload rate estimates from verified router counters, when available.
- Router data-source status and last successful refresh time.
- Clear loading, empty, stale, unavailable, and error states.
- Light mode, dark mode, and system appearance.
- Settings for monitoring behaviour and notification preferences.
- Manual pause and resume controls.
- A clear indication of when monitoring is active.
- A standalone application bundle that launches independently of a web browser.
- Shared monitoring state between the main window and the menu-bar interface.
- Background monitoring that continues when the main window is closed.
- Clean monitoring shutdown when the user explicitly quits the application.

### Supporting features

- macOS notifications when connectivity is lost or restored.
- Sustained high-latency and packet-loss alerts.
- Connected-device information from verified router endpoints.
- Router health information where automated retrieval has been verified.
- Optical signal readings if a reliable router data source is identified.
- CPU and memory readings if supported by the router.
- Small latency and traffic sparklines in the menu-bar popover.
- Configurable history retention.
- Diagnostic summary export.
- Safe handling of router session expiration.
- Monitoring recovery after network changes and Mac sleep/wake.
- Optional launch-at-login support.
- Clear documentation of unavailable router metrics.
- A compact and responsive macOS application window.
- Keyboard-accessible controls and appropriate accessibility labels.
- Clear indicators for monitoring activity, data freshness, and router integration status.

### Router integration requirements

The router is a Huawei OptiXstar HG8145X7-10 GPON terminal running MTN-customised firmware.

The following endpoints have already been observed during manual inspection of the router interface. Their availability, response formats, authentication requirements, and field meanings must be verified against the actual router before production integration.

#### WAN statistics

`GET /html/bbsp/common/get_wan_list_pppwanstat.asp`

The response uses the `WaninfoStats` constructor and includes cumulative `BytesSent`, `BytesReceived`, `PacketsSent`, and `PacketsReceived` counters.

The app should investigate safe authenticated access, verify counter direction and units, and calculate transfer rates from successive readings.

Cumulative counters must never be presented as instantaneous speeds.

Rate calculations should use elapsed time between valid counter samples. If counters reset, decrease unexpectedly, or become unavailable, the app must reset or suspend the affected calculation instead of generating misleading spikes.

#### Wireless topology

`POST /html/amp/wificoverinfo/getTopoInfo.asp`

The observed response includes a topology object and wireless client entries containing fields such as `HostName`, `AccessType`, `WifiFreq`, `WirelessMode`, `RSSI`, `TX`, `RX`, and `RealTimeBandWidth`.

Only verified fields should be used. Unknown units and fields must not be presented as confirmed bandwidth measurements.

Wireless signal strength, negotiated link rates, and actual data throughput are different measurements and must not be treated as interchangeable.

#### LAN device information

`POST /html/bbsp/common/GetLanUserDevInfo.asp`

The observed response includes device information such as hostname, device status, connection type, and network identifiers.

Use this source for device information only after confirming the field meanings and online-status behaviour.

Hide MAC addresses and private IP addresses by default. Do not retain unnecessary device identifiers or expose them in diagnostic exports unless the user explicitly requests them.

#### Optical and router health

The router interface displays optical transmit and receive power, temperature, working voltage, and bias current. Automated retrieval of these values has not yet been confirmed.

The endpoint `/html/ssmp/common/GetDeviceState.asp` has been observed, but its response has not been established as a source of optical readings.

Do not invent an endpoint or infer optical measurements from unrelated status responses.

Before implementing optical or hardware-health readings:

- Identify the actual authenticated request used by the router interface.
- Inspect and validate the response.
- Confirm each field's meaning and unit.
- Confirm whether the value represents a current reading or a historical/status value.
- Handle missing, stale, or invalid readings explicitly.
- Record the source and last successful retrieval time.
- Keep unsupported metrics unavailable rather than substituting estimates.

#### Router integration safety

- Use the router's normal authentication mechanism.
- Do not bypass authentication or exploit undocumented security weaknesses.
- Do not hardcode router credentials, session cookies, or request tokens.
- Do not log sensitive authentication headers or session material.
- Use bounded request timeouts and conservative polling intervals.
- Avoid simultaneous duplicate polling from different UI components.
- Do not make router configuration changes.
- Keep router access read-only.
- Ensure the core monitoring experience continues when router access fails.

### Explicitly out of MVP

- Changing router settings.
- Rebooting the router from the app.
- Changing Wi-Fi credentials.
- Firewall, WAN, or DNS configuration.
- Automatic router repair.
- Packet capture and deep packet inspection.
- Full network-security monitoring.
- Remote access through a hosted service.
- Cloud synchronisation and user accounts.
- A hosted analytics backend.
- Guaranteed per-device bandwidth monitoring without a verified source.
- Continuous automatic speed tests.
- Automated claims about the exact cause of ISP outages.
- Expensive third-party monitoring integrations.
- A mandatory browser-based dashboard.
- A mandatory hosted web frontend.
- A cloud database or backend required for core functionality.

### Success criteria for MVP

The user should be able to:

1. Install and launch the macOS application.
2. Open FibreX Monitor directly from macOS without opening a browser.
3. See its connection status from the menu bar.
4. Open a main dashboard with real monitoring information.
5. Monitor real latency and packet loss.
6. Distinguish local router reachability from external connectivity.
7. Review connectivity history and recorded incidents.
8. See recovery times and outage durations.
9. View WAN traffic rates when router counters are accessible and validated.
10. Continue using the core monitoring features when router integration is unavailable.
11. Close the main window without unintentionally stopping background monitoring.
12. Quit the app and have monitoring stop cleanly.
13. See explicit unavailable or stale states instead of fabricated measurements.
14. Restart the app and recover persisted measurements and incident history.
15. Use the application without creating an account or depending on a hosted service.

If these workflows work reliably and the app does not misrepresent unavailable or unverified data, the MVP is successful.

---

## 4. Data - What are we storing?

The application is local-first. Network history should remain on the user's Mac unless the user explicitly exports it.

### Connectivity data

- Sample ID.
- Probe timestamp.
- Target identifier or configured target name.
- Probe method.
- Success or failure.
- Latency measurement, when successful.
- Timeout or error category.
- Monitoring state.
- Relevant probe configuration.

Probe records should distinguish router reachability from external reachability. A failed external probe alone must not automatically establish that the entire internet connection is offline.

### Traffic data

- Sample ID.
- Collection timestamp.
- Cumulative WAN bytes sent, when available.
- Cumulative WAN bytes received, when available.
- Calculated upload rate.
- Calculated download rate.
- Counter-source status.
- Last successful collection time.

Rates should be calculated from counter differences and elapsed time.

The application must handle counter resets, router restarts, rollover, and missing samples without creating misleading spikes.

When counters are unavailable, traffic rates should be marked unavailable rather than displayed as zero.

### Router health data

When supported by verified sources:

- Collection timestamp.
- ONT registration state.
- WAN connection state.
- Optical transmit power.
- Optical receive power.
- Temperature.
- Working voltage.
- Bias current.
- CPU utilisation.
- Memory utilisation.
- Source availability and freshness.

Each metric must include its unit, source, and last update time.

Unavailable metrics should remain unavailable. Do not substitute invented readings or development mock data.

### Device data

When supported by verified router responses:

- Device display name or hostname.
- Online/offline state.
- Connection type.
- Wi-Fi band.
- Signal strength, when available.
- Last refresh time.

Avoid persisting unnecessary device identifiers. MAC addresses and private IP addresses should not be shown by default.

### Incident data

- Incident ID.
- Incident type.
- Start timestamp.
- Recovery timestamp, when resolved.
- Duration.
- Severity.
- Relevant latency and packet-loss summary.
- Router reachability evidence.
- External connectivity evidence.
- Incident classification.
- Resolution state.

The app should distinguish observed facts from suspected causes.

Incident classifications should be based on evidence from the monitoring engine. For example, a router that remains reachable while external probes fail provides different evidence from a router that becomes unreachable at the same time.

The app must not claim that MTN is responsible for an outage unless the available evidence supports that conclusion.

### Application settings

- Monitoring enabled or paused.
- Probe interval and timeout.
- Configured probe targets.
- Latency and packet-loss thresholds.
- Recovery policy.
- Notification preferences.
- History retention.
- Appearance preference.
- Router base URL.
- Router integration status.
- Optional launch-at-login preference.
- Menu-bar display preferences, where applicable.

Sensitive router credentials must not be stored in ordinary application preferences as plaintext.

### What we are not storing in the MVP

- Router passwords in plain text.
- Session cookies or request tokens in application logs.
- Unnecessary raw router responses.
- Cloud copies of network history.
- Advertising profiles.
- Behavioural tracking.
- Unnecessary personal device identifiers.
- Packet contents.
- User accounts or remote telemetry.
- Network information that is not needed for monitoring.

### Runtime and temporary data

- Current connection state.
- Latest successful probe.
- Previous WAN counter sample.
- In-flight router requests.
- Monitoring task status.
- Temporary chart data.
- Current incident evaluation state.
- Current network-interface state, where needed.
- Pending notification state.
- Router authentication/session state in memory, when necessary.

Runtime state should be reconstructed from persisted history where practical. Temporary values should not be mistaken for durable records.

The main window and menu-bar popover must read from the same monitoring service. They must not each create their own independent monitoring loops.

**Data principle:** Collect only what is necessary to measure, explain, and troubleshoot the connection.

---

## 5. Tech - What stack are we using?

### Frontend

- React.
- TypeScript.
- Vite for frontend development and production asset builds.
- React functional components and hooks.
- React Router only if route-based navigation is needed for the main window.
- A lightweight React-compatible state-management approach.
- Tailwind CSS for styling.
- CSS variables for design tokens, colour themes, and shared interface properties.
- A lightweight charting library compatible with React.
- Lucide React for icons.

The frontend should handle navigation, rendering, charts, settings, and user interactions.

React should not independently create competing monitoring loops. Network monitoring, incident evaluation, router polling, and durable storage belong in the backend.

Use React state and context for straightforward shared UI state. Add a dedicated state-management library only if the implementation demonstrates a genuine need for one.

Avoid unnecessary frontend dependencies and complex abstractions.

### Frontend responsibilities

React is responsible for:

- The main desktop dashboard.
- Overview, connectivity, traffic, router, devices, incidents, and settings screens.
- Rendering live monitoring updates received from the backend.
- Displaying latency and packet-loss charts.
- Displaying validated traffic statistics.
- Showing loading, empty, stale, unavailable, and error states.
- Managing user interface preferences and form interactions.
- Sending monitoring configuration changes through typed Tauri commands.
- Subscribing to relevant Tauri events and cleaning up event listeners.
- Displaying notifications and status messages when appropriate.
- Sharing a consistent design system between the main window and menu-bar popover.

React must not be responsible for privileged native operations or persistent monitoring tasks that should survive closure of the main window.

### Desktop application

- Tauri 2.
- Rust backend.
- Tauri commands for typed communication between React and Rust.
- Tauri events for live monitoring updates.
- A compatible tray or menu-bar integration for macOS.
- Native macOS notifications where supported.
- A standalone macOS application bundle.
- A dedicated main window and a compact menu-bar popover.
- Appropriate application lifecycle handling.
- Optional launch-at-login support, if implemented and verified.

Tauri provides the desktop shell and native integration. React renders the user interface inside the application window.

The application must not require the user to open Chrome, Safari, or another browser to use the dashboard.

### Desktop window and menu-bar architecture

The application should use one shared backend monitoring service.

The main window and menu-bar popover are separate presentation surfaces, not separate monitoring systems.

The shared backend should:

- Start monitoring according to the saved application configuration.
- Maintain the current connection state.
- Perform scheduled connectivity probes.
- Coordinate router polling.
- calculate traffic rates from validated counters.
- Record measurements and incidents.
- Emit updates to any open frontend window.
- Support monitoring pause and resume.
- Handle network changes and sleep/wake recovery where practical.
- Shut down tasks cleanly when the application quits.

Closing the main window should not stop monitoring. If the menu-bar interface remains active, the monitoring service should continue running.

The exact behaviour of window closure, menu-bar activation, application termination, and background monitoring must be tested in the packaged macOS application.

### Monitoring and network diagnostics

- Rust for monitoring coordination and native integration.
- ICMP ping where suitable and supported.
- Configurable external reachability targets.
- Local router reachability checks.
- Bounded retries and explicit timeouts.
- A central connection-state engine.
- A traffic-rate calculation module.
- A router client that uses only verified read-only endpoints.
- A controlled background task lifecycle.
- Cancellation of in-flight operations during shutdown.
- Explicit handling of missing probes and stale samples.

Do not assume that every external host responds to ICMP. A failed ping must not automatically be treated as proof that the entire internet connection is offline.

Where suitable, combine more than one external reachability target or probe method to reduce false outage classifications.

### Backend / Database

- SQLite for local history.
- Rust data-access modules.
- Explicit schema migrations.
- Typed models for measurements and incidents.
- Local retention and cleanup logic.
- Transactional writes for related incident and measurement records.
- Appropriate indexes for timestamp-based history queries.
- Safe database initialization and recovery behaviour.
- A stable database location within the application's macOS application-data directory.

There is no need for a hosted database or remote API in the MVP.

The database schema should support historical charts, incident summaries, retention cleanup, and application restarts without loading the entire history into memory.

### Router integration

- Local HTTP requests to the router.
- Configurable router base URL.
- Explicit request timeouts.
- Response validation and parsing.
- Conservative polling intervals.
- Safe session handling.
- macOS Keychain if persistent secret storage is necessary.
- Clear separation between router transport, authentication, parsing, and domain models.
- Graceful handling of unavailable endpoints and expired sessions.

Do not hardcode credentials, active session cookies, session IDs, or request tokens.

Do not bypass router authentication.

Router integration should be implemented through a dedicated backend module. React components should never directly issue authenticated router requests or independently parse raw router responses.

### Charts and history

Use a lightweight charting library for:

- Latency over time.
- Packet loss over time.
- Download and upload rates.
- Incident markers.
- Historical summaries.

Charts must reflect actual stored measurements.

Show empty or unavailable states when there is insufficient data.

Do not interpolate missing measurements in a way that suggests real measurements were collected. If aggregation or downsampling is needed for longer history periods, it must preserve the distinction between observed and derived values.

### Suggested project structure

The exact structure should be confirmed after repository inspection. The following is the intended separation of responsibilities:

```text
fibrex-monitor/
├── src/
│   ├── app/
│   │   ├── App.tsx
│   │   ├── router.tsx
│   │   └── providers.tsx
│   ├── components/
│   │   ├── layout/
│   │   ├── navigation/
│   │   ├── charts/
│   │   ├── status/
│   │   └── ui/
│   ├── features/
│   │   ├── overview/
│   │   ├── connectivity/
│   │   ├── traffic/
│   │   ├── router/
│   │   ├── devices/
│   │   ├── incidents/
│   │   └── settings/
│   ├── hooks/
│   ├── lib/
│   │   ├── tauri/
│   │   ├── formatting/
│   │   └── validation/
│   ├── styles/
│   ├── types/
│   └── main.tsx
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── commands/
│       ├── monitoring/
│       ├── router/
│       ├── incidents/
│       ├── notifications/
│       ├── storage/
│       └── models/
├── public/
├── package.json
├── tsconfig.json
├── vite.config.ts
└── README.md
```

This is a proposed structure, not a requirement to create empty files or modules in advance. Build only the modules required by the current implementation stage.

### Architecture overview

```text
User
 │
 ▼
macOS Menu Bar
 │
 ├── Compact Status Popover
 │
 └── Main Tauri Window
         │
         ▼
      React UI
         │
         ▼
   Tauri Commands / Events
         │
         ▼
      Rust Backend
         │
         ├── Monitoring Coordinator
         │     ├── Router Reachability
         │     └── External Reachability
         │
         ├── Router Client
         │     ├── WAN Statistics
         │     ├── Device Information
         │     └── Verified Health Metrics
         │
         ├── Traffic Rate Calculator
         │
         ├── Incident Engine
         │
         ├── Notification Service
         │
         └── SQLite
               ├── Connectivity History
               ├── Traffic History
               ├── Router Snapshots
               └── Incidents
```

### Important constraints

- Keep the stack local-first and free of required hosted services.
- Build the UI before completing every router integration.
- Keep the application buildable throughout development.
- Avoid excessive abstractions and unnecessary dependencies.
- Use React and TypeScript for the frontend.
- Use Tauri 2 and Rust for the desktop runtime and native functionality.
- Keep router access read-only.
- Avoid excessive polling and unbounded background tasks.
- Do not treat unverified endpoint fields as reliable metrics.
- Do not silently replace failed real requests with mock data.
- Keep secrets out of source code, logs, and diagnostic exports.
- Use mock data only for clearly identified development and UI-preview purposes.
- Keep monitoring logic out of React component lifecycle effects.
- Prevent duplicate monitoring tasks when windows open, close, or reload.
- Use Tauri capabilities and permissions conservatively.
- Validate that the application works as a packaged macOS app, not only in browser-based development mode.
- Do not assume that web-browser behaviour and packaged Tauri behaviour are identical.
- Keep the frontend, monitoring service, router client, and database responsibilities clearly separated.

---

## 6. Monetize - How will this make money?

### MVP phase

The application is a free personal utility.

The initial goal is to build a reliable monitoring experience for the user's own FibreX connection, not to introduce payments or commercial infrastructure.

### Future possibilities

If the application eventually becomes a public product, possible directions include:

- A free personal network-monitoring utility.
- Optional advanced historical reports.
- Additional router integrations.
- Export and diagnostic improvements.
- Support for other home internet connections.
- Optional advanced monitoring features for technically inclined users.

These are possibilities, not MVP commitments.

### Explicit early exclusion

- No advertising.
- No subscription billing.
- No paid cloud backend.
- No forced account creation.
- No paywall for core monitoring.
- No monetisation work before the application is useful and reliable.

The first version should focus entirely on utility, reliability, and a good macOS experience.

---

## 7. UI/UX - How should this look and feel?

The application should feel like a modern, well-crafted macOS utility: compact in the menu bar, detailed in the main window, and visually restrained throughout.

It is a network-monitoring instrument, not a generic admin dashboard or an AI analytics interface.

### Core principles

- Modern macOS-inspired design.
- Clear visual hierarchy.
- Compact, useful information.
- Fast access to connection status.
- Consistent light and dark themes.
- Subtle animations only where useful.
- Clear typography and readable charts.
- Minimal visual noise.
- Accurate status labels.
- Accessible controls.
- Responsive desktop layouts.
- Native-feeling interactions.
- Consistent interaction patterns across the main window and menu-bar popover.
- Clear differentiation between actual readings, derived calculations, stale values, and unavailable information.

Avoid excessive gradients, glassmorphism, oversized metric cards, decorative charts, and unnecessary rounded containers.

### Main screens

1. **Overview**
   - Current connection status.
   - Latency and packet loss.
   - Download and upload rates, when available.
   - Recent connection history.
   - Recent incidents.
   - Router data-source status.
   - Monitoring activity and last successful probe.
   - A concise indication of whether the connection is healthy, degraded, or offline.

2. **Ping & Connectivity**
   - Router reachability.
   - External connectivity.
   - Latency history.
   - Packet-loss history.
   - Configured probe targets.
   - Recent failures and recovery events.
   - Probe intervals and timeouts.
   - Clear distinction between failed probes and confirmed loss of internet connectivity.

3. **Traffic**
   - Download and upload rates.
   - Historical traffic chart.
   - Cumulative WAN counters.
   - Peak observed rates.
   - Last successful router-statistics refresh.
   - Counter-source status.
   - Clear indication when traffic information is unavailable.

4. **Fibre & Router**
   - ONT registration status.
   - WAN connection state.
   - Optical power, if available.
   - Temperature, voltage, and bias current, if available.
   - CPU and memory, if available.
   - Metric source and last update time.
   - Router connection and authentication status.
   - Clear unavailable states for unsupported metrics.

5. **Devices**
   - Connected-device list.
   - Hostname or display name.
   - Connection type.
   - Wi-Fi band and signal strength, when available.
   - Online/offline status.
   - Last refresh time.
   - Privacy-conscious handling of MAC addresses and private IP addresses.

6. **Incidents**
   - Chronological outage and degradation history.
   - Start time and recovery time.
   - Duration.
   - Relevant diagnostic measurements.
   - Confirmed or suspected classification.
   - Evidence used to classify the incident.
   - Filters for incident type and time range.
   - Empty state when no incidents have been recorded.

7. **Settings**
   - Monitoring interval.
   - Probe targets and timeout.
   - Latency and packet-loss thresholds.
   - Notifications.
   - Appearance.
   - History retention.
   - Router connection settings.
   - Diagnostic export.
   - Clear local history.
   - Launch-at-login preference, if supported.
   - Monitoring pause and resume behaviour.

8. **Menu-Bar Popover**
   - Compact connection status.
   - Current latency.
   - Packet loss.
   - Download and upload speeds, when available.
   - Small recent-performance sparkline.
   - Last update time.
   - Pause/resume monitoring.
   - Open dashboard action.
   - Clear indication of unavailable or stale data.
   - A route to relevant settings or detailed diagnostics.

### Main desktop window

The main window is the full monitoring interface. It should be a normal macOS application window with clear navigation and enough space for charts, diagnostic details, and historical records.

It must:

- Launch from the Dock, Applications folder, or other normal macOS launch mechanisms.
- Open without requiring a browser.
- Support resizing within sensible minimum dimensions.
- Display loading, error, empty, and stale states properly.
- Use the same monitoring state as the menu-bar interface.
- Remain responsive while background monitoring and database writes occur.
- Avoid blocking the UI while waiting for network requests.
- Restore a sensible window size and appearance where practical.

The main window must not own the lifetime of the monitoring service.

### Menu-bar behaviour

The menu-bar interface should make the most important information available without opening the full window.

The main window should be closable without unintentionally stopping monitoring. Quitting the application should stop monitoring cleanly.

The menu-bar popover should remain small and focused. Detailed history, device information, configuration, and long charts belong in the main window.

The exact tray behaviour should be validated on macOS using the selected Tauri 2 integration.

The implementation must account for the fact that macOS menu-bar integration and native application lifecycle behaviour are separate concerns from React rendering.

### Tone of the interface

Use plain, direct language:

- “Connected”
- “Connection degraded”
- “Internet unreachable”
- “Router unreachable”
- “Monitoring paused”
- “Waiting for sample”
- “Last updated 10 seconds ago”
- “Router statistics unavailable”
- “Connection restored”

Avoid unexplained network jargon and do not label a suspected outage as a confirmed ISP fault.

### Status and chart behaviour

- Green or an equivalent accessible success indicator for healthy states.
- Amber or an equivalent warning indicator for degradation.
- Red or an equivalent error indicator for confirmed connectivity failure.
- Neutral styling for unknown, stale, or unavailable data.
- Text labels in addition to colour.
- Timestamps and units for measured values.
- Clear empty states when no history exists.
- No fabricated live data.
- No misleading zero values when a source has failed.
- No unexplained gaps or artificial continuity in historical charts.
- Clear separation between measured values and calculated values.
- Appropriate loading indicators while new measurements are being collected.

### Appearance

Support:

- System appearance.
- Light mode.
- Dark mode.

System appearance should be the default.

Use shared design tokens and CSS variables so that colours, borders, typography, spacing, and status indicators remain consistent across screens.

### Responsive behaviour

The application should work well in a typical MacBook window and when resized.

The menu-bar popover should remain compact, while the main dashboard should use its available space efficiently.

The frontend should use responsive desktop layouts without treating the application as a mobile-first website.

Navigation, charts, tables, and settings should adapt to narrower windows without becoming unusable.

### Interaction and accessibility

- Use keyboard-accessible navigation and controls.
- Provide clear focus indicators.
- Use meaningful accessible labels for icon-only buttons.
- Ensure status is communicated with text and not colour alone.
- Support standard macOS window and keyboard interaction expectations where practical.
- Avoid animations that interfere with monitoring information.
- Ensure charts have readable labels and summaries.
- Confirm destructive actions, such as clearing history.
- Keep monitoring pause/resume actions clearly distinguishable from quitting the application.

---

## 8. Deployment - Where and how will this ship?

### Application platform

- Build the application for macOS using Tauri 2.
- Use React and TypeScript for the frontend.
- Use Vite for frontend development and production builds.
- Use Rust for native monitoring and desktop integration.
- Target Apple Silicon Macs first.
- Verify the actual macOS deployment target and build requirements during setup.
- Package the result as a standalone macOS application.
- Ensure ordinary use does not require a browser or hosted web service.

### Development environment

- Node.js and npm for React dependencies.
- Vite for frontend development and production builds.
- TypeScript for frontend type checking.
- Rust and Cargo for the native backend.
- Tauri CLI and compatible plugins.
- Xcode Command Line Tools as required by the local environment.
- A compatible Rust toolchain for the chosen Tauri version.
- SQLite tooling or Rust-compatible database libraries.
- Testing tools appropriate to the frontend and Rust backend.

The implementation agent must inspect the current environment before installing or changing tooling.

Use the existing Node.js and Rust installations where compatible. Do not replace working toolchains without a clear reason.

### Local storage

- SQLite database stored in the application's appropriate macOS application-data directory.
- Local retention and cleanup.
- No remote database.
- No cloud synchronisation in the MVP.
- Explicit database schema migrations.
- Safe handling of application upgrades.
- Appropriate indexing for historical measurements and incident queries.

### Configuration

- Store ordinary preferences in application settings.
- Use macOS Keychain if persistent credentials are necessary.
- Keep secrets out of source control.
- Never commit active router session cookies or request tokens.
- Do not require a `.env` file containing router credentials for ordinary use.
- Keep development-only environment configuration separate from production settings.
- Avoid exposing router credentials or authenticated request material to the React frontend unless strictly necessary.

### Build and packaging

- Provide development and production build commands.
- Package the application as a macOS app bundle.
- Verify that the menu-bar integration works in the packaged app, not only in development.
- Verify that the main window opens and communicates with the Rust backend correctly.
- Verify that the menu-bar popover and main window share one monitoring state.
- Document any macOS permissions required.
- Determine code-signing and distribution requirements before public distribution.
- Support local installation for personal use.
- Verify launch, window closure, monitoring continuation, application termination, and restart behaviour.
- Ensure that SQLite history persists across normal application restarts.
- Confirm that the packaged app does not require a running Vite development server.

### Architecture overview

```text
MacBook Pro
    │
    ▼
FibreX Monitor.app
    │
    ├── React + TypeScript Interface
    │     ├── Main Dashboard
    │     ├── Connectivity
    │     ├── Traffic
    │     ├── Fibre & Router
    │     ├── Devices
    │     ├── Incidents
    │     └── Settings
    │
    ├── Tauri 2 Desktop Runtime
    │     ├── Main Window
    │     ├── Menu-Bar Popover
    │     ├── Desktop Lifecycle
    │     └── Native Integration
    │
    ├── Rust Monitoring Backend
    │     ├── Connectivity Probes
    │     ├── Router Client
    │     ├── Traffic Calculations
    │     ├── Incident Engine
    │     └── Notifications
    │
    ├── Local Network Probes
    │
    ├── Local Router Requests
    │
    └── SQLite History
```

### Deployment principle

Keep everything local and straightforward.

No hosted frontend, hosted backend, external database, or cloud account is required for the initial release.

The application must function as an installable macOS utility. The frontend is bundled with the desktop application rather than deployed as a website.

### Development workflow

The project will be built incrementally using OpenCode when available, with ChatGPT used to review implementation decisions, troubleshoot issues, and refine the UI based on the user's feedback.

The coding agent should:

- Inspect the existing repository before editing.
- Confirm the current branch and working-tree state before making changes.
- Use React and TypeScript, not Angular.
- Verify compatibility between the selected Tauri version and its plugins.
- Implement one coherent feature at a time.
- Preserve working code.
- Keep the application buildable throughout development.
- Run builds and relevant tests.
- Test the React frontend and Rust backend independently where practical.
- Verify real Tauri integration instead of relying exclusively on browser previews.
- Report actual results and unresolved issues.
- Avoid claiming completion without verification.
- Update this project plan as implementation progresses.
- Keep all monitoring tasks coordinated by the backend.
- Use mock data only for clearly identified development and UI-preview purposes.
- Never present mock readings as live router or network measurements.
- Document any functionality that cannot yet be verified against the actual router.
- Avoid adding dependencies that do not materially improve the application.

### Suggested implementation sequence

#### Phase 1: Project foundation

- Inspect the local development environment.
- Initialise or inspect the React, TypeScript, Vite, and Tauri 2 project.
- Confirm that the development build works.
- Confirm that a packaged macOS app can be produced.
- Establish the shared design tokens and application layout.
- Configure basic error handling and logging.

#### Phase 2: Desktop interface

- Implement the main window and navigation.
- Build the overview, connectivity, traffic, router, devices, incidents, and settings screens.
- Add loading, empty, stale, unavailable, and error states.
- Implement light, dark, and system appearance.
- Validate the interface at typical MacBook window sizes.

#### Phase 3: Core monitoring backend

- Implement router reachability probes.
- Implement external connectivity probes.
- Add configurable intervals, timeouts, and targets.
- Implement latency and packet-loss measurements.
- Implement the connection-state engine.
- Ensure the frontend receives real monitoring updates.
- Validate the distinction between router failure and external connectivity failure.

#### Phase 4: Local persistence and incidents

- Implement SQLite storage and migrations.
- Persist connectivity measurements.
- Implement incident detection and recovery tracking.
- Build the incidents interface.
- Verify history survives application restarts.
- Add retention and cleanup rules.

#### Phase 5: Router integration

- Verify the authentication mechanism and observed endpoints.
- Implement the WAN statistics client.
- Validate cumulative counters and traffic-rate calculations.
- Investigate device and wireless topology responses.
- Investigate optical and hardware-health readings without assuming unverified endpoints.
- Add explicit source status and freshness information.
- Ensure core monitoring continues when router integration fails.

#### Phase 6: Menu-bar experience

- Implement the macOS menu-bar icon and popover.
- Connect the popover to the shared monitoring state.
- Add compact status and traffic summaries.
- Implement pause/resume and open-dashboard actions.
- Verify that closing the main window does not stop monitoring.
- Verify that quitting the application stops monitoring cleanly.

#### Phase 7: Notifications and recovery

- Add connectivity loss and recovery notifications.
- Add sustained latency and packet-loss alerts.
- Handle network changes and Mac sleep/wake.
- Implement safe retry and cancellation behaviour.
- Validate notification preferences and duplicate-alert suppression.

#### Phase 8: Testing and packaging

- Test core monitoring without router authentication.
- Test router integration against actual responses.
- Test missing and stale data.
- Test incident creation and recovery.
- Test application restart and history persistence.
- Test packaged menu-bar behaviour.
- Test Apple Silicon builds.
- Document installation, known limitations, and troubleshooting.

These phases provide an implementation order, not a requirement to build every supporting feature before validating the core monitoring workflow.

---

## Explicit MVP Exclusions

The following are intentionally outside the first version:

- Router configuration changes.
- Remote router administration.
- Automatic router rebooting or repair.
- Wi-Fi credential management.
- Firewall and WAN configuration.
- Packet capture and deep packet inspection.
- Network-security threat detection.
- Hosted telemetry and cloud synchronisation.
- User accounts and multi-user workspaces.
- Remote access through an internet-facing server.
- Guaranteed per-device bandwidth tracking without a verified source.
- Automatic diagnosis of the exact cause of every outage.
- Continuous automatic speed testing.
- Paid third-party monitoring APIs.
- Complex alert-routing integrations.
- Commercial billing and subscriptions.
- A separate iOS or Android application.
- Artificially populated charts when live data is unavailable.
- Optical metrics before their retrieval source has been verified.
- A mandatory browser-based interface.
- A hosted dashboard or backend required for ordinary use.
- Multiple independent monitoring loops owned by different windows.
- Automatic router changes or operations that modify network configuration.

The MVP succeeds if the user can monitor the connection from the macOS menu bar, inspect real latency and packet loss, review incidents, and see verified router traffic statistics when available, all through a standalone desktop application without relying on a cloud service.
