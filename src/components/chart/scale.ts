/**
 * The chart's scale maths, kept apart from the component so the rules that keep
 * a mark inside its own axis can be tested without rendering anything.
 *
 * The component owns presentation; this owns the arithmetic that decides where a
 * point lands.
 */

export const VIEW_WIDTH = 720;
export const VIEW_HEIGHT = 200;

/** Room for the axis labels and the lowest gridline's own line weight. */
export const PAD = { top: 14, right: 10, bottom: 22, left: 34 };

/**
 * The shape of a plot: how big the drawing surface is and how much of it the
 * data may not use.
 *
 * Three charts share the same arithmetic and differ only in this, so it is a
 * parameter rather than a set of constants baked into one function. The hero
 * chart fills its card with no axis room at all, the panel's sparkline is a
 * short strip, and the detailed chart leaves room for its labels.
 */
export interface ChartGeometry {
  width: number;
  height: number;
  pad: { top: number; right: number; bottom: number; left: number };
}

/** The detailed chart, with room for its axis labels. */
export const DETAIL_GEOMETRY: ChartGeometry = {
  width: VIEW_WIDTH,
  height: VIEW_HEIGHT,
  pad: PAD,
};

/**
 * The hero's background chart.
 *
 * Full bleed: the line runs the whole width of the card and the top of the
 * plot is left clear for the figure that sits over it.
 */
export const HERO_GEOMETRY: ChartGeometry = {
  width: 900,
  height: 520,
  pad: { top: 40, right: 0, bottom: 0, left: 0 },
};

/** The menu-bar panel's strip. */
export const SPARK_GEOMETRY: ChartGeometry = {
  width: 340,
  height: 62,
  pad: { top: 6, right: 0, bottom: 6, left: 0 },
};

/**
 * How much time the chart shows before it starts scrolling.
 *
 * Below this the x-axis stretches a handful of samples across the full width,
 * which makes the line jump as points arrive. Anchoring to real time keeps the
 * scale steady and lets the plot drift left instead.
 */
export const MIN_TIME_SPAN_MS = 30_000;

/**
 * Rounds a rough axis step up to 1, 2, 5 or 10 times a power of ten.
 *
 * The point is stability. Snapping the bounds to a round step means a
 * millisecond of jitter does not move the axis, so a live chart does not
 * rescale itself every poll.
 */
export function niceStep(rough: number): number {
  if (!(rough > 0)) return 1;
  const magnitude = 10 ** Math.floor(Math.log10(rough));
  const normalised = rough / magnitude;
  const step = normalised <= 1 ? 1 : normalised <= 2 ? 2 : normalised <= 5 ? 5 : 10;
  return step * magnitude;
}

export interface ChartScale {
  lowBound: number;
  highBound: number;
  ticks: number[];
  /** Where a time lands, already clamped to the plot's left and right edges. */
  xFor: (at: number) => number;
  /** Where a value lands, already clamped to the plot's top and bottom edges. */
  yFor: (value: number) => number;
  plotTop: number;
  plotBottom: number;
  plotLeft: number;
  plotRight: number;
}

/**
 * A scale for a series that has no readings yet.
 *
 * The bounds are a round 0 to 10 so a tick or two can still be drawn, and the
 * plot box is the geometry's own. Nothing is plotted against it, because
 * nothing has been measured.
 */
function emptyScale(geometry: ChartGeometry): ChartScale {
  const plotWidth = geometry.width - geometry.pad.left - geometry.pad.right;
  const plotHeight = geometry.height - geometry.pad.top - geometry.pad.bottom;
  const plotTop = geometry.pad.top;
  const plotBottom = geometry.pad.top + plotHeight;

  return {
    lowBound: 0,
    highBound: 10,
    ticks: [0, 5, 10],
    xFor: () => geometry.pad.left,
    yFor: () => plotBottom,
    plotTop,
    plotBottom,
    plotLeft: geometry.pad.left,
    plotRight: geometry.pad.left + plotWidth,
  };
}

/**
 * Builds the scale for a series.
 *
 * `points` must be non-empty and every value must be present: the caller filters
 * gaps out before asking, because a gap is drawn as a gap rather than a value.
 *
 * An empty series is answered with a default scale rather than a panic. The
 * chart asks for the scale before it knows whether there is anything to draw,
 * and a screen that opens with no readings yet must render an empty chart
 * rather than crash on the first point.
 *
 * The geometry defaults to the detailed chart, so the common case does not have
 * to name it.
 */
export function buildScale(
  points: Array<{ at: number; value: number }>,
  geometry: ChartGeometry = DETAIL_GEOMETRY,
): ChartScale {
  if (points.length === 0) {
    return emptyScale(geometry);
  }

  const values = points.map((point) => point.value);
  const lowest = Math.min(...values);
  const highest = Math.max(...values);
  const rawSpan = Math.max(highest - lowest, 8);

  // Padded to a round step, so the plot has headroom and the axis only moves when
  // the data genuinely leaves the range. The span is then made an exact number of
  // steps, so every gridline lands on a round value instead of a third of one.
  const step = niceStep((rawSpan * 1.4) / 3);
  let lowBound = Math.floor((lowest - rawSpan * 0.15) / step) * step;
  const highBound = lowBound + Math.ceil((highest + rawSpan * 0.15 - lowBound) / step) * step;

  /*
   * A latency cannot be negative.
   *
   * The padding below the lowest reading can push the axis under zero, and an
   * axis that says "-20 ms" is a number that cannot happen. When every reading
   * is at or above zero, the axis starts at zero.
   */
  if (lowest >= 0 && lowBound < 0) {
    lowBound = 0;
  }

  const ticks: number[] = [];
  for (let value = lowBound; value <= highBound; value += step) {
    ticks.push(value);
  }

  const plotWidth = geometry.width - geometry.pad.left - geometry.pad.right;
  const plotHeight = geometry.height - geometry.pad.top - geometry.pad.bottom;
  const plotTop = geometry.pad.top;
  const plotBottom = geometry.pad.top + plotHeight;

  const firstAt = points[0].at;
  const lastAt = points[points.length - 1].at;
  const timeSpan = Math.max(lastAt - firstAt, MIN_TIME_SPAN_MS);

  const xFor = (at: number) =>
    geometry.pad.left +
    plotWidth * Math.min(Math.max((at - firstAt) / timeSpan, 0), 1);

  /*
   * Clamped to the plot area.
   *
   * The bounds come only from the values already in the series, so a reading that
   * leaves the current range would otherwise be plotted outside the axis, and the
   * pulsing end dot would scale 1.9x from there and leave the chart entirely.
   * Pinning every mark to the plot keeps the chart honest at all times, including
   * the moment before the axis has rescaled to include a new extreme.
   */
  const yFor = (value: number) => {
    const ratio = (value - lowBound) / (highBound - lowBound);
    const y = plotTop + plotHeight - ratio * plotHeight;
    return Math.min(Math.max(y, plotTop), plotBottom);
  };

  return {
    lowBound,
    highBound,
    ticks,
    xFor,
    yFor,
    plotTop,
    plotBottom,
    plotLeft: geometry.pad.left,
    plotRight: geometry.pad.left + plotWidth,
  };
}
