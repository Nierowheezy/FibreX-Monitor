# Stored history with a real range, WAN traffic, pause and resume, and the panel from the design

**Type:** Feature

**Status:** verified


> One work item delivered several build-plan items: 6 (the history range),
> 8b (the panel), 9 (pause and resume), 11 (WAN traffic) and 18 (the clock
> format). Each is checked off in the build plan from this single archive.

**Build plan:** 6 (Latency and probe-failure history), 9 (Monitoring lifecycle
controls), 11 (WAN traffic monitoring), 8b (Panel visual treatment), 18
(Appearance)

## What this is

The interface work that was asked for, plus the two producers the design's
panel needs so its controls are not dead.

Four things, in the order they depend on each other:

1. **The chart labels are readable in dark mode.** They were black.
2. **The latency history has a real range control.** The design has a stepper
   that moves between ranges, and the app had a chart that could only ever show
   the current launch. The stored history is already on disk, so the stepper
   reads real samples for the range it names.
3. **WAN traffic.** The byte counters are behind a verified endpoint, so the
   design's Download, Upload and Session total rows have a real producer.
4. **Pause and resume, and the panel rebuilt to the design.** The design's panel
   has a Pause control and a sparkline. A control that does nothing is not
   allowed, so pause and resume are built for real and the panel is rebuilt
   around what the app actually measures.

Plus the fullscreen button, which sets a flag but does not produce the
fullscreen the user asked for.

## The fullscreen problem, and the fix

The button set a flag and the window grew to the screen, but the macOS menu bar
stayed exactly where it was, and so did the Dock. Three separate causes, found
by measurement rather than by argument, and each one needed its own fix.

**The app was an accessory, and an accessory app cannot own a fullscreen
space.** It ran as `ActivationPolicy::Accessory` so it would have no Dock icon
and no application menu. macOS does not give such an app a fullscreen space,
so the native fullscreen never engaged. The app now becomes a regular app
whenever the dashboard is on screen, and a menu-bar app when it is not. A
regular app can have a fullscreen space, and in it the menu bar and the Dock
are hidden until the pointer reaches the top of the screen.

**The library reports a fullscreen that never happened.** It records the
request as fullscreen the moment the request is made, so `is_fullscreen()`
answers true whether or not macOS honoured it. The evidence has to be the
window's own frame: the native path is treated as engaged only when the window
actually grew to cover the screen.

**The green button was zooming.** macOS's default for it is to zoom the
window, which fills the screen but leaves the menu bar where it was, so a
button that looks like a fullscreen button was not one. It is now pointed at
the window's own `toggleFullScreen:` action, which the window already
implements, so nothing had to be reimplemented and no new class was needed.

The app's own fullscreen command still exists, for the usual `Ctrl+Cmd+F`
shortcut, and it tries the native path first and falls back to the pre-Lion
fullscreen, which sets the application's presentation options to hide the menu
bar and the Dock.

## The range control

The stored series is already there: `get_stored_series` reads any range from
SQLite. What was missing was the control that chooses the range, and the
honest label that says when the readings in it were taken.

The ranges are five minutes, fifteen minutes, one hour, six hours and
twenty-four hours. The control is the design's stepper: two arrows and a
label. The arrows stop at the ends rather than wrapping, because a pair of
arrows that silently jumps to the other end of the list is disorienting.

**The label is the clock times, not a duration.** "5 min" says how long the
window is; it does not say *when* the readings were taken, and a latency shown
in milliseconds invites the reader to treat it as a number with no time
attached. So the label names the span, and the range's own length stays in the
card's subtitle where it belongs.

A range wider than the stored data is still selectable, because the data grows
while the app runs, and the card says how many stored probes the range holds
rather than implying the range is full.

**A long range is thinned before it crosses the IPC boundary.** At the shipped
two second interval, twenty-four hours is forty-three thousand samples, which
is more than a chart needs and far more than should be sent on every poll. The
series is cut into equal slices of time, and the rule for thinning is the one
that matters: a slice that contains a failed probe becomes a gap. Thinning a
failure away would draw a smooth line over an outage. A slice with no failures
keeps the last real sample in it, which is a stored measurement rather than an
average of measurements, so nothing invented reaches the chart.

## WAN traffic

The counters come from `/html/bbsp/common/get_wan_list_pppwanstat.asp`, which
returns a `WaninfoStats(domain, BytesSent, BytesReceived, PacketsSent,
PacketsReceived, ...)` object. The IP variant of the same call returns an
empty array on this firmware, so the PPP counters are the usable source.

A rate needs two readings. The rules that matter:

- A counter that went **down** means the router reset it. The rate is
  unavailable, not a huge spike.
- Two readings with no time between them produce no rate.
- Two readings too far apart describe a period the user is not looking at, so
  the rate is marked stale rather than presented as current.
- The cumulative counters are real totals since the session began, and are
  shown as totals. They are not rates and are never labelled as one.

## Pause and resume

The probe loop already has a stop channel. Pausing is a different thing: the
loop must stay alive so it can resume, and the readings it took before the
pause must stay stored.

The paused flag lives in the monitoring state, and the loop skips its probe
while the flag is set. The interface reports `paused` separately from
`monitoringActive`, because a paused app is not a stopped app and the two
should not be collapsed into one.

## The panel

Rebuilt to `prototypes-new/panel.html`, which is the approved design:

- The app mark and the name in the header, with the live dot and a pause
  control.
- A `LATENCY` label over the figure, the health pill, and a foot line with
  when it was probed and which target it was against.
- A sparkline of the same latency series, with gaps left as gaps.
- The rows: probe failures, download, upload, router, devices online.
- A primary `Open dashboard` action and a `Pause` action.
- A foot line: the interval, and that everything stays local.

Every row has a real producer now. Devices online does not: the router's
device list is not readable on this firmware, so that row says so rather than
showing a count of zero.

The panel is 340 points wide, and the charts inside it are taken out of the
flow. An SVG with a view box has an intrinsic width, and that width became the
minimum width of the panel's column, which pushed the rows' values past the
edge of the window.

## The clock

A setting for the clock format, because a latency in milliseconds invites the
reader to treat it as a number with no time attached, and the times beside it
were always on the twenty-four-hour clock.

The format lives in a module-level value rather than in React state, because
the formatters that use it are plain functions called from all over the app,
including from the panel, which is a separate window with its own React root. A
value a formatter can read is a value that cannot be forgotten by a component
that forgot to pass it down.

It is loaded before the first render, in both windows, for the same reason the
theme is: the panel must not show one clock format and the dashboard another.

## Done when

- `npm run verify` passes.
- `cargo test` and `cargo clippy --all-targets` are clean.
- The chart's axis labels are legible in dark mode. They were black, because
  the labels asked for a variable that does not exist, so the fill fell back to
  the default black.
- The range stepper changes the range the chart shows, and the range it names
  is the range in force.
- Download and upload show a rate that came from two real counter readings, or
  say they are unavailable. Neither ever shows zero unless the counters really
  did not move.
- Pause stops the probes and Resume starts them again, and the readings taken
  before the pause are still in the history.
- The panel matches the design and has no control that does nothing.
- Fullscreen covers the screen with the menu bar and the Dock out of the way.

## What was found on the way

Two things that were not obvious and are now settled by measurement rather
than by argument:

- **The chart's black labels were a missing variable, not a colour choice.**
  The axis labels asked for `var(--ink-faint)`, and the token is
  `--color-ink-faint`. The fill resolved to nothing, and an SVG text with no
  fill is black. It looked like a dark mode bug and was a typo.
- **The native macOS fullscreen does not engage for this app, and the library
  does not notice.** See the section above. Both were measured with a
  throwaway program that drove a real window, and the program was deleted
  afterwards.

The router also went away during the work, because the machine changed
network. That turned out to be a useful accident: the app kept monitoring the
internet, kept recording history, and reported the router as unreachable with
the reason, which is exactly the behaviour the plan asks for when the router
is absent.
