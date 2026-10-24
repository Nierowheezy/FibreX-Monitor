# Coding Standards

> Your conventions. Tuned by `/onboard` for FibreX Monitor. Edit freely; the
> stack is React + TypeScript + Vite in a Tauri 2 shell, with a Rust backend and
> local SQLite history.
>
> `/onboard` keeps this file, `AGENTS.md`, `CLAUDE.md` when present,
> `ai-interaction.md`, `.gitignore`, and README placement aligned with the real
> project stack. Re-run it when the stack changes.

## TypeScript

- Strict mode enabled
- No `any` types - use proper typing or `unknown`
- Define interfaces for all props, API responses, and data models
- Use type inference where obvious, explicit types where helpful

## React

- Functional components only, no class components
- Use hooks for state and side effects
- Keep components focused - one job per component
- Extract reusable logic into custom hooks
- Prefer React state and context for shared UI state; add a state library only
  when the implementation shows a real need for one
- Subscribe to Tauri events in `useEffect` and always clean up listeners on unmount
- Components render state and dispatch typed Tauri commands; they never open
  sockets, ping hosts, or parse raw router responses

## Vite and the Tauri boundary

- Vite builds the frontend bundle. There is no server, no SSR, and no server routes.
- The frontend runs inside the Tauri webview. Browser and packaged-app behaviour
  are not identical: verify native behaviour in `npm run tauri dev` or a packaged build.
- All privileged work - network probes, router requests, SQLite, notifications,
  tray, and background monitoring - lives in Rust behind typed Tauri commands and events.
- Frontend code reaches the backend through the wrappers in `src/lib/tauri/`, not
  through `invoke` scattered across components.
- Keep long-running monitoring out of React lifecycle effects; the backend owns
  the monitoring loop.
- `npm run dev` is for fast UI iteration only. It cannot exercise native APIs.

## File Organization

Frontend (`src/`), matching the structure proposed in `project-plan.md`:

- App shell: `src/app/App.tsx`, `router.tsx`, `providers.tsx`
- Screens: `src/features/[feature]/` (overview, connectivity, traffic, router, devices, incidents, settings)
- Shared components: `src/components/[layout|navigation|charts|status|ui]/ComponentName.tsx`
- Hooks: `src/hooks/useThing.ts`
- Backend wrappers: `src/lib/tauri/`
- Types: `src/types/[feature].ts`
- Styles and design tokens: `src/styles/`

Rust backend (`src-tauri/src/`):

- Commands: `commands/[feature].rs`
- Monitoring: `monitoring/`
- Router client: `router/`
- Incidents: `incidents/`
- Notifications: `notifications/`
- Storage: `storage/`
- Models: `models/`

Create only the modules the current step needs; do not pre-create empty files.

## Naming

- Components: PascalCase (`ItemCard.tsx`)
- Files: Match component name or kebab-case
- Functions: camelCase
- Constants: SCREAMING_SNAKE_CASE
- Types/Interfaces: PascalCase (no prefix)

## Styling

- Tailwind CSS for layout and component styling
- CSS variables in `src/styles/` for design tokens (colour, spacing, typography);
  expose them through the Tailwind theme rather than hardcoding values
- Lucide React for icons
- No inline styles
- Support system, light, and dark appearance; system is the default
- Pair status colours with a text label, never colour alone
- Share one design system between the main window and the menu-bar popover

## Database

- SQLite, accessed from Rust in `src-tauri/src/storage/`. The frontend never
  touches the database directly.
- Schema changes go through explicit, versioned migrations checked into the repo.
  Never edit a migration that has already shipped.
- Store the database under the macOS application-data directory, not inside the
  app bundle.
- Use typed models for measurements, traffic samples, router snapshots, incidents,
  and settings; keep row types separate from domain types where it clarifies intent.
- Wrap related writes (for example an incident and its measurements) in a
  transaction, and index the timestamp columns used by history queries.
- Handle initialisation, corruption, and recovery without crashing the app.

## Data Flow

- The Rust backend is the single source of truth for connection state,
  measurements, and incidents.
- The frontend reads data through typed Tauri commands and subscribes to Tauri
  events for live updates.
- Validate every payload that crosses the Tauri boundary, in Rust, before it
  reaches storage or the UI. Treat router responses as untrusted input.
- Keep command inputs and event payloads typed on both sides; never pass untyped
  `any` across the boundary.
- There are no user accounts and no remote calls in the MVP. Do not add a hosted
  API or an auth layer.

## Error Handling

- Rust commands return typed `Result` values; use an error enum and convert it to
  a serialisable error for the frontend.
- Never swallow an error silently or substitute fabricated data. A failed probe,
  an unavailable endpoint, or a stale reading must surface as an explicit state.
- Distinguish measured, calculated, stale, and unavailable values in both the
  backend and the UI.
- The frontend renders loading, empty, stale, unavailable, and error states for
  every data surface.
- Bound retries and set explicit timeouts on all network operations; cancel
  in-flight work on shutdown.

## Testing

The buildflow installs no test runner; testing is opt-in at the project level,
because the overlay can't know your stack. Adding unit testing is an explicit
setup task the AI can do through the normal workflow, either as a build-plan item
or with `/tests`. The setup should choose the stack-native runner, wire the
scripts or commands, add a small example test, and update the Commands section
of `AGENTS.md`.

When `AGENTS.md` declares a `Verify` command, treat it as the umbrella automated
gate. It combines only the checks this project actually has, in this order when
available: typecheck, tests, then build. The command does not enable an absent
test runner or replace focused evidence. It gives local work and optional CI one
exact command to run. `/ci` owns Verify and CI setup. `/tests` adds the real test
command to Verify when it already exists, but never creates CI only because
testing was configured.

**The opt-in switch is one signal: a `test` command in the Commands section of
`AGENTS.md`.** Declare one and **tests become a gate for logic-bearing steps**,
not an optional extra; leave it out and the loop verifies logic with the evidence
it already uses (run it, a screenshot, the build). Adding the runner is itself a
deliberate step, never a silent mid-step install. This is the single definition
of the switch; the skills and `ai-interaction.md` only point back here.

- **What to test (the scope rule):** pure logic where a wrong answer is possible -
  parsers, formatters, validators, id/slug builders, server actions. These have
  assertable inputs and outputs and real edge cases (empty, missing, malformed).
- **What not to test:** UI components and integration-level surfaces (render or
  export routes, anything driving a real browser or external service). Verify those
  with a screenshot and the build, not brittle unit tests.
- **The gate (when a runner is configured):** a build step that adds in-scope logic
  must ship a passing test in the same reviewable diff. The project's test command
  must be green before the step is approved, before any checkpoint commit, and
  before `/complete` merges. UI and integration-only steps are exempt and ride on
  screenshot plus build evidence.
- **When it's named:** the `/feature` spec's Testing section predicts the coverage,
  `/implement` writes the test with the step, and if a step surfaces logic the spec
  didn't foresee, add a focused test then.
- An empty suite should fail, not pass, so "no tests ran" never looks like "passed".
- Test files live next to source files (for example `feature.test.ts`).
- Run them via the project's test command (see Commands in `AGENTS.md`), not a
  hardcoded tool name.

Stack binding for this project: Vitest with `vi.mock()` and `vi.useFakeTimers()`
for frontend logic, and `cargo test` for Rust modules such as router response
parsers, the connection-state engine, and traffic-rate calculations. The test
gate turns on only when a real `test` command exists in `AGENTS.md`.

## Browser Verification

For UI and integration behavior, prefer real browser evidence over reading the
code and assuming it works.

- Browser automation is separately opt-in through `/browser-tests`. That setup
  reuses a compatible runner or prefers Playwright for supported projects, then
  documents the exact command as `Browser tests` in `AGENTS.md`.
- When `Browser tests` is declared, add focused coverage for stable behavioral
  done-whens when it is proportionate, and run the documented command during
  `/check`. Do not assume it proves visual fidelity, real authenticated-profile
  behavior, browser chrome, or another claim the test does not observe.
- If no Browser tests command is declared, do not add a runner silently in the
  middle of an unrelated feature. Use the available dev server, browser
  screenshots, build output, API output, or manual evidence instead.
- Browser tests are not part of the default Verify command or CI unless the user
  separately chooses that slower gate.
- Browser evidence is especially important for flows that click, type, submit,
  navigate, download files, render complex layouts, or depend on client-side
  state.
- Browser evidence is not enough for native behaviour. Menu-bar, tray, window
  lifecycle, notifications, background monitoring, and packaging claims must be
  verified in `npm run tauri dev` or a packaged build, not only in the browser.

## Code Quality

- No commented-out code unless specified
- No unused imports or variables
- Keep functions under 50 lines when possible

## Comments

Write code that explains itself; comment only what the code cannot say.
Over-commenting is a common AI tell, so resist it.

- Comment the **why**, not the **what**. Delete any comment that restates the code.
- No banner/header blocks, section dividers, or step-by-step narration of obvious
  code. A file does not need a comment announcing each region.
- A comment earns its place only when it captures something the code can't: a
  non-obvious decision, a gotcha or workaround, why a value is what it is, or a
  link to a spec or issue.
- Prefer self-documenting names and small functions over explanatory comments.
- Keep doc comments minimal: a one-line purpose on an exported type or function is
  plenty; don't write JSDoc that just repeats the signature.
- When in doubt, leave the comment out.

## Writing

- No em dashes (U+2014) in generated content: docs, comments, commit messages,
  READMEs, specs. They read as AI-generated.
- Use a hyphen for `term - description` separators; rephrase prose with commas,
  parentheses, or a colon. Avoid en dashes and the ellipsis character too.
