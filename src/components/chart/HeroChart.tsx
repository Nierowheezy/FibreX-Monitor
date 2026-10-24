import { buildScale, HERO_GEOMETRY } from "./scale";
import { hasDrawableRun, lastPoint, smoothPath, splitRuns } from "./series";
import type { PlotPoint } from "./series";

/**
 * The hero card's background chart.
 *
 * The design puts the latency line behind the figure, full bleed, with the
 * grid lines barely visible and no axis at all. It is a picture of the shape
 * of the connection, not a chart to read a value off, so it carries no labels
 * and no numbers.
 *
 * The same series drives it as the detailed chart below, so the two can never
 * show different stories.
 */
export default function HeroChart({
  points,
  gradientId = "hero-wash",
}: {
  points: PlotPoint[];
  gradientId?: string;
}) {
  const present = points.filter((point) => point.value !== null);

  if (present.length < 2) {
    return null;
  }

  const scale = buildScale(
    present.map((point) => ({ at: point.at, value: point.value as number })),
    HERO_GEOMETRY,
  );

  const runs = splitRuns(points, scale.xFor, scale.yFor);

  if (!hasDrawableRun(runs)) {
    return null;
  }

  const end = lastPoint(runs);
  const baseline = scale.plotBottom;

  return (
    <svg
      viewBox={`0 0 ${HERO_GEOMETRY.width} ${HERO_GEOMETRY.height}`}
      preserveAspectRatio="none"
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 size-full"
    >
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--chart-area-top)" />
          <stop offset="100%" stopColor="var(--chart-area-bottom)" />
        </linearGradient>
      </defs>

      {/* Four grid lines, the same count the design draws. */}
      {[0, 1, 2, 3].map((step) => {
        const y = scale.plotTop + (scale.plotBottom - scale.plotTop) * (step / 3);
        return (
          <line
            key={step}
            className="stroke-[var(--chart-grid)]"
            x1={scale.plotLeft}
            y1={y}
            x2={scale.plotRight}
            y2={y}
            strokeWidth={1}
            vectorEffect="non-scaling-stroke"
          />
        );
      })}

      {runs.map((segment, index) =>
        segment.length > 1 ? (
          <path
            key={`area-${index}`}
            d={`${smoothPath(segment)} L${segment[segment.length - 1][0]},${baseline} L${segment[0][0]},${baseline} Z`}
            fill={`url(#${gradientId})`}
            stroke="none"
          />
        ) : null,
      )}

      {runs.map((segment, index) =>
        segment.length > 1 ? (
          <path
            key={`line-${index}`}
            d={smoothPath(segment)}
            fill="none"
            stroke="var(--chart-line)"
            strokeWidth={1.75}
            strokeLinecap="round"
            strokeLinejoin="round"
            vectorEffect="non-scaling-stroke"
          />
        ) : null,
      )}

      {runs.slice(0, -1).map((segment, index) => {
        const endOfRun = segment[segment.length - 1];
        const nextStart = runs[index + 1][0];
        return (
          <line
            key={`gap-${index}`}
            className="stroke-[var(--chart-gap)]"
            x1={endOfRun[0]}
            y1={endOfRun[1]}
            x2={nextStart[0]}
            y2={nextStart[1]}
            strokeWidth={1.75}
            strokeDasharray="3 5"
            vectorEffect="non-scaling-stroke"
          />
        );
      })}

      {end !== null && (
        <circle
          className="fill-[var(--chart-dot)] origin-center animate-[chart-pulse_2.4s_ease-in-out_infinite]"
          cx={end[0]}
          cy={end[1]}
          r={3.5}
          vectorEffect="non-scaling-stroke"
        />
      )}
    </svg>
  );
}
