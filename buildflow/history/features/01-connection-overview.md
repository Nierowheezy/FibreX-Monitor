# Feature: Connection overview

**From build-plan:** feature 1
**Status:** verified

## Goal

Ship the first screen of FibreX Monitor: the unified dashboard area that answers
"is my connection healthy right now?" from connectivity state, latency, packet
loss, download/upload rates, and the time of the last successful probe.

This feature also lays the two foundations the rest of the MVP stands on: the
app shell (design tokens, layout, removal of the scaffold demo) and the
`ConnectionOverview` contract that later data features feed. Because probing,
health classification, and WAN counters arrive in features 2, 4, and 11, this
screen ships honest "no measurements yet" states rather than sample data.

## Design reference

- `/prototype` has run. The visual target is
  `prototypes/choose/overview.html` and `prototypes/choose/popover.html`, with
  `prototypes/choose/theme.css` as the token source. That directory holds both
  approved themes behind one markup, which is the arrangement this feature
  ships.
- The user approved **two complete themes** rather than one:
  - **Wispr**, from `prototypes/d-wispr/theme.css`, built from the published
    Wispr Flow style reference: cream `#ffffeb` canvas, 2px ink `#1a1a1a`
    borders, lavender `#f0d7ff` primary, 32px cards, no shadows, EB Garamond for
    display type with Figtree for every interface surface.
  - **Soft**, from `prototypes/c-soft/theme.css`, the original variant C: warm
    cream canvas, pastel tile tints, 20px cards, soft shadows, system sans.
- Both themes expose the **same 61 token names**, so they are two value sets for
  one contract rather than two designs. Components consume tokens only, never a
  literal colour, radius, font, or shadow.
- `project-plan.md` section 7 still governs: compact and useful, minimal visual
  noise, status colour always paired with a text label, no gradients, no
  glassmorphism, no oversized metric cards, no decorative charts. The one
  deliberate relaxation is card radius, since Wispr's signature is a 32px radius
  against section 7's tighter guidance, approved by the user to match the
  reference.

## In scope

- Design tokens in `src/styles/global.css`: Tailwind v4 `@theme` declaring every
  token name once, then four value blocks (Soft and Wispr, each in light and
  dark) scoped by `data-palette` and `data-theme` on the root element.
- Theme selection: both themes and all three appearance modes (system, light,
  dark) are user-selectable and persisted locally. Switching sets two
  attributes on the root element and nothing else, because components read
  tokens rather than values. The picker ships in the app shell header here;
  feature 18 moves it into Preferences.
- EB Garamond and Figtree bundled locally as woff2 under `src/assets/fonts/`.
  The app is local-first, so it must never fetch a font from a CDN at runtime.
- App shell: `src/app/App.tsx`, header with the product name, main content
  region, `OverviewScreen` container.
- Removal of all scaffold demo content: the greet form and logos, `App.css`,
  `assets/react.svg`, `public/vite.svg`, `public/tauri.svg`, the favicon link,
  and the `greet` Rust command.
- The `ConnectionOverview` contract in TypeScript and Rust, including
  `ValueState`, `HealthState`, and a `cargo test` that locks the first-run
  payload so a later change cannot smuggle in fabricated readings.
- The `get_connection_overview` Tauri command returning the honest first-run
  shape.
- Typed wrapper `getConnectionOverview` and the `useConnectionOverview` hook.
- Overview screen: status summary (health label, monitoring activity, last probe
  time) and four metric tiles (latency, packet loss, download rate, upload rate),
  each rendering loading, unavailable, and error states.

## Out of scope

- Probing, latency measurement, packet-loss windows (feature 2), and the
  freshness logic that decides when a real reading becomes `stale`. Nothing in
  this feature can be stale yet, so that logic ships with feature 2 where it gets
  a call site.
- Router-versus-external reachability separation (feature 3).
- Health-state classification logic (feature 4): this screen renders states, it
  does not compute them.
- SQLite persistence, history charts, incidents (features 5, 6, 7).
- Menu bar, tray, popover, pause/resume lifecycle (features 8, 9).
- Router integration and WAN counters (features 10, 11): both rate tiles stay
  `unavailable`.
- The router data-source status line, recent-history panel, and recent-incidents
  panel that `project-plan.md` section 7 lists for the Overview screen. They
  belong to features 10, 6, and 7, so this screen reserves no empty slots for
  them.
- Multi-screen navigation: there is no second screen yet, so no router.
  `src/app/router.tsx` is added by the first feature that needs a second screen.
- The Preferences window that will host the theme picker (feature 18), an
  accent-colour picker, notifications, and accessibility refinement (feature
  40). This feature ships the picker itself, in the shell header, so the choice
  is real and persisted from the first run.

## Build loop

Build one step at a time, never the whole feature at once.

1. Plan mode lays out the step before any code.
2. The AI implements just that step.
3. It shows the diff (not full files); you read it and understand it.
4. You approve, then choose whether to commit a checkpoint or roll straight on.

This project runs the Efficient preset (`buildflow/config.json`:
`stepReview: "feature"`, `checkpointCommits: "disabled"`), so review happens as
one packet after all steps rather than a pause after each one. Never accept a
step you have not read. If a diff is too big to review, the step was too big.

## Build steps

- [x] **Step 1 - Two-theme token layer** - port `prototypes/choose/theme.css`
  into `src/styles/global.css`. `@theme static` declares each token name once so
  Tailwind emits the utilities, then four blocks supply the values:
  `:root[data-palette="soft"]`, `:root[data-palette="soft"][data-theme="dark"]`,
  `:root[data-palette="wispr"]`, and
  `:root[data-palette="wispr"][data-theme="dark"]`, plus base element styling and
  a visible focus ring. EB Garamond and Figtree are bundled locally under
  `src/assets/fonts/` and loaded with `@font-face`. *Done when:* `npm run build`
  passes, all four combinations render as four visibly different themes under
  `npm run dev`, and a search for hex or rgb values under `src/` finds them only
  in `src/styles/global.css` and `src/assets/fonts/`.

- [x] **Step 2 - Overview contract and first-run command** -
  `src/types/overview.ts` (`ConnectionOverview`, `ValueState`, `HealthState`,
  `MetricValue<T>`), the serde mirror `src-tauri/src/models/overview.rs`, and
  `src-tauri/src/commands/overview.rs` with `get_connection_overview` returning
  the first-run shape (health `unknown`, `monitoring_active` false,
  `last_probe_at` null, all four metrics null with state `unavailable`,
  `packet_loss_window_ms` null), registered in `lib.rs` next to `greet`. Ships a
  `cargo test` asserting those values, so no `0`, no timestamp, and no
  `connected` state can appear in the first-run payload without failing.
  *Done when:* `cargo check` and `cargo test` pass, `npm run build` compiles the
  new types with no `any`, and `cargo test` proves `get_connection_overview`
  returns the contract over the real Tauri IPC boundary using the mock runtime
  (`tauri::test`), so no manual devtools step is required.

- [x] **Step 3 - Typed wrapper and hook** -
  `src/lib/tauri/overview.ts` exporting `getConnectionOverview()`, plus
  `src/hooks/useConnectionOverview.ts` returning a discriminated
  `loading | ready | error` state with the error message on rejection. Both
  files exist from the earlier pass, so this step re-verifies them against the
  done-when rather than rewriting them. *Done when:* `npm run build` passes, and
  a search for `invoke` under `src/features` and `src/components` finds
  nothing, with `src/lib/tauri/` as the only importer outside tests.

- [x] **Step 4 - Theme controller and picker** -
  `src/app/theme.ts` owning the two settings (`palette`: `soft | wispr`,
  `appearance`: `system | light | dark`), reading and writing `localStorage`,
  applying `data-palette` and `data-theme` to the root element before first
  paint, and following the OS while `appearance` is `system`.
  `src/app/ThemePicker.tsx` renders both controls: a two-option theme segment
  plus the sun, moon, and half-circle appearance icons. *Done when:* both themes
  and all three appearance modes are reachable and survive a reload, `system`
  tracks an OS change with no reload, every control reports `aria-pressed`, and
  the selected control is visible in both themes.

- [x] **Step 5 - Shell and status summary, restyled** - the shell, header,
  `OverviewScreen.tsx` and `StatusSummary.tsx` were built in the earlier pass and
  keep their behaviour. This step restyles them against the Step 1 tokens so the
  summary becomes the prototype's three-up stat strip (connection, monitoring,
  last probe), mounts `ThemePicker`, and replaces every literal colour with a
  token. *Done when:* the summary matches `prototypes/choose/overview.html`
  under both themes, `npm run build` and `cargo check` pass, and no hex or rgb
  value appears outside the token layer.

- [x] **Step 6 - Metric tiles** -
  `src/features/overview/MetricGrid.tsx` and `MetricTile.tsx` for latency, packet
  loss, download rate, and upload rate, plus shared
  `src/components/status/StatusBadge.tsx`, each tile pairing a value and unit
  with a text value-state label, and an unavailable value rendering as
  `No reading` rather than `0`. *Done when:* the tiles match the prototype under
  both themes, four tiles read `No reading` with an `Unavailable` badge on first
  run, no missing value ever renders as `0 ms`, `0 %`, or `0 Mbps`, and
  `npm run build`, `cargo check`, and `cargo test` all pass.

## Files / areas

| Area | Files |
| --- | --- |
| Styles | `src/styles/global.css` |
| Theme | `src/app/theme.ts`, `src/app/ThemePicker.tsx` |
| Fonts | `src/assets/fonts/` (bundled woff2; the `@font-face` block lives in `global.css`) |
| Shell | `src/app/App.tsx` (moved from `src/App.tsx`), `src/main.tsx`, `index.html` |
| Types | `src/types/overview.ts` |
| Frontend | `src/lib/tauri/overview.ts`, `src/hooks/useConnectionOverview.ts`, `src/features/overview/OverviewScreen.tsx`, `StatusSummary.tsx`, `MetricGrid.tsx`, `MetricTile.tsx`, `src/components/status/StatusBadge.tsx` |
| Rust | `src-tauri/src/models/overview.rs`, `src-tauri/src/commands/overview.rs`, `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml` (dev-only `tauri/test` feature) |
| Deleted | `src/App.tsx`, `src/App.css`, `src/assets/react.svg`, `public/vite.svg`, `public/tauri.svg` |

Create only the files each step needs. No empty placeholder modules, no
`router.tsx`, no `providers.tsx`.

## Data / contracts

No storage in this feature (feature 5 owns SQLite). The payload is in-memory,
built by Rust and read through the typed wrapper.

```ts
export type ValueState = "measured" | "calculated" | "stale" | "unavailable";

export type HealthState = "connected" | "degraded" | "offline" | "unknown" | "paused";

export interface MetricValue<T> {
  value: T | null;
  state: ValueState;
  measuredAt: number | null; // UTC epoch ms
}

export interface ConnectionOverview {
  healthState: HealthState;
  monitoringActive: boolean;
  lastProbeAt: number | null;
  packetLossWindowMs: number | null;
  latency: MetricValue<number>;      // ms
  packetLoss: MetricValue<number>;   // percent, 0..100
  downloadRate: MetricValue<number>; // bits per second
  uploadRate: MetricValue<number>;   // bits per second
}
```

Rust mirrors this with serde (camelCase field names). `value: null` is the only
representation of "no reading"; `0` is never an unavailable value, and
`measuredAt` is null until something real was measured. There is deliberately no
"assembled at" timestamp: it would read as data freshness while every metric is
still empty.

**Load-bearing.** Features 2, 4, 6, 7, 8, and 11 all consume this shape. The
menu-bar popover (8) shows exactly these five headline values, and history (6)
derives from the same measurement type. Change it deliberately and early rather
than after those features are built against it. `ValueState.stale` and
`packetLossWindowMs` become observable at feature 2.

## Testing

- **Rust:** `cargo test` is declared in `AGENTS.md`, so it is a gate for
  logic-bearing steps. Step 2 ships the first-run payload test, the wire-format
  test for the enums, and a mock-runtime IPC test that proves the command is
  registered and returns the contract. Freshness and rate-calculation logic
  belong to features 2 and 11, where they will get their own tests.
- **Frontend:** no `test` command is declared for JavaScript and no `Browser
  tests` command exists, so no runner is added mid-feature. Frontend steps ride
  on `npm run build` (which runs `tsc`) plus screenshots from `npm run tauri dev`.
  If you want a frontend test gate from the start, run `/tests` before this
  feature rather than adding a runner inside it.
- **Native evidence required.** `npm run dev` cannot call Tauri commands, so the
  data path is only real inside `npm run tauri dev`. Browser screenshots prove
  layout and tokens only.
- **Error state check:** the error panel is proven by temporarily removing
  `get_connection_overview` from `invoke_handler` in `lib.rs`, launching
  `npm run tauri dev`, confirming the panel renders, then reverting the line.
- **Gate order per step:** `npm run build`, then `cargo check`, then
  `cargo test`. There is no `Verify` command declared in `AGENTS.md` yet; `/ci`
  would define one later.

## Notes for the AI

- Rust is the single source of truth. React renders what the command returns and
  never synthesises a value, a timestamp, or a status colour it did not receive.
- First-run unavailable is the correct result, not a bug to work around. Do not
  add fixture data, seeded samples, or a mock command to make the screen look
  populated.
- A status colour is always paired with a text label (`StatusBadge`), never
  colour alone.
- Tailwind is v4 with no `tailwind.config.js`: tokens belong in `@theme` inside
  `src/styles/global.css`, referenced as theme utilities rather than arbitrary
  values.
- Frontend code reaches the backend only through `src/lib/tauri/`; components
  dispatch the typed wrapper and subscribe in `useEffect`, cleaning up on
  unmount.
- Keep `monitoringActive` false until feature 9 starts real monitoring, and never
  report `connected` until feature 4 classifies it.
- The layout must survive a narrow window: use a wrapping grid, not fixed
  pixel widths.
- Do not edit `buildflow/project-plan.md` or `buildflow/build-plan.md`; those are
  user-owned. `/complete` checks the build-plan box.
- No em dashes, en dashes, or ellipsis characters in code, comments, or docs.
  Comment the why, not the what.
- With two themes live, a hard-coded colour, radius, font, or shadow inside a
  component is a bug in both themes. The audit gate should treat any literal
  value outside the token layer as a finding: that rule is what keeps the second
  theme free rather than doubling the work for every later feature.
- The picker is mounted in the shell header for this feature. Feature 18 moves it
  into Preferences, so `src/app/theme.ts` must stay independent of where the
  control lives and make that a relocation rather than a rewrite.
- If a step's diff grows past what you would read in one sitting, stop and split
  it rather than shipping it as one review.
