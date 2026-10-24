/**
 * The geometry every chart in the app shares.
 *
 * Three charts draw the same thing at three sizes: the detailed chart on the
 * overview, the hero's background chart, and the menu-bar panel's strip. The
 * arithmetic that decides where a point lands and how a line runs through them
 * lives here once, so the three cannot drift apart.
 */

/** One point in a series. A null value is a gap, not a zero. */
export interface PlotPoint {
  at: number;
  value: number | null;
}

/** A run of consecutive present points, already placed on the plot. */
export type PlotRun = Array<[number, number]>;

/**
 * Splits a series into runs of present values.
 *
 * A gap stays a gap. Two points with a failure between them are two runs, not
 * one line with a hole bridged across it, because the plan forbids drawing
 * continuity that was never measured.
 */
export function splitRuns(
  points: PlotPoint[],
  xFor: (at: number) => number,
  yFor: (value: number) => number,
): PlotRun[] {
  const runs: PlotRun[] = [];
  let run: PlotRun = [];

  for (const point of points) {
    if (point.value === null) {
      if (run.length > 0) {
        runs.push(run);
        run = [];
      }
      continue;
    }
    run.push([xFor(point.at), yFor(point.value)]);
  }

  if (run.length > 0) {
    runs.push(run);
  }

  return runs;
}

/**
 * The smoothed path through a run.
 *
 * A straight line between every pair of points reads as a seismograph, so each
 * segment is drawn as a curve whose control points share the midpoint of the
 * two ends. That flattens the corners without inventing a peak the readings
 * did not have.
 *
 * A run of one point has no path. It returns an empty string rather than a
 * path with a single move, so the caller can skip it.
 */
export function smoothPath(segment: PlotRun): string {
  if (segment.length < 2) {
    return "";
  }

  let d = `M${segment[0][0]},${segment[0][1]}`;
  for (let index = 0; index < segment.length - 1; index += 1) {
    const [x0, y0] = segment[index];
    const [x1, y1] = segment[index + 1];
    const midX = (x0 + x1) / 2;
    d += ` C${midX},${y0} ${midX},${y1} ${x1},${y1}`;
  }
  return d;
}

/**
 * The end of the last run, which is where the current reading is.
 *
 * Null when there is nothing to point at, so a caller cannot accidentally draw
 * a dot at the origin.
 */
export function lastPoint(runs: PlotRun[]): [number, number] | null {
  const last = runs[runs.length - 1];
  if (!last || last.length === 0) {
    return null;
  }
  return last[last.length - 1];
}

/**
 * Whether a series is worth drawing.
 *
 * One point is not a line. A chart with a single reading on it reads as
 * broken, which is exactly what someone sees after the loop has measured
 * once, so that case gets an empty state instead.
 */
export function hasDrawableRun(runs: PlotRun[]): boolean {
  return runs.some((run) => run.length > 1);
}
