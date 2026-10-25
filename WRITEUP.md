# FibreX Monitor: what it is, and how it got built

*A local-first macOS menu-bar app for an MTN FibreX line, plus the 100-second
video that explains it.*

---

## The short version

FibreX Monitor sits in the macOS menu bar and watches one internet connection.
Every two seconds it opens a real TCP connection to `1.1.1.1:443`, times it,
and records whether it worked. When a probe fails, it opens an incident: when
the line went down, when it came back, how long it was gone, and whether the
router was still answering at the time. It also runs a real speed test against
`speed.cloudflare.com`, and, if you give it your router's admin login, reads
WAN traffic, link state, uptime, optical receive power and firmware details out
of a Huawei OptiXstar — read-only, never writing a setting.

Everything lives in a local SQLite database. No account, no cloud, no
telemetry, MIT licensed, macOS only.

The video:
[v0.1.1 · fibrex-monitor-launch.mp4](https://github.com/Nierowheezy/FibreX-Monitor/releases/download/v0.1.1/fibrex-monitor-launch.mp4).

---

## The problem I actually had

Not "I want to see my ping". The problem was narrower and more annoying than
that.

The line drops for four minutes while I am on a call. By the time I notice, it
is back. I have no screenshot, no timestamp, no duration — just a memory that
says "it keeps dropping". So I ring support, and the conversation goes nowhere,
because from their side the line looks fine *right now*, and from my side I
have nothing but a feeling. Every ISP support script starts with "is it
connected?", and the honest answer at that moment is always yes.

The useful thing is not a live number. It is the receipt: **when** it broke,
**how long** for, and **which half** of the connection broke. "4m 12s, router
stayed up" is a completely different fault from "router also unreachable". One
is upstream of my flat; the other is in my flat. The app can tell the
difference because it measures both, separately, and it never guesses a cause
it did not observe.

So the design goal was never "a prettier ping". It was: if the line fails while
I am not looking, the app should still be able to describe what happened.

---

## What it measures

### The probe

Every two seconds the Rust backend opens a TCP connection to `1.1.1.1:443` and
times how long the connect took. There is no ICMP ping involved — TCP connect is
what a browser actually does, and it works without root privileges. Each probe
produces one of three outcomes: connected, failed, or a latency reading that is
kept with the time it was taken.

From those probes the app derives:

- **Round-trip latency** — the last successful connect time, with how long ago
  it was measured.
- **Jitter** — the variation between consecutive readings, not a guess.
- **Probe failures** — a count and a rate over the recent window, so "3 of 25
  failed" rather than a vague red dot.
- **Freshness** — the app tells you when the last reading is old. A stale
  number is never presented as current.

### The incident record

When probes start failing, the app marks the start. When they recover, it
closes the incident and stores the duration. Each one records whether the
router was reachable during the outage, which is the part that makes the record
worth showing to someone else.

Incidents are listed newest first in a fixed-height scroll view, with a summary
card beside it — count, total downtime, longest outage. The card hugs its own
content; it does not stretch to fill a column and leave a dead rectangle on
screen.

### The speed test

A real measurement, not an estimate. Three named phases — **Ping**,
**Download**, **Upload** — each labelled on the gauge while it runs, against
`speed.cloudflare.com`, whose name is on screen. Download uses six parallel
streams with a warm-up discard, and upload streams rather than sending one big
block. Each transfer phase runs about ten seconds, so the whole test is roughly
half a minute.

While a phase is running, the metric cards say **No reading**. That is not a
bug and it is not laziness: the app has not measured ping or jitter yet, and it
will not print a placeholder number to fill the space. The live rate appears on
the gauge, because that is the number that actually exists at that moment.

### The router read

The router integration is deliberately boring. The app signs in to the router's
web UI and fetches pages. It does not POST a configuration change, it does not
restart anything, it does not "optimise" the Wi-Fi. Read-only by construction.

What it surfaces: WAN traffic, link state, uptime, firmware details, and the
optical receive power. The firmware on this model reports receive power only —
transmit power, temperature, voltage and bias current are not exposed at all, so
there is no field for them and nothing pretends otherwise. A "Devices" page was
built and then removed, because the firmware answers `"NONE"` for the device
list endpoint and a screen that cannot be filled is worse than no screen.

Configuration lives in the app under **Settings → Router connection**: address,
port, user name, password. The password goes to the macOS Keychain, never to a
file, and the field never echoes it back — when one is stored, the field is
empty with "Stored — type a new one to replace it". Saving without typing a new
password keeps the old one, so changing the address does not mean retyping the
secret every time. Removal is a two-click confirm that erases both the settings
and the Keychain entry.

The reader runs on a slow cadence, about every half a minute, to be kind to the
router's small web server. Saving is arming, not an instant read; the badge says
**Reading** while a read is under way.

### The menu bar

The menu-bar panel shows live state without opening the dashboard: current
health, today's traffic, the router's state. Clicking through opens the full
window. The icon is the app's own three-ember-chevron mark drawn as a macOS
template image, so it inverts correctly with the menu bar's appearance.

v0.1.1 fixed a genuinely subtle bug here: the panel was created at launch while
the app was still a regular app, and a window born at that moment ignores
macOS's "join all Spaces" behaviour for the rest of its life. So clicking the
icon while another app was frontmost looked like nothing happened. The panel is
now created only after the app has become a menu-bar app, which is when macOS
honours the behaviour.

---

## The screens

| Screen | What is on it |
|---|---|
| **Overview** | Live connection health, the last incident, probe failures with a count and a rate, live latency, today's traffic, the router snapshot. |
| **Fibre and Router** | The line and the router on it: link status, uptime, WAN traffic, optical receive power, firmware details. |
| **Speed test** | The gauge, the three phases, the four result cards, and a footer naming the host and the time it was measured. |
| **Incidents** | The exact record — start, recovery, duration, router reachability — newest first. |
| **Settings** | Pause monitoring, light/dark appearance, the router connection. |

Dark is the default theme. Light is a full inversion with its own colour
decisions, not a recolour of the dark palette.

---

## The rules I refused to break

These started as taste and became the app's actual character:

1. **A `0` never means "unavailable".** A measured zero and a missing reading
   are different facts. Missing readings say "No reading" in words.
2. **Status colour always comes with words.** A green dot alone is not a
   status; "Connected" next to it is.
3. **Every figure has a provenance.** The speed test footer names the host and
   the timestamp. Latency says how long ago it was measured.
4. **No cause is claimed without evidence.** Router unreachable and internet
   unreachable are tracked separately, and the incident says which one broke.
5. **No mock data in product code.** The browser harness has stubs; the app
   does not. If the app cannot measure something, the screen says so.
6. **No secrets in files.** The router password lives in the Keychain. The
   settings file holds the address, port and user name only.

Rule 5 is why the launch video took longer than it should have — more on that
below.

---

## What it deliberately does not do

| Not there | Why |
|---|---|
| A "Devices" page | The firmware returns `"NONE"` for the device list. A screen built on nothing was removed. |
| Optical transmit figures | The firmware only reports receive power. No field exists for the rest. |
| A WAN number that matches the speed test | They answer different questions. The router card reports what **all devices** are moving on the WAN right now; the speed test is **one synthetic burst** from one machine. Both are Mbps, both are correct, neither is the other. |
| Automatic updates | Built and released by hand. |
| A notarised installer | Needs an Apple Developer identity. The bundle is unsigned; macOS warns once, and the README says so plainly. |
| Blaming the ISP | The app does not know where the failure is unless a probe proves it. |

---

## How it is built

**Stack:** Tauri 2 desktop shell, React + TypeScript + Vite frontend, Rust
backend, local SQLite history, macOS Keychain for the router password.

The Rust side owns the measurement loop, the probe primitives, the incident
state machine, the speed test engine, the router client and the settings
storage. The frontend asks for state over Tauri commands and subscribes to
progress events — the speed test publishes its phase and its live rate as
events, which is why the gauge animates while the cards stay honest.

**Tests:** 104 Vitest unit tests over the pure logic — formatting, parsing,
latency and jitter rules, incident derivation — plus `cargo test` for the engine
and settings logic, `cargo clippy` clean. `npm run verify` is typecheck plus
unit tests, and it is the command to run before pushing; there is no CI
workflow, because the checks are cheap enough to run locally.

**The browser harness:** `npm run harness` serves the built frontend on
`localhost:4173` with a stub that answers every Tauri command with realistic
figures and mirrors the Tauri v2 event API. It exists so the interface can be
inspected, and captured, without the desktop app running. It is a development
tool; it is not imported by the app and never reaches the bundle.

**The workflow:** the project is built with the AI BuildFlow — one feature
spec at a time, implemented in small steps, verified after each, recorded in
`buildflow/history/`. Features like the probe primitives, the monitoring loop,
packet loss and freshness, latency history with a real range, and the router
integration each have a spec and an archived record. It is process scaffolding
around normal engineering, not a different kind of engineering.

---

## Making the 100-second video

### Why a video at all

The app's whole argument is visual and temporal: a graph with a gap in it, a
duration, a router state during the outage. A screenshot of a dashboard does not
carry that. A 100-second narrated walkthrough can show every screen, explain
why each one exists, and answer the question the app keeps inviting — is this
useful, or is it just for people who ping for sport?

The honest answer in the video is "both", and the useful half gets the
explanation.

### Tools

The video is **Remotion** — React components rendered frame by frame, 1920×1080
at 30 fps, 3028 frames, 100.9 seconds. The creative direction followed the
`brag` skill: pick an angle, storyboard it, script it, cast the voice, cut to
music, then validate and deliver.

Two deliberate departures, both written down when the brief was drawn up:

- `brag` targets 15–25 second social cuts. Showing five screens plus the
  benefit plus the honest verdict does not fit in 25 seconds without becoming a
  feature list, so this is 100.9 s. That is inside X's limit and fine for
  LinkedIn; a 20-second teaser can be cut later from the opening, the verdict
  and the outro without reshooting anything.
- `brag` normally drives Hyperframes. This one is built in Remotion directly,
  because the composition needed frame-accurate camera moves over real captures
  and a rendered, measured audio mix.

### Structure

Ten scenes, each one job:

1. The problem — "your internet drops for four minutes", over a latency line
   with a gap in it.
2. The app — what it is, where it lives.
3. Overview — live health, failures, traffic.
4. Speed test — a finished measurement, then a running upload.
5. Incidents — the record, and why the record is the point.
6. Fibre and Router — what the router read gives you, and what it does not.
7. Appearance — the light/dark pair.
8. The honest answer — useful, or ping-nerd toy.
9. How it is built — local, read-only, MIT.
10. Outro — where to get it.

### Sound

Narration is Kokoro TTS (`af_heart`, speed 1.0), one WAV per scene, placed 0.3 s
after each cut, resampled to 48 kHz. The music bed is
`happy-beats-business-moves-vol-12` by Sascha Ende (CC BY 4.0, credited in the
README, the video description and the share copy). It runs at 109.96 BPM, and
every scene boundary is quantised to that beat grid — 0.545653 s — so cuts land
on the music instead of near it. The bed sits at 0.24 and ducks to 0.09 under
narration over 10 frames. Seven sound effects sit at or below 0.30 gain: window
entrance, the run click, the panel drop, the theme switch, a bell on "Both.",
and two soft impacts.

### The font crash

The first renders died repeatedly with `A delayRender() "Loading font …" was
called but not cleared`. The composition was loading variable fonts over the
local static server, and a recycled render tab could finish before the fetch
did. The fix was to stop fetching at render time: the fonts are now inlined as
base64 data URIs in `src/fontData.ts`, so there is no network step to race.
Render concurrency is 2 for the same reason — stability over speed.

### The scene that was lying

The first cut of the speed-test scene was wrong in a way that is easy to miss
if you only look at thumbnails. The "completed" capture was actually the
never-run state — every card said **No reading** — and the "running" capture
was a frame before any measurement existed. So the scene showed four seconds of
an app that had measured nothing, in a video whose entire argument is that the
app only shows what it measured. The visual was honest; the *story* was
misleading.

The fix was to capture again, deliberately:

- a **finished** test — 38 ms ping, 3 ms jitter, 48.8 Mbps down, 17.2 Mbps up,
  footer naming `speed.cloudflare.com` and the time;
- a **mid-run upload** frame, where the gauge reads 17.2 Mbps and the cards
  correctly say "No reading".

The camera was then reframed: the scene opens on the finished result so all
four numbers are legible, and after the click it moves onto the gauge, because
that is the only place a live number legitimately exists during a run. The
metric cards being empty is not hidden — it is just not the subject for those
two seconds.

Captures were taken through the browser harness, window-only, with the stub
answering in place of the router. Nothing in the video exposes a desktop, a
credential, or a real router address.

### Loudness and QA

The raw render measured −25.3 dBFS RMS with peaks at −4.7 dB. It went through
`loudnorm=I=-16:TP=-1.5:LRA=11`, landing at −16.1 LUFS integrated and −1.5 dBTP
true peak — the usual target for web video. The un-normalised file is kept
locally as `brag-raw.mp4`; the delivered master is `brag.mp4`.

QA was not "watch it once and hope":

- one frame per second through a luma scan, to catch unintended black or frozen
  stretches (none except the intentional final fade);
- OCR on encoded frames at known timestamps, to confirm each scene's text was
  actually present after compression;
- a specific check on the speed-test transition at 43.4 s → 45.0 s, to prove
  the results-to-running cut happens where the click does.

The poster is frame 0, composed as a poster: headline, label and the broken
latency line all fully on screen. That way the thumbnail everywhere — GitHub,
X, LinkedIn — is the frame the video actually starts on, with no re-baking step
that could drift.

### Delivery

- Master: `brag.mp4`, 100.99 s, 23 MB, H.264 CRF 16, AAC 320 kbps.
- Poster: `brag.jpg`, 1920×1080, frame 0.
- Published as a GitHub release asset on v0.1.1 and linked from the README.
- Caption variants: an X post at 274/280 weighted characters, a long-form
  LinkedIn version, and poster alt text.

The Remotion source, captures, narration and music stay local. They are not
committed: the working files are large and the music is CC BY 4.0, not MIT. The
`brag` skill itself is installed from `skills-lock.json` for the same reason.

---

## Is it useful, or is it a toy for people who ping for sport?

Both, and the split is about 80/20.

The 80% is the record. A line that drops for four minutes once a day is
invisible to a live dashboard and impossible to prove from memory. The app makes
it a row: start, end, duration, router state. That row is the difference between
"it keeps dropping" and "four minutes twelve, router stayed up" — and the second
sentence gets a different conversation from support than the first one does.

The 20% is that a well-drawn latency graph is genuinely pleasant to look at, and
yes, I have left it open at 2 a.m. for no operational reason at all. That part
is not a feature. It is a hobby, honestly labelled.

---

## Try it

1. Download the `.zip` from the
   [Releases page](https://github.com/Nierowheezy/FibreX-Monitor/releases).
2. Drag **FibreX Monitor.app** into `/Applications`.
3. Right-click → **Open** the first time, because the bundle is unsigned.
   macOS warns once; after that it opens normally.

It works with zero configuration — probes and incidents start immediately.
Connect the router under **Settings → Router connection** if you want the WAN,
link and optical figures.

Source: **https://github.com/Nierowheezy/FibreX-Monitor**
Video: **https://github.com/Nierowheezy/FibreX-Monitor/releases/download/v0.1.1/fibrex-monitor-launch.mp4**
License: MIT. Music: Sascha Ende, ende.app (CC BY 4.0).
