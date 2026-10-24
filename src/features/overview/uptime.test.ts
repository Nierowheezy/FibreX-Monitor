import { describe, expect, it } from "vitest";

import type { UptimeSummary } from "../../types/history";
import { describeUptimeWindow, formatUptimePercent } from "./uptime";

/**
 * The uptime rules, pinned.
 *
 * Two things matter here and both are about restraint: a percentage must never
 * appear without the period it covers, and a rounded figure must never claim a
 * perfect connection. Both are the kind of error that reads as a measurement
 * rather than as a bug.
 */

const NOW = 1_700_000_000_000;

function uptime(overrides: Partial<UptimeSummary> = {}): UptimeSummary {
  return {
    percent: 98.4,
    connected: 492,
    total: 500,
    windowStart: NOW - 3_600_000,
    windowEnd: NOW,
    ...overrides,
  };
}

describe("formatUptimePercent", () => {
  it("keeps one decimal", () => {
    expect(formatUptimePercent(98.42)).toBe("98.4%");
  });

  it("never rounds a real connection up to a perfect one", () => {
    // 99.96 percent of probes connected is not 100 percent, and printing 100.0
    // would erase the handful of failures that actually happened.
    expect(formatUptimePercent(99.96)).toBe("99.9%");
    expect(formatUptimePercent(99.99)).toBe("99.9%");
  });

  it("prints a genuinely perfect record as 100 percent", () => {
    expect(formatUptimePercent(100)).toBe("100.0%");
  });

  it("clamps values above the possible range", () => {
    expect(formatUptimePercent(100.4)).toBe("100.0%");
  });

  it("clamps values below the possible range", () => {
    expect(formatUptimePercent(-1)).toBe("0.0%");
  });

  it("renders zero honestly rather than as an empty string", () => {
    expect(formatUptimePercent(0)).toBe("0.0%");
  });
});

describe("describeUptimeWindow", () => {
  it("describes a window under an hour in minutes", () => {
    expect(describeUptimeWindow(uptime({ windowStart: NOW - 42 * 60_000 }))).toBe(
      "Over the last 42 min",
    );
  });

  it("describes a window under a day in hours", () => {
    expect(describeUptimeWindow(uptime({ windowStart: NOW - 5 * 3_600_000 }))).toBe(
      "Over the last 5 h",
    );
  });

  it("describes a multi-day window in days", () => {
    expect(describeUptimeWindow(uptime({ windowStart: NOW - 3 * 86_400_000 }))).toBe(
      "Over the last 3 days",
    );
  });

  it("does not say one day for a single day's span", () => {
    expect(describeUptimeWindow(uptime({ windowStart: NOW - 86_400_000 }))).toBe(
      "Over the last day",
    );
  });

  it("admits when the period is too short to describe", () => {
    // A single stored sample has a window of zero width. "Over 0 seconds" would
    // be true and useless, so the period is stated as what it is.
    expect(
      describeUptimeWindow(uptime({ windowStart: NOW, windowEnd: NOW, total: 1 })),
    ).toBe("Since the first probe, moments ago");

    expect(
      describeUptimeWindow(uptime({ windowStart: NOW, windowEnd: NOW, total: 6 })),
    ).toBe("Since the first probe, under a minute");
  });

  it("returns null when the window is unknown, so no percentage is shown bare", () => {
    // This is the rule the whole module exists for: an unattached percentage
    // invites the reader to assume a period that was never measured.
    expect(describeUptimeWindow(uptime({ windowStart: null, windowEnd: NOW }))).toBeNull();
    expect(describeUptimeWindow(uptime({ windowStart: NOW, windowEnd: null }))).toBeNull();
  });
});
