import { describe, expect, it } from "vitest";

import { saveClockFormat } from "../../app/clock";
import { formatClock, formatClockMinute } from "./format";

/** A time in the afternoon, so the two clock formats differ. */
const AFTERNOON = new Date(2026, 0, 2, 15, 4, 5).getTime();
const MORNING = new Date(2026, 0, 2, 9, 4, 5).getTime();

describe("formatClock", () => {
  it("writes a twelve-hour clock when that is the choice", () => {
    saveClockFormat("12h");

    const text = formatClock(AFTERNOON);

    expect(text).toMatch(/3:04:05/);
    expect(text).toMatch(/PM/);
  });

  it("writes a twenty-four-hour clock when that is the choice", () => {
    saveClockFormat("24h");

    const text = formatClock(AFTERNOON);

    expect(text).toMatch(/15:04:05/);
    expect(text).not.toMatch(/PM/);
  });

  it("says never rather than showing a time for nothing", () => {
    saveClockFormat("24h");
    expect(formatClock(null)).toBe("Never");
  });
});

describe("formatClockMinute", () => {
  it("writes the hour the way the clock format says", () => {
    saveClockFormat("12h");
    expect(formatClockMinute(MORNING)).toMatch(/9:04/);
    expect(formatClockMinute(MORNING)).toMatch(/AM/);

    saveClockFormat("24h");
    expect(formatClockMinute(MORNING)).toMatch(/09:04/);
    expect(formatClockMinute(MORNING)).not.toMatch(/AM/);
  });
});
