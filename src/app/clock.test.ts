import { describe, expect, it } from "vitest";

import { clockFormatLabel, saveClockFormat, usesTwelveHours } from "./clock";

describe("usesTwelveHours", () => {
  it("follows the choice when one is made", () => {
    expect(usesTwelveHours("12h")).toBe(true);
    expect(usesTwelveHours("24h")).toBe(false);
  });

  it("follows the system when the choice is the system", () => {
    // The system answer is the environment's own, so the test reads it rather
    // than assuming one.
    const expected =
      new Intl.DateTimeFormat().resolvedOptions().hour12 === true;
    expect(usesTwelveHours("system")).toBe(expected);
  });
});

describe("the format in force", () => {
  it("is what the last choice set", () => {
    // The formatters read the format from here rather than from React, so a
    // component cannot forget to pass it down and quietly get the wrong clock.
    saveClockFormat("12h");
    expect(usesTwelveHours()).toBe(true);

    saveClockFormat("24h");
    expect(usesTwelveHours()).toBe(false);
  });
});

describe("clockFormatLabel", () => {
  it("names each option the way the control shows it", () => {
    expect(clockFormatLabel("system")).toBe("System");
    expect(clockFormatLabel("12h")).toBe("12-hour");
    expect(clockFormatLabel("24h")).toBe("24-hour");
  });
});
