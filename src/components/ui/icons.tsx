import type { ReactNode } from "react";

/**
 * The icon set, inline.
 *
 * Hand-written paths rather than an icon dependency, so the app ships no font
 * or package for a dozen glyphs and every stroke weight matches the one below.
 * All of them are decorative: each is paired with a visible text label, so
 * `aria-hidden` is correct rather than an accessibility gap.
 */

type IconProps = { className?: string };

const railStroke = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.7,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

function Svg({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" className={className} {...railStroke}>
      {children}
    </svg>
  );
}

export function OverviewIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <rect x="3" y="3" width="7" height="9" rx="2" />
      <rect x="14" y="3" width="7" height="5" rx="2" />
      <rect x="14" y="12" width="7" height="9" rx="2" />
      <rect x="3" y="16" width="7" height="5" rx="2" />
    </Svg>
  );
}

export function RouterIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <rect x="3" y="13" width="18" height="8" rx="2" />
      <path d="M7 17h.01M11 17h.01" />
      <path d="M12 9V5M9.5 6.5a3.5 3.5 0 0 1 5 0M7 4a7 7 0 0 1 10 0" />
    </Svg>
  );
}

export function DevicesIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <rect x="2" y="4" width="14" height="10" rx="2" />
      <path d="M6 19h6" />
      <circle cx="18" cy="16" r="4" />
      <path d="M18 14v4M16.5 16h3" />
    </Svg>
  );
}

export function IncidentsIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M10.3 3.9L1.9 18a2 2 0 0 0 1.7 3h16.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" />
      <path d="M12 9v4M12 17h.01" />
    </Svg>
  );
}

export function SettingsIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </Svg>
  );
}

export function OpticalIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M12 2v4M12 18v4M2 12h4M18 12h4" />
      <circle cx="12" cy="12" r="4" />
    </Svg>
  );
}

export function TrafficIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M3 17l5-5 4 3 8-8" />
      <path d="M14 7h6v6" />
    </Svg>
  );
}

export function ClockIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3 2" />
    </Svg>
  );
}

export function GlobeIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18" />
    </Svg>
  );
}

export function SignalIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M4 19h2v-4H4zM9 19h2V9H9zM14 19h2V5h-2zM19 19h2v-7h-2z" />
    </Svg>
  );
}

export function ShieldIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M12 3l8 3v6c0 5-3.4 8.3-8 9-4.6-.7-8-4-8-9V6z" />
    </Svg>
  );
}

export function SystemIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <circle cx="12" cy="12" r="8" />
      <path d="M12 4a8 8 0 0 0 0 16z" fill="currentColor" stroke="none" />
    </Svg>
  );
}

export function SunIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
    </Svg>
  );
}

export function MoonIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" />
    </Svg>
  );
}

/** Four corners pointing out: enter fullscreen. */
export function ExpandIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />
    </Svg>
  );
}

/** Four corners pointing in: leave fullscreen. */
export function CollapseIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M4 14h6v6M20 10h-6V4M14 10l7-7M10 14l-7 7" />
    </Svg>
  );
}

/** A chevron pointing left, for stepping back through a list. */
export function ChevronLeftIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M15 18l-6-6 6-6" />
    </Svg>
  );
}

/** A chevron pointing right, for stepping forward through a list. */
export function ChevronRightIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M9 18l6-6-6-6" />
    </Svg>
  );
}

/** An arrow into a tray: traffic arriving from the internet. */
export function DownloadIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M12 4v14M6 13l6 6 6-6" />
    </Svg>
  );
}

/** An arrow out of a tray: traffic leaving towards the internet. */
export function UploadIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M12 20V6M6 11l6-6 6 6" />
    </Svg>
  );
}

/** A pulse trace: the shape of probe outcomes over time. */
export function PulseIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M2 12h4l3-8 4 16 3-8h6" />
    </Svg>
  );
}

/** Two bars: pause the monitoring loop. */
export function PauseIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M10 4H6v16h4zM18 4h-4v16h4z" />
    </Svg>
  );
}

/** A triangle: resume the monitoring loop. */
export function PlayIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M7 4l12 8-12 8z" />
    </Svg>
  );
}

/** A screen on a stand: open the dashboard. */
export function DashboardIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <rect x="3" y="4" width="18" height="14" rx="2" />
      <path d="M8 21h8M12 18v3" />
    </Svg>
  );
}

/** A cylinder: the cumulative total. */
export function DatabaseIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <ellipse cx="12" cy="6" rx="8" ry="3" />
      <path d="M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6" />
      <path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3" />
    </Svg>
  );
}

/**
 * A speedometer: a needle over a dial.
 *
 * The dial and the needle are separate, so the needle can be drawn at the angle
 * the reading is at rather than as a fixed glyph.
 */
export function SpeedTestIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M4.5 18a9 9 0 1 1 15 0" />
      <path d="M12 14l4-4" />
      <circle cx="12" cy="14" r="1.4" />
    </Svg>
  );
}

/** A dial with a needle, for a measurement rather than a reading. */
export function GaugeIcon({ className }: IconProps) {
  return (
    <Svg className={className}>
      <path d="M3.5 18a9 9 0 1 1 17 0" />
      <path d="M12 18l4.5-5" />
      <circle cx="12" cy="18" r="1.5" />
      <path d="M6.5 12h.01M17.5 12h.01" />
    </Svg>
  );
}

/**
 * The app mark: three chevrons falling off in opacity, which reads as a signal
 * fanning out from a point.
 *
 * Adopted from the prototype's topbar wordmark. The ember fill is inlined as a
 * token reference rather than a literal so it follows the light theme.
 */
export function Wordmark({ className }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" className={className}>
      <path d="M12 2.6l3.1 5.3a4.6 4.6 0 0 0-6.2 0L12 2.6z" fill="var(--color-ember)" />
      <path
        d="M6.6 9.4l3.2 5.5a4.6 4.6 0 0 0-6.3 0l3.1-5.5z"
        fill="var(--color-ember)"
        opacity="0.55"
      />
      <path
        d="M17.4 9.4l-3.2 5.5a4.6 4.6 0 0 1 0-5.5z"
        fill="var(--color-ember)"
        opacity="0.3"
      />
    </svg>
  );
}
