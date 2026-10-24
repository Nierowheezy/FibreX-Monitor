import { useId } from "react";

/**
 * The speed test's dial.
 *
 * A needle over an arc, the way a speed test has always shown it, because it
 * is the shape that reads at a glance: where the needle sits is the number.
 *
 * The scale is logarithmic between the marks. A line can be a thousand times
 * faster or slower than the one next to it, and a linear scale would press every
 * ordinary speed into the first few degrees of the dial. The tick labels say
 * what each mark is, so the dial is never a mystery.
 *
 * While the test runs the needle scans back and forth across the scale and the
 * figure in the middle is the real rate the transfer has reached. When the test
 * ends the needle settles on the measured value.
 */

/** The scale's marks, in megabits per second. */
const TICKS = [0, 1, 10, 25, 50, 100, 250, 500, 1000];

/** Where the arc starts and ends, in degrees. Zero is up, positive is clockwise. */
const START_ANGLE = -120;
const END_ANGLE = 120;

const CENTRE_X = 180;
const CENTRE_Y = 186;
const RADIUS = 135;

/** How far the needle reaches, and how far its tail goes back. */
const NEEDLE_LENGTH = 110;
const NEEDLE_TAIL = 20;

/**
 * The angle a value sits at.
 *
 * The position between two marks is taken in log space, so that the gap
 * between 1 and 10 Mbps takes as much room as the gap between 500 and 1000.
 */
export function angleFor(megabits: number): number {
  if (!Number.isFinite(megabits) || megabits <= 0) {
    return START_ANGLE;
  }

  const scale = (value: number) => Math.log10(value + 1);
  const target = scale(megabits);

  for (let index = 0; index < TICKS.length - 1; index += 1) {
    const low = TICKS[index];
    const high = TICKS[index + 1];
    if (megabits <= high) {
      const lowScaled = scale(low);
      const highScaled = scale(high);
      const share =
        highScaled === lowScaled ? 0 : (target - lowScaled) / (highScaled - lowScaled);
      const step = (END_ANGLE - START_ANGLE) / (TICKS.length - 1);
      return START_ANGLE + step * (index + Math.min(Math.max(share, 0), 1));
    }
  }

  return END_ANGLE;
}

/** A point on the dial, in the view box's coordinates. */
function pointAt(angle: number, radius: number): { x: number; y: number } {
  const radians = (angle * Math.PI) / 180;
  return {
    x: CENTRE_X + radius * Math.sin(radians),
    y: CENTRE_Y - radius * Math.cos(radians),
  };
}

/** The arc from one end of the scale to the other. */
function arcPath(): string {
  return arcBetween(START_ANGLE, END_ANGLE);
}

/**
 * The arc between two angles.
 *
 * The sweep is clockwise, because the scale runs from the lower left to the
 * lower right through the top, which is the way a speed test's dial has always
 * gone round.
 */
function arcBetween(from: number, to: number): string {
  const start = pointAt(from, RADIUS);
  const end = pointAt(to, RADIUS);
  // The large-arc flag is set when the sweep is more than half a circle.
  const large = Math.abs(to - from) > 180 ? 1 : 0;
  return `M ${start.x} ${start.y} A ${RADIUS} ${RADIUS} 0 ${large} 1 ${end.x} ${end.y}`;
}

export default function SpeedGauge({
  megabits,
  value,
  unit,
  caption,
  running,
}: {
  /** The rate the needle points at, in megabits per second. */
  megabits: number | null;
  /** The figure in the middle of the dial. */
  value: string;
  /** The unit of that figure. */
  unit: string;
  /** What the figure is a measurement of. */
  caption: string;
  /** Whether the test is still running. */
  running: boolean;
}) {
  const gradientId = useId();
  const angle = angleFor(megabits ?? 0);

  // The needle is drawn pointing up, and the group carries the rotation, so
  // that the sweeping animation and the resting position are the same property
  // and one replaces the other cleanly.
  const needleTip = { x: CENTRE_X, y: CENTRE_Y - NEEDLE_LENGTH };
  const needleTail = { x: CENTRE_X, y: CENTRE_Y + NEEDLE_TAIL };

  return (
    <svg
      viewBox="0 0 360 300"
      className="block h-auto w-full max-w-[440px]"
      role="img"
      aria-label={`${caption}: ${value} ${unit}`}
    >
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--color-ember)" stopOpacity="0.3" />
          <stop offset="100%" stopColor="var(--color-ember)" stopOpacity="0" />
        </linearGradient>
      </defs>

      {/* The filled part of the scale, up to where the measured rate sits. It
          grows with the reading while the test runs, so the arc and the figure
          always agree. */}
      <path
        d={arcBetween(START_ANGLE, Math.max(angle, START_ANGLE))}
        fill="none"
        stroke={`url(#${gradientId})`}
        strokeWidth="28"
        strokeLinecap="round"
      />
      <path
        d={arcPath()}
        fill="none"
        stroke="var(--color-hairline-2)"
        strokeWidth="2"
        strokeLinecap="round"
      />

      {/* The marks and their labels. */}
      {TICKS.map((tick, index) => {
        const step = (END_ANGLE - START_ANGLE) / (TICKS.length - 1);
        const angle = START_ANGLE + step * index;
        const outer = pointAt(angle, RADIUS + 11);
        const inner = pointAt(angle, RADIUS - 3);
        const label = pointAt(angle, RADIUS + 26);

        return (
          <g key={tick}>
            <line
              x1={inner.x}
              y1={inner.y}
              x2={outer.x}
              y2={outer.y}
              stroke="var(--color-hairline-2)"
              strokeWidth="1.5"
            />
            <text
              x={label.x}
              y={label.y}
              textAnchor="middle"
              dominantBaseline="middle"
              className="fill-ink-faint font-mono"
              fontSize="11"
            >
              {tick}
            </text>
          </g>
        );
      })}

      {/* The needle. */}
      <g
        style={{
          transform: `rotate(${angle}deg)`,
          // The origin has to be the dial's centre, and the origin of a CSS
          // transform on an SVG element is its own box unless this says
          // otherwise. Getting it wrong swings the needle around the wrong
          // point instead of around the hub.
          transformBox: "view-box",
          transformOrigin: `${CENTRE_X}px ${CENTRE_Y}px`,
          transition: running
            ? "none"
            : "transform 700ms cubic-bezier(0.2, 0.8, 0.2, 1)",
        }}
        className={running ? "gauge-sweep" : undefined}
      >
        <line
          x1={needleTail.x}
          y1={needleTail.y}
          x2={needleTip.x}
          y2={needleTip.y}
          stroke="var(--color-ember)"
          strokeWidth="3"
          strokeLinecap="round"
        />
      </g>
      <circle cx={CENTRE_X} cy={CENTRE_Y} r="7" fill="var(--color-ember)" />
      <circle cx={CENTRE_X} cy={CENTRE_Y} r="2.5" fill="var(--color-canvas)" />

      {/* The figure, in the middle of the dial, the way a speed test has always
          put it. */}
      <text
        x={CENTRE_X}
        y={CENTRE_Y + 62}
        textAnchor="middle"
        className="fill-ink font-display"
        fontSize="46"
        fontWeight="300"
        letterSpacing="-1.5"
      >
        {value}
      </text>
      <text
        x={CENTRE_X}
        y={CENTRE_Y + 84}
        textAnchor="middle"
        className="fill-ink-faint font-mono"
        fontSize="11"
        letterSpacing="1.2"
      >
        {unit.toUpperCase()}
      </text>
      <text
        x={CENTRE_X}
        y={CENTRE_Y + 104}
        textAnchor="middle"
        className="fill-ink-faint font-mono"
        fontSize="10"
        letterSpacing="1.6"
      >
        {caption.toUpperCase()}
      </text>
    </svg>
  );
}
