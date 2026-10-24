import {
  IncidentsIcon,
  OverviewIcon,
  RouterIcon,
  SettingsIcon,
  SpeedTestIcon,
} from "../components/ui/icons";
import { MONITORING_SCREENS, type ScreenId } from "./screens";

type ScreenEntry = {
  id: ScreenId;
  label: string;
  icon: (props: { className?: string }) => React.ReactNode;
};

/** Every screen with the words and glyph the rail shows for it. */
const SCREENS: Record<ScreenId, ScreenEntry> = {
  overview: { id: "overview", label: "Overview", icon: OverviewIcon },
  router: { id: "router", label: "Fibre and Router", icon: RouterIcon },
  speedtest: { id: "speedtest", label: "Speed test", icon: SpeedTestIcon },
  incidents: { id: "incidents", label: "Incidents", icon: IncidentsIcon },
  settings: { id: "settings", label: "Settings", icon: SettingsIcon },
};

function RailButton({
  entry,
  active,
  onSelect,
}: {
  entry: ScreenEntry;
  active: boolean;
  onSelect: (id: ScreenId) => void;
}) {
  const Icon = entry.icon;
  return (
    <button
      type="button"
      onClick={() => onSelect(entry.id)}
      aria-current={active ? "page" : undefined}
      // The label is on the button rather than in a tooltip-only span, so the
      // name reaches a screen reader as well as the mouse.
      aria-label={entry.label}
      title={entry.label}
      className={`grid size-12 place-items-center rounded-pill border-0 transition-colors duration-[var(--motion)] ${
        active
          ? "bg-ink text-canvas"
          : "text-ink-faint hover:bg-surface-3 hover:text-ink"
      }`}
    >
      {/* The glyph states its own colour from the button's computed text colour,
          rather than relying on inheriting it: a base `button { color }` rule
          once beat the utility and painted this near-white on a near-white pill. */}
      <Icon className="size-5" />
    </button>
  );
}

/**
 * The floating navigation rail.
 *
 * A fixed capsule rather than a full-height sidebar: the app has five
 * destinations and a permanent column would take width from the bento for a
 * list that never grows. The active item inverts to a filled circle, which is
 * the one place ink and canvas swap roles.
 */
export default function Rail({
  current,
  onSelect,
}: {
  current: ScreenId;
  onSelect: (id: ScreenId) => void;
}) {
  return (
    <nav
      aria-label="Sections"
      className="fixed top-1/2 left-[var(--s5)] z-40 flex -translate-y-1/2 flex-col gap-gap rounded-pill border border-hairline bg-overlay p-gap shadow-float"
    >
      {MONITORING_SCREENS.map((id) => (
        <RailButton
          key={id}
          entry={SCREENS[id]}
          active={current === id}
          onSelect={onSelect}
        />
      ))}

      <div className="mx-2 my-1 h-px bg-hairline" />

      <RailButton
        entry={SCREENS.settings}
        active={current === "settings"}
        onSelect={onSelect}
      />
    </nav>
  );
}
