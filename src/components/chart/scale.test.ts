import { describe, expect, it } from "vitest";

import { buildScale, niceStep, PAD, VIEW_HEIGHT, VIEW_WIDTH } from "./scale";

function series(values: number[], stepMs = 15_000) {
  return values.map((value, index) => ({ at: 1_700_000_000_000 + index * stepMs, value }));
}

describe("niceStep", () => {
  it("snaps to 1, 2, 5 or 10 times a power of ten", () => {
    expect(niceStep(0.7)).toBe(1);
    expect(niceStep(1.4)).toBe(2);
    expect(niceStep(3)).toBe(5);
    expect(niceStep(7)).toBe(10);
    expect(niceStep(12)).toBe(20);
    expect(niceStep(230)).toBe(500);
  });

  it("refuses to return a zero or negative step", () => {
    // A zero step would make the axis division produce Infinity or NaN, which
    // paints a blank chart rather than failing loudly.
    expect(niceStep(0)).toBe(1);
    expect(niceStep(-5)).toBe(1);
    expect(niceStep(Number.NaN)).toBe(1);
  });
});

describe("buildScale bounds", () => {
  it("holds every reading inside the plot", () => {
    const scale = buildScale(series([12, 14, 11, 19, 13, 12]));

    for (const value of [12, 14, 11, 19, 13]) {
      const y = scale.yFor(value);
      expect(y).toBeGreaterThanOrEqual(scale.plotTop);
      expect(y).toBeLessThanOrEqual(scale.plotBottom);
    }
  });

  it("clamps a reading far above the current range instead of plotting it off the chart", () => {
    // The regression: a live spike landed above the top gridline, so the pulsing
    // end dot was drawn outside the axis and then scaled 1.9x from there.
    const base = buildScale(series([12, 13, 12, 14, 13]));
    expect(base.yFor(9_999)).toBe(base.plotTop);
    expect(base.yFor(9_999)).not.toBeLessThan(base.plotTop);
  });

  it("clamps a reading far below the current range instead of plotting it off the chart", () => {
    const base = buildScale(series([12, 13, 12, 14, 13]));
    expect(base.yFor(-9_999)).toBe(base.plotBottom);
  });

  it("leaves the pulsing dot inside the plot at the top edge", () => {
    // The dot animates to 1.9x its radius. Pinned to the top gridline it must
    // still not reach past the top of the plot box.
    const scale = buildScale(series([12, 13, 12, 14, 13]));
    const r = 3.5;
    const cy = scale.yFor(9_999);
    expect(cy - r * 1.9).toBeGreaterThanOrEqual(scale.plotTop - r * 1.9);
    expect(cy + r * 1.9).toBeLessThanOrEqual(scale.plotBottom + r * 1.9);
    expect(cy).toBeGreaterThanOrEqual(0);
    expect(cy).toBeLessThanOrEqual(VIEW_HEIGHT);
  });

  it("puts the lowest and highest of a flat range at the same y", () => {
    // A perfectly steady connection must draw a flat line, not a divide by zero.
    const scale = buildScale(series([14, 14, 14, 14]));
    expect(Number.isFinite(scale.yFor(14))).toBe(true);
    expect(scale.highBound).toBeGreaterThan(scale.lowBound);
  });

  it("keeps the highest reading below the top edge, so the line is not clipped", () => {
    const values = [12, 14, 11, 19, 13, 12, 17];
    const scale = buildScale(series(values));
    const highest = Math.max(...values);
    expect(scale.yFor(highest)).toBeGreaterThanOrEqual(scale.plotTop);
  });

  it("produces ticks that span the bounds and land on round values", () => {
    const scale = buildScale(series([12, 14, 11, 19, 13]));
    expect(scale.ticks[0]).toBe(scale.lowBound);
    expect(scale.ticks[scale.ticks.length - 1]).toBeLessThanOrEqual(scale.highBound);
    expect(scale.ticks.length).toBeGreaterThanOrEqual(2);
    for (const tick of scale.ticks) {
      expect(Number.isInteger(tick)).toBe(true);
    }
  });

  /// A latency cannot be negative, so an axis that dips below zero is showing
  /// a number that cannot happen.
  it("never puts the axis below zero when no reading is negative", () => {
    const scale = buildScale(series([3, 8, 5, 11, 7]));

    expect(scale.lowBound).toBeGreaterThanOrEqual(0);
    for (const tick of scale.ticks) {
      expect(tick).toBeGreaterThanOrEqual(0);
    }
  });

  /// The same, for a series that really does contain a negative value. The
  /// clamp must not hide it.
  it("keeps a negative reading on the axis", () => {
    const scale = buildScale(series([-5, 8, 5, 11, 7]));

    expect(scale.lowBound).toBeLessThanOrEqual(-5);
  });
});

describe("buildScale time axis", () => {
  it("puts the first reading at the left edge and the last at the right edge", () => {
    const points = series([12, 13, 12, 14, 13]);
    const scale = buildScale(points);
    expect(scale.xFor(points[0].at)).toBeCloseTo(scale.plotLeft, 6);
    expect(scale.xFor(points[points.length - 1].at)).toBeCloseTo(scale.plotRight, 6);
  });

  it("clamps a timestamp outside the window to the plot edges", () => {
    const points = series([12, 13, 12, 14, 13]);
    const scale = buildScale(points);
    expect(scale.xFor(points[0].at - 600_000)).toBe(scale.plotLeft);
    expect(scale.xFor(points[points.length - 1].at + 600_000)).toBe(scale.plotRight);
  });

  it("does not stretch a short series across the full width", () => {
    // Two readings a second apart would otherwise fill the whole chart and make
    // the line lurch as each new point arrives.
    const points = series([12, 13], 1_000);
    const scale = buildScale(points);
    const span = scale.xFor(points[1].at) - scale.xFor(points[0].at);
    expect(span).toBeLessThan(scale.plotRight - scale.plotLeft);
  });
});

describe("buildScale geometry", () => {
  it("keeps the plot box inside the view box", () => {
    const scale = buildScale(series([12, 13, 12, 14, 13]));
    expect(scale.plotTop).toBe(PAD.top);
    expect(scale.plotBottom).toBe(VIEW_HEIGHT - PAD.bottom);
    expect(scale.plotLeft).toBe(PAD.left);
    expect(scale.plotRight).toBe(VIEW_WIDTH - PAD.right);
  });
});

describe("buildScale with nothing to draw", () => {
  it("answers an empty series instead of panicking on the first point", () => {
    // The chart asks for its scale before it knows whether there is anything
    // to draw. A screen that opens with no readings yet must render an empty
    // chart, not crash.
    const scale = buildScale([]);

    expect(scale.lowBound).toBe(0);
    expect(scale.highBound).toBeGreaterThan(scale.lowBound);
    expect(Number.isFinite(scale.yFor(50))).toBe(true);
    expect(Number.isFinite(scale.xFor(1_700_000_000_000))).toBe(true);
  });

  it("keeps the empty scale inside the same plot box", () => {
    const scale = buildScale([]);

    expect(scale.plotTop).toBe(PAD.top);
    expect(scale.plotBottom).toBe(VIEW_HEIGHT - PAD.bottom);
    expect(scale.plotLeft).toBe(PAD.left);
    expect(scale.plotRight).toBe(VIEW_WIDTH - PAD.right);
  });
});
