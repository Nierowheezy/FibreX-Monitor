import { buildScale, SPARK_GEOMETRY } from "./scale";
import { hasDrawableRun, smoothPath, splitRuns } from "./series";
import type { PlotPoint } from "./series";

/**
 * The menu-bar panel's sparkline.
 *
 * A short strip under the headline reading, so someone glancing at the popover
 * can see the shape of the connection and not just its current value. It has
 * no axis and no labels: at 340 pixels wide there is no room for them, and a
 * number repeated is not a second reading.
 *
 * Gaps are drawn as gaps, on the same rule as the dashboard chart. A failure
 * that is bridged over would be a claim the app never made.
 */
export default function Sparkline({
  points,
  gradientId = "panel-wash",
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
    SPARK_GEOMETRY,
  );

  const runs = splitRuns(points, scale.xFor, scale.yFor);

  if (!hasDrawableRun(runs)) {
    return null;
  }

  const baseline = scale.plotBottom;
  const middle = scale.plotTop + (scale.plotBottom - scale.plotTop) / 2;

  return (
    <svg
      viewBox={`0 0 ${SPARK_GEOMETRY.width} ${SPARK_GEOMETRY.height}`}
      preserveAspectRatio="none"
      aria-hidden="true"
      // Absolutely placed, so the view box's intrinsic width does not become
      // the minimum width of whatever holds it. A chart that decides the width
      // of the panel is a chart that pushes the panel's content off the edge.
      className="absolute inset-0 block size-full"
    >
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--chart-area-top)" />
          <stop offset="100%" stopColor="var(--chart-area-bottom)" />
        </linearGradient>
      </defs>

      <line
        className="stroke-[var(--chart-grid)]"
        x1={scale.plotLeft}
        y1={middle}
        x2={scale.plotRight}
        y2={middle}
        strokeWidth={1}
        vectorEffect="non-scaling-stroke"
      />

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
    </svg>
  );
}
