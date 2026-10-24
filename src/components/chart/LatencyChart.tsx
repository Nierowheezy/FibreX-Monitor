import { useEffect, useRef } from "react";

import type { LatencyPoint } from "../../types/latency";
import { buildScale, VIEW_HEIGHT, VIEW_WIDTH } from "./scale";
import { hasDrawableRun, lastPoint, smoothPath, splitRuns } from "./series";

/**
 * A market-style latency chart drawn to SVG by hand.
 *
 * No charting library, deliberately: the wash, the draw-in animation and the
 * explicit gaps are the design, and a library would have to be fought to produce
 * them. The trade is that axis and tick logic is ours, so that arithmetic lives
 * in ./scale where it is unit tested rather than only observed.
 */
export default function LatencyChart({ points }: { points: LatencyPoint[] }) {
  const line = useRef<SVGPathElement>(null);
  const revealed = useRef(false);

  const present = points.filter((point) => point.value !== null);

  useEffect(() => {
    const path = line.current;
    // Only the first reveal animates. Re-running it on every appended point
    // would restart a one-second draw-in each time a reading landed, which on a
    // live chart is a permanent flicker rather than an introduction.
    if (!path || revealed.current) {
      return;
    }

    const length = path.getTotalLength();
    path.style.strokeDasharray = String(length);
    path.style.strokeDashoffset = String(length);
    revealed.current = true;

    const frame = requestAnimationFrame(() => {
      path.style.transition = "stroke-dashoffset 1100ms cubic-bezier(0.2, 0.8, 0.2, 1)";
      path.style.strokeDashoffset = "0";
    });

    return () => cancelAnimationFrame(frame);
  }, [present.length]);

  const scale = buildScale(
    present.map((point) => ({ at: point.at, value: point.value as number })),
  );
  const { ticks, xFor, yFor } = scale;
  const baseline = scale.plotBottom;
  const rightEdge = scale.plotRight;

  // Split into runs of present values, so a gap stays a gap instead of being
  // bridged. The plan forbids interpolating continuity we never measured.
  const runs = splitRuns(points, scale.xFor, scale.yFor);

  // A run of one point is not a line. Rendering an axis with nothing on it reads
  // as a broken chart, which is exactly what a user sees after the loop pauses
  // having measured once, so that case gets the empty state instead.
  if (!hasDrawableRun(runs)) {
    return (
      <div className="hairline flex h-[200px] flex-col items-center justify-center gap-3 rounded-control border-dashed text-label tracking-label text-ink-faint">
        <span>
          {present.length === 0
            ? "No readings to plot yet"
            : "One reading so far"}
        </span>
        <span className="font-mono text-micro">
          {present.length === 0
            ? "waiting for the first probe to succeed"
            : "a line needs two successful probes"}
        </span>
      </div>
    );
  }

  const path = (segment: Array<[number, number]>) => smoothPath(segment);

  const end = lastPoint(runs);
  const endOfData = end ?? [0, 0];

  // A newest probe that failed leaves the line ending short of the right edge.
  // Drawing that tail makes the reason visible instead of letting a stale dot
  // sit at the last good reading as though it were current.
  const trailingGap = points[points.length - 1].value === null;
  const gapXs = points
    .map((point) => xFor(point.at))
    .filter((x) => x > endOfData[0]);

  return (
    <svg
      viewBox={`0 0 ${VIEW_WIDTH} ${VIEW_HEIGHT}`}
      role="img"
      aria-label={`Latency over ${present.length} readings, lowest ${Math.round(Math.min(...present.map((p) => p.value as number)))} milliseconds, highest ${Math.round(Math.max(...present.map((p) => p.value as number)))} milliseconds${trailingGap ? ", most recent probes failed" : ""}`}
      // `min-w-0` and the hidden overflow keep the view box's intrinsic width
      // from setting a minimum on whatever holds the chart. A chart that
      // decides the width of its card is a chart that pushes the card's
      // content past the edge.
      className="block h-auto w-full min-w-0 overflow-hidden"
    >
      <defs>
        <linearGradient id="latency-wash" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--chart-area-top)" />
          <stop offset="100%" stopColor="var(--chart-area-bottom)" />
        </linearGradient>
      </defs>

      {ticks.map((value) => {
        const y = yFor(value);
        return (
          <g key={value}>
            <line
              className="stroke-[var(--chart-grid)]"
              x1={scale.plotLeft}
              y1={y}
              x2={scale.plotRight}
              y2={y}
              strokeWidth={1}
            />
            <text
              className="fill-ink-faint font-mono text-[10px]"
              x={0}
              y={y + 3}
            >
              {Math.round(value)}
            </text>
          </g>
        );
      })}

      {runs.map((segment, index) =>
        segment.length > 1 ? (
          <path
            key={`area-${index}`}
            className="animate-[chart-wash_900ms_var(--ease)_380ms_both]"
            d={`${path(segment)} L${segment[segment.length - 1][0]},${baseline} L${segment[0][0]},${baseline} Z`}
            fill="url(#latency-wash)"
            stroke="none"
          />
        ) : null,
      )}

      {runs.slice(0, -1).map((segment, index) => {
        const end = segment[segment.length - 1];
        const nextStart = runs[index + 1][0];
        return (
          <line
            key={`gap-${index}`}
            className="stroke-ink-faint"
            x1={end[0]}
            y1={end[1]}
            x2={nextStart[0]}
            y2={nextStart[1]}
            strokeWidth={2}
            strokeDasharray="3 5"
          />
        );
      })}

      {runs.map((segment, index) =>
        segment.length > 1 ? (
          <path
            key={`line-${index}`}
            ref={index === 0 ? line : undefined}
            d={path(segment)}
            fill="none"
            stroke="var(--chart-line)"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        ) : null,
      )}

      {trailingGap && (
        <line
          className="stroke-ink-faint"
          x1={endOfData[0]}
          y1={endOfData[1]}
          x2={gapXs[gapXs.length - 1] ?? rightEdge}
          y2={baseline}
          strokeWidth={2}
          strokeDasharray="3 5"
        />
      )}

      {!trailingGap && (
        <circle
          className="fill-[var(--chart-dot)] origin-center animate-[chart-pulse_2.4s_ease-in-out_infinite]"
          cx={endOfData[0]}
          cy={endOfData[1]}
          r={3.5}
        />
      )}
    </svg>
  );
}
