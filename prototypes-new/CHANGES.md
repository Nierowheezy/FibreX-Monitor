# Changes to the dark bento prototype

The pristine design reference is committed at `f310fb2`, on top of `1f55c25`. Diff
any change here against that commit to see exactly what moved:

```
git diff f310fb2 -- prototypes-new/
```

Every change below is a correction or an agreed scope cut, not a redesign. The
hero chart is not touched. `theme.css` is not touched yet.

## Kept, do not remove

- `prototypes-new/` stays in the repo as the design reference. `/complete` must not
  delete it.
- The old light `prototypes/` folder also stays.
- The hero latency chart stays exactly as drawn. `drawHero` and the `drawChart`
  rig are byte-identical to `f310fb2`.
- The wordmark SVG in the top bar is adopted as the app mark.

## Top bar

1. Removed the search field (`div.search` and `input#search`). There is no search
   backend and no indexed data, so it was chrome promising a feature.
2. Removed the notification bell and its `dot-badge`. The badge was a hardcoded
   unread count that no code ever produced.
3. Removed the account avatar and its popover. The app is local-first with no
   account, no sign-in and no cloud.
4. Replaced the three-segment appearance row with one icon button whose glyph
   changes with the mode. Same pattern `src/app/AppearanceMenu.tsx` already ships.
   The glyph shows the mode that was chosen, so System never silently reads as
   Dark.
5. Rewrote `applyMode` to swap the trigger glyph and its `aria-label`, keep the
   header menu and the Settings segmented row in step, and close the menu on
   select, outside click and Escape.
6. Removed the now-dead account popover script and the `[data-goto]` handler.

## Hero

7. Removed the hero dock, five icon buttons. The file's own comment called them
   "the reference's camera controls, remapped to monitoring". They duplicated
   panel and tray actions and pointed at unbuilt features.

## Incident card

8. Removed the music-player transport (previous, play, pin, next) and the scrub
   bar, left over from the reference design's music card.
9. Replaced it with a plain two-line time row: `Started 23:41:08` and
   `Recovered 23:45:20`.

## Factual corrections

10. `1.1.1.1 · ICMP` changed to `1.1.1.1 · TCP 443`. The probe is a TCP connect.
    This app never sends ICMP and never will.
11. `192.168.1.1 · ICMP` changed to `192.168.1.1 · TCP 80`.
12. Probe targets sub, `timeout 2s` changed to `timeout 1.5s`, matching the real
    `DEFAULT_TIMEOUT_MS = 1500`.
13. Settings probe timeout input, `2000 ms` changed to `1500 ms`.
14. Probe targets list cut from three invented targets to the one real target:
    Internet, `1.1.1.1`, `tcp 443`, `2s`.

## Scope cuts agreed with the user

15. Removed the **Connectivity** screen. Four of its seven cards duplicated the
    hero chart, the Incidents timeline, Settings monitoring, and Overview's own
    Router card. Its only card with no other home was Uptime today.
16. Removed the **Traffic** screen. None of its four cards can be live before
    router integration, and Overview already carried a WAN traffic card.
17. Folded WAN traffic into **Fibre & Router** as a router-gated section:
    Download, Upload, Session total and Throughput history, each with an honest
    `No reading` empty state.
18. Added **Uptime today** to Overview, the one genuinely unique card from
    Connectivity.
19. No Probes card was added. Overview already carries a Router card at
    `192.168.1.1` and the hero already owns internet latency, so a second copy
    would be the duplication the cut was meant to remove.
20. Removed the connectivity and traffic rail buttons. Five screens remain:
    Overview, Fibre & Router, Devices, Incidents, Settings.
21. Moved the line "Probed separately from the internet, so a local fault is
    never blamed on MTN" onto the Fibre & Router screen subtitle, which is where
    the router-versus-internet distinction now lives.

## Script fixes, a real regression

22. The removal of the search field left `search.addEventListener(...)` behind.
    That threw a TypeError which aborted the whole script block before
    `drawHero()` ran, which is why the chart was blank. The reference is gone and
    the filter now runs on incident kind alone.
23. Removed `drawHistory()`, its `[data-range]` click handler and its init call.
    Those elements belonged to the deleted Connectivity screen, and the missing
    `[data-range]` node was a second throw on the same path.

## Round two: the panel and the failure figure

24. Probe failures, not packet loss. The app labels this figure "Probe failures"
    in `src/features/overview/MetricGrid.tsx` and `src/panel/PanelScreen.tsx`,
    and `formatProbeFailures` renders it as `${failed} of ${total}`. Both
    prototypes said "Packet loss". The dashboard gauge card is now "Probe
    failures" over "Last 5 minutes", showing `3` `of 25` with the caption
    `failed`. The percent sign is gone, because a lone percentage is exactly the
    packet-loss claim the formatter refuses to make.
25. The invented "Warn at 2%" threshold chip is gone. Feature 4, the thresholds
    that would define it, is not built, so the number came from nowhere. The
    surviving chip reads "25 probes sent", which is a fact.
26. Settings, incident list: "Packet loss above 5% / confirmed" is now "Probe
    failures sustained / above threshold". `confirmed` was an unearned
    assertion, and the design's own rule is that a suspected outage is never
    labelled a confirmed fault.
27. Settings, incident categories: "latency or loss above threshold" is now
    "latency or probe failures above threshold".
28. The panel's target line no longer uses an em dash, and no longer hides the
    target when there are no readings. 1.1.1.1 is compiled-in configuration, not
    a reading, so it is shown either way.
29. "Unsupported readings" showed em dashes for values the router does not
    expose. They now read "No reading", the same words the app uses.
30. Removed the stranded empty state "No device matches that search." and a
    stale JS comment block, both left behind when the search field went.

## A bug I introduced, then fixed

31. The Uptime card was inserted inside the empty-state note div instead of into
    the bento grid. The insertion script targeted the last `</div>` before the
    section close, which was the note's. Because the nesting stayed balanced,
    the div-balance check passed while the placement was wrong. It is now a
    sibling of the other cards.

## CSS cleanup

32. Removed 38 orphaned rules from `theme.css`: `.search` (including its
    media-query copy), the narrow `.segmented`, `.hero-dock`, `.dock-btn`,
    `.dock-btn--power`, `.dot-badge`, `.transport`, `.scrub`, `.scrub-fill`,
    `.play`, `.account-avatar`, `.account-wrap`, `.popover-head`,
    `.popover-title`, `.popover-note`, `.pill--accent` and `.gauge-scale`, along
    with their light-theme variants. `theme.css` went from 1546 lines to 1289.
33. Kept `.popover`, `.popover-item`, `.segmented-wide` and `.square-btn`. All
    four are still in use.

## Observed, deliberately not changed

34. `.chart-frame` is used in the markup in two places but has never had a CSS
    rule, including in the pristine reference. The charts still draw from their
    own viewBox. Adding a rule would be a design change, so it is left as found.

## Verified

- `node --check` passes on both extracted scripts.
- Both scripts run to completion against a stub DOM built from the real element
  ids. The stub reproduces the `search` TypeError when that fix is reverted, so
  the pass is meaningful rather than vacuous.
- Every `getElementById` target and every `querySelector` attribute hook in both
  files resolves.
- All of `div`, `section`, `article`, `nav`, `header`, `button`, `svg`, `span`,
  `p`, `h1` and `h2` balance in both files.
- No em dash, en dash or ellipsis character remains in any of the three files.
- The `drawHero` block and the `drawChart` rig are byte-identical to `f310fb2`.
- `theme.css` braces balance at 155 and 155 after the cleanup.

## Not done yet

- `data-search` attributes on list rows are now inert, since nothing reads them.
  They are harmless and left in place rather than churned.
- The wordmark SVG is adopted as the app mark but has not been ported into
  `src/` yet.
- The theme has not been ported into `src/styles/global.css` yet.

