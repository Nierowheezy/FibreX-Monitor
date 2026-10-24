# Feature: Design port to the approved prototype

**From build-plan:** feature 8b
**Status:** verified

## Goal

Make the real app look like `prototypes/dashboard.html` and
`prototypes/panel.html`, which the user approved after rejecting the earlier
theme. One theme, not two. The dashboard gets the status strip, the focal hero
pair, hairline cards, the animated latency chart, and a single-icon appearance
menu with a System option. The panel gets the same treatment at its own size.

This goes before the monitoring loop so features 2b onward land in the final
design instead of being restyled afterwards.

## Design reference

- `prototypes/theme.css` - the token source, ported in Step 1.
- `prototypes/dashboard.html` - the dashboard target, light and dark.
- `prototypes/panel.html` - the panel target at its real 340x460 size.

These are kept, not discarded. `AGENTS.md` records that, so `/complete` does
not delete them.

## What changed from the rejected theme, and why

Read from the real Wispr Flow site's computed styles rather than a style
document, which corrected the earlier build:

| Rejected | Corrected |
| --- | --- |
| 2px ink border on every card, button and badge | 1px `#d1d1c1` hairline; the 2px ink border appears three times on their page and is kept here for the hero and the primary button only |
| 32px card radius everywhere | 16px cards, 32px and 48px for focal surfaces, 12px controls, pills |
| Cream, ink, lavender, forest, amber | plus coral `#ff6c4c`, pink `#ffbcf2`, warm grey `#a39d8e`, off-white `#fffdf9` |
| No positive tracking on small text | `+0.015em` on labels and `+0.03em` on micro labels, which is on their site |
| No chart | a market-style area chart with a gradient wash and a draw-in animation |

The Soft theme is retired. Two themes meant every future feature was verified
twice, and the user chose one excellent theme instead.

## In scope

- `src/styles/global.css` rebuilt from `prototypes/theme.css`: one theme, light
  and dark, plus the chart tokens, exposed through Tailwind v4 `@theme`.
- The dashboard restyled to the prototype: status strip, hero pair, metric tiles,
  and the status vocabulary strip.
- `src/components/chart/LatencyChart.tsx`: a hand-built SVG chart, no charting
  library, with the gradient wash, the draw-in animation, the pulsing latest
  point, and dashed bridges across missing samples.
- The appearance control replaced by a single icon button opening a System /
  Light / Dark menu, the way Vercel does it.
- The panel restyled to the prototype.
- `@font-face` for JetBrains Mono, added to the two bundled fonts.

## Out of scope

- **Any real chart data.** The monitoring loop is 2b and history is 6, so the
  chart ships its honest empty state: "No readings to plot yet". No sample
  numbers reach product code.
- **Time-range switching.** 1m to 1h needs stored samples, which is feature 6.
- **Pause and resume.** Feature 9.
- **A status-coloured tray icon.** Still no measured states to colour.
- **Rounded panel corners.** Still no `macOSPrivateApi`, so the App Store option
  stays open.
- **Rust changes.** This is a frontend feature.

## Build steps

- [x] **Step 1 - Port the token layer** - rebuild `src/styles/global.css` from
  `prototypes/theme.css` as a single theme with light and dark, add the chart
  tokens and the new type scale, and add JetBrains Mono. *Done when:*
  `npm run build` passes, no colour literal exists outside `global.css` and
  `src/assets/fonts/`, and both appearance modes resolve their tokens.

- [x] **Step 2 - Restyle the dashboard** - the status strip, hero pair, metric
  tiles and status vocabulary, matching `prototypes/dashboard.html`. *Done when:*
  the dashboard matches the prototype in both modes with no literal colours in
  any component.

- [x] **Step 3 - The icon appearance menu** - one icon button opening System,
  Light and Dark, replacing the segmented control, with the OS followed live in
  System mode. *Done when:* all three modes are reachable and persist, System
  tracks an OS change without a reload, the menu closes on Escape and on an
  outside click, and `aria-expanded` and `aria-checked` are correct.

- [x] **Step 4 - The latency chart** - `LatencyChart.tsx` drawing to SVG with the
  gradient wash, draw-in animation, pulsing latest point and dashed bridges
  across gaps. *Done when:* it renders its empty state with no readings, renders
  a supplied series with those four behaviours, keeps gaps visible rather than
  interpolating, and honours both appearance modes.

- [x] **Step 5 - Restyle the panel** - the panel to the prototype at 340x460. *Done
  when:* it matches `prototypes/panel.html` in both modes, still fits without
  scrolling, and shows the same values as the dashboard.

## Files / areas

| Area | Files |
| --- | --- |
| Styles | `src/styles/global.css` |
| Shell | `src/app/App.tsx`, `src/app/AppearanceMenu.tsx` (replaces the segmented picker), `src/app/ThemePicker.tsx` (deleted) |
| Chart | `src/components/chart/LatencyChart.tsx`, `src/types/latency.ts` |
| Dashboard | `src/features/overview/OverviewScreen.tsx`, `StatusSummary.tsx`, `MetricGrid.tsx`, `MetricTile.tsx` |
| Panel | `src/panel/PanelScreen.tsx` |
| Fonts | `src/assets/fonts/` (JetBrains Mono added) |
| Unchanged | all of `src-tauri/` |

## Testing

- `npm run build` after every step, since it runs `tsc`.
- `cargo check`, `cargo test` and `cargo clippy --all-targets` must stay green,
  as a regression net. This feature touches no Rust.
- No JavaScript test runner is configured and none is added, so the frontend
  rides on the build plus direct visual evidence in both appearance modes.
- The empty chart state is the honest default and must be screenshotted, since
  a chart that invents a line is the failure this app cannot ship.

## Notes for the AI

- Components consume tokens only. A literal colour, radius, font or shadow in a
  component is a defect, because the whole value of one theme is that it can be
  replaced in one file.
- The chart draws gaps as gaps. The plan forbids interpolating fake continuity
  across missing samples, so `null` must break the line and be marked, never
  bridged silently.
- Lavender keeps a fixed foreground. `--on-accent` does not flip between modes
  because the fill does not, and that bug shipped once already: cream text on
  lavender was unreadable in dark mode.
- Appearance mode is `system | light | dark`, resolved through one attribute, and
  it was already verified to persist and follow the OS. Reuse
  `src/app/theme.ts` rather than writing new state.
- No em dashes, en dashes, or ellipsis characters in code, comments, or docs.
## Note on 8a's record

8a (the menu-bar shell and panel content) was merged to `main` as `6888ba1`
before this feature was specced, and its spec file was overwritten rather than
archived. That commit is therefore 8a's only durable record, and the build-plan
box for 8a was ticked then, to keep the plan honest about what has shipped.
Nothing was lost: the merged commit carries the full diff and message.
