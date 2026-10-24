import { describe, expect, it } from "vitest";

import { hasDrawableRun, lastPoint, smoothPath, splitRuns } from "./series";

/** Places points on a fixed grid, so the geometry can be checked exactly. */
const xFor = (at: number) => at * 10;
const yFor = (value: number) => 100 - value;

describe("splitRuns", () => {
  it("keeps a failure as a gap rather than bridging over it", () => {
    // The rule the whole chart rests on: two readings with a failure between
    // them are two runs, never one line drawn across the hole.
    const runs = splitRuns(
      [
        { at: 1, value: 20 },
        { at: 2, value: 22 },
        { at: 3, value: null },
        { at: 4, value: 21 },
        { at: 5, value: 23 },
      ],
      xFor,
      yFor,
    );

    expect(runs).toHaveLength(2);
    expect(runs[0]).toHaveLength(2);
    expect(runs[1]).toHaveLength(2);
  });

  it("carries a trailing failure as the end of the data, not a run", () => {
    const runs = splitRuns(
      [
        { at: 1, value: 20 },
        { at: 2, value: 22 },
        { at: 3, value: null },
      ],
      xFor,
      yFor,
    );

    expect(runs).toHaveLength(1);
    expect(runs[0]).toHaveLength(2);
  });

  it("returns nothing for a series that is entirely gaps", () => {
    const runs = splitRuns([{ at: 1, value: null }], xFor, yFor);

    expect(runs).toHaveLength(0);
  });

  it("places a point at the position the scale gave it", () => {
    const runs = splitRuns([{ at: 3, value: 20 }], xFor, yFor);

    expect(runs[0][0]).toEqual([30, 80]);
  });
});

describe("hasDrawableRun", () => {
  it("refuses a single point, because one point is not a line", () => {
    const runs = splitRuns([{ at: 1, value: 20 }], xFor, yFor);

    expect(hasDrawableRun(runs)).toBe(false);
  });

  it("accepts two points", () => {
    const runs = splitRuns(
      [
        { at: 1, value: 20 },
        { at: 2, value: 21 },
      ],
      xFor,
      yFor,
    );

    expect(hasDrawableRun(runs)).toBe(true);
  });
});

describe("smoothPath", () => {
  it("returns nothing for a run of one, so a lone point is not drawn as a line", () => {
    expect(smoothPath([[10, 20]])).toBe("");
  });

  it("starts at the first point and ends at the last", () => {
    const path = smoothPath([
      [0, 10],
      [20, 30],
    ]);

    expect(path.startsWith("M0,10")).toBe(true);
    expect(path.endsWith("20,30")).toBe(true);
  });

  it("curves through the midpoint of each pair, so the corners are not sharp", () => {
    const path = smoothPath([
      [0, 0],
      [20, 20],
    ]);

    // The control points share the midpoint of the two ends, which is what
    // flattens the corner.
    expect(path).toContain("C10,0 10,20");
  });
});

describe("lastPoint", () => {
  it("names the end of the last run", () => {
    const runs = splitRuns(
      [
        { at: 1, value: 20 },
        { at: 2, value: 22 },
      ],
      xFor,
      yFor,
    );

    expect(lastPoint(runs)).toEqual([20, 78]);
  });

  it("is null when there is nothing to point at", () => {
    expect(lastPoint([])).toBeNull();
  });
});
