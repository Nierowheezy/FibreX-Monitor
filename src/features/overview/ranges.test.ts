import { describe, expect, it } from "vitest";

import { DEFAULT_RANGE_INDEX, HISTORY_RANGES, rangeAt } from "./ranges";

describe("the history ranges", () => {
  it("starts on the shortest range, which is the live picture", () => {
    expect(HISTORY_RANGES[DEFAULT_RANGE_INDEX].label).toBe("5 min");
  });

  it("runs from the shortest to the longest", () => {
    const windows = HISTORY_RANGES.map((range) => range.windowMs);
    for (let index = 1; index < windows.length; index += 1) {
      expect(windows[index]).toBeGreaterThan(windows[index - 1]);
    }
  });

  it("every range is long enough to hold a reading", () => {
    for (const range of HISTORY_RANGES) {
      expect(range.windowMs).toBeGreaterThan(0);
    }
  });
});

describe("rangeAt", () => {
  it("clamps an index past the end rather than wrapping", () => {
    // The control is a pair of arrows, so stepping past the end has to stop.
    // Wrapping would silently jump the reader to the other end of the list.
    expect(rangeAt(HISTORY_RANGES.length + 5).label).toBe(
      HISTORY_RANGES[HISTORY_RANGES.length - 1].label,
    );
  });

  it("clamps a negative index to the first range", () => {
    expect(rangeAt(-3).label).toBe(HISTORY_RANGES[0].label);
  });

  it("returns the range that was asked for", () => {
    expect(rangeAt(2).label).toBe(HISTORY_RANGES[2].label);
  });
});
