# Current Feature

# A speed test, the endpoints that do have data, and the screens that were only apologising

**Type:** Feature

**Status:** implemented, awaiting review

**Build plan:** 13 (Connected-device overview, removed), 14 (Router health
monitoring, extended), 47 (Internet speed test)

## What this is

Three things, all of them about the app showing what it can actually measure.

1. **A speed test**, on its own screen with its own rail icon, so the line to
   the internet can be measured from inside the app without going to a website
   first.
2. **The router endpoints that do have data**, added to the Fibre & Router
   screen: the Wi-Fi radios, the access type of the WAN, and the IPv6 state.
3. **The screens that could only ever apologise, taken out.** The Devices
   screen and the panel's device row are gone, because the router's device list
   is not readable on this firmware.

Plus two smaller things that were asked for: the chart's end dot no longer
grows, and a change of screen animates rather than jumping.

## The speed test

Three measurements against the same host: the round trip, the download rate and
the upload rate, each from a real transfer.

The host is Cloudflare's speed test endpoint. It needs no account and no key,
and it is the same network the app's own probe target sits on. A measurement
against a host the user cannot reach would be a measurement of nothing, so a
failure is reported as a failure and never as a number.

**The transfers are measured the way the speed tests people compare this one
against measure them.** Six connections at once, for a ten-second window, with
the first second thrown away and the figure taken from what moved after it.
The first version of this measured one request and divided its total bytes by
its total time, and on the line it was built for that read 30 Mbps of download
where speedtest.net read 49, and 5 Mbps of upload where a website test read 17:
one connection cannot fill the line with the round trip this path has, and
counting a connection's opening ramp into the average reads a number the line
never sustains. The upload pushes a continuous body per connection rather than
a series of one-shot bodies, which is what a speed test's own upload does, and
without it every request carried its own opening ramp into the figure.

**The figures are the line's, not the test host's distance.** The round trip is
to Cloudflare's edge; a website speed test usually runs against a server in
town, so the two pings are measurements of different paths and can differ by an
order of magnitude while both being true. The screen says which host the
figures came from. The download runs first so the latency that follows is the
round trip over pooled connections and not the cost of opening one, and the
latency shown is the median of six samples.

**The three figures are independent.** A line that uploads at nothing but
downloads at 200 Mbps is a real and common situation, and a result that
dropped the whole test because one of the three failed would be throwing away
the two that worked.

How much data a test moves depends on the line: the transfer window is ten
seconds each way, so a 50 Mbps line moves about sixty megabytes down and up to
twenty megabytes up. The test is not a probe of the router. It measures the
line to the internet, which is what a speed test is for, and it is kept apart
from the monitoring loop so a test never becomes an incident.

**The dial.** The screen shows a speedometer: an arc from 0 to 1000 Mbps, a
needle, and the figure in the middle of the dial the way a speed test has
always put it. The scale is logarithmic between the marks, because a line can
be a thousand times faster or slower than the one next to it and a linear scale
would press every ordinary speed into the first few degrees of the arc. The
tick labels name each mark, so the dial is never a mystery.

While the test runs the needle scans back and forth across the scale, which is
the motion a speed test has always made while it measured, so a user is never
left staring at the word running. The figure in the middle is the real rate the
transfer has reached, published from the backend as the transfer progresses,
and the filled part of the arc grows with it. When the test ends the needle
settles on the measured value. The needle's motion is decoration; the number
and the fill are not.

## The router endpoints that do have data

Three more pages are read, and each was verified against the live device.

| Page | What it gives |
|---|---|
| `get_wan_list_wlaninfo.asp` | The Wi-Fi radios. Two on this router, `ath0` and `ath4`, both switched on. |
| `get_wan_list_wanaccesstype.asp` | The access type of the WAN, which this router reports as `GPON`. |
| `wan_list_cache_wanipv6.asp` | The router's IPv6 state. It reports the WAN's IPv6 connection as `Invalid`, and delegates no prefixes. |
| `getWanDynamicData.asp` | Whether the router says it supports IPv6, which it does. |

**The IPv6 state is the router's own word and is passed through.** The
observed connection's IPv6 status is `Invalid`, and the app shows that rather
than interpreting it. The router's vocabulary for its own state is the one
that matters.

**Two of these pages answer a plain request with a stub**, the way the
router's own JavaScript fetches them: with the token in the query string
rather than in the body. Reading them the other way returns a 602-byte page
that looks like a page and carries nothing, which is the same trap that cost
a full debugging session over the session cookie.

## The screens that were removed

The Devices screen and the panel's device row are gone.

The router's device list is behind `html/bbsp/userdevinfo/getuserdevinfo.asp`,
which answers `"NONE"`. The router builds the table lazily from an ARP scan,
and the file does not exist until that scan has run, so the read is not
available on demand. A screen that could only ever have said so, and a row
that could only ever have said "no reading", were both permanent apologies
for data that is not there. The Wi-Fi radios the router does report are on the
Fibre & Router screen instead.

## The end dot

It no longer grows.

The dot marks where the current reading is, and it breathes by fading. It used
to scale to nearly twice its size and come back, which read as a second mark
appearing rather than as the same one still being there, and it ended up
sitting outside the axis it belongs to. The design's dot fades and does not
scale, and that is what it does now.

## The screen transition

A change of screen fades and rises into place rather than jumping.

The screen is keyed on its id, so the animation plays again for each new
screen rather than leaving the new one half faded. It is short on purpose:
long enough to read as movement, short enough that a person clicking through
the rail is never waiting for it. And it is switched off entirely when the
system asks for reduced motion, because it is decoration and nothing about
what is shown depends on it.

## Done when

- `npm run verify` passes.
- `cargo test` and `cargo clippy --all-targets` are clean.
- The speed test measures a real line: verified against the live endpoint,
  and comparable with speedtest.net's own figures for the same line
  (download 44.5 and 48.2 Mbps across runs against 48.91; upload 17.8 Mbps
  against 17.09) when the line was in the same state.
- The transfer phases open several connections and measure a steady window,
  not a single request divided by its total time.
- The router's radios, access type and IPv6 state are read live.
- There is no Devices screen and no device row.
- The end dot does not grow.
- A change of screen animates.
- The dial's needle sweeps while the test runs and settles on the measured
  value, verified in the browser harness.
