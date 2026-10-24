import { describe, expect, it } from "vitest";

import type { IncidentSummary } from "../../types/history";
import {
  describeIncident,
  describeRouter,
  durationOf,
  formatDuration,
  isOpen,
  summariseIncidents,
} from "./format";

/**
 * The incident formatters decide how a recorded outage is described, which is
 * the part of this feature a reader actually sees. A wrong duration or a router
 * claim that was never measured is the difference between evidence and
 * invention, so the rules are pinned here rather than checked by eye.
 */

const NOW = 1_700_000_000_000;

function incident(overrides: Partial<IncidentSummary> = {}): IncidentSummary {
  return {
    id: 1,
    targetId: 1,
    startedAt: NOW - 10_000,
    recoveredAt: NOW - 5_000,
    probeCount: 3,
    routerReachable: null,
    ...overrides,
  };
}

describe("formatDuration", () => {
  it("keeps a decimal below ten seconds", () => {
    // An outage of 2.4 s and one of 2.0 s are different when a line flickers.
    expect(formatDuration(2_400)).toBe("2.4s");
    expect(formatDuration(2_000)).toBe("2.0s");
  });

  it("drops the decimal once a duration is in the tens of seconds", () => {
    expect(formatDuration(42_000)).toBe("42s");
  });

  it("switches to minutes at sixty seconds", () => {
    expect(formatDuration(60_000)).toBe("1m");
    expect(formatDuration(90_000)).toBe("1m 30s");
  });

  it("switches to hours past sixty minutes", () => {
    expect(formatDuration(3_600_000)).toBe("1h");
    expect(formatDuration(5_400_000)).toBe("1h 30m");
  });

  it("never prints a negative duration", () => {
    // A clock that moved backwards is not an outage that lasted minus five
    // seconds, and printing one would be a number that cannot have happened.
    expect(formatDuration(-1)).toBe("Unknown");
  });

  it("treats a zero duration as real rather than unknown", () => {
    expect(formatDuration(0)).toBe("0.0s");
  });
});

describe("durationOf", () => {
  it("measures a closed incident to its recovery, not to now", () => {
    const closed = incident({ startedAt: NOW - 10_000, recoveredAt: NOW - 5_000 });

    expect(durationOf(closed, NOW)).toBe(5_000);
    // Moving the clock forward must not extend a finished outage.
    expect(durationOf(closed, NOW + 60_000)).toBe(5_000);
  });

  it("measures an open incident against now", () => {
    const open = incident({ startedAt: NOW - 10_000, recoveredAt: null });

    expect(durationOf(open, NOW)).toBe(10_000);
    expect(durationOf(open, NOW + 5_000)).toBe(15_000);
  });
});

describe("isOpen", () => {
  it("distinguishes a running outage from a recovered one", () => {
    expect(isOpen(incident({ recoveredAt: null }))).toBe(true);
    expect(isOpen(incident({ recoveredAt: NOW }))).toBe(false);
  });
});

describe("describeRouter", () => {
  it("reports the router staying up as its own fact", () => {
    expect(describeRouter(incident({ routerReachable: true }))).toBe("Router stayed up");
  });

  it("reports the router being down as its own fact", () => {
    expect(describeRouter(incident({ routerReachable: false }))).toBe(
      "Router also unreachable",
    );
  });

  it("never reports an unchecked router as unreachable", () => {
    // This is the one distinction the whole field exists for. A router that was
    // never probed is not a router that was down, and claiming otherwise would
    // be a fabricated diagnosis of the user's own equipment.
    const unchecked = describeRouter(incident({ routerReachable: null }));

    expect(unchecked).toBe("Router not checked");
    expect(unchecked).not.toContain("unreachable");
  });
});

describe("describeIncident", () => {
  it("says a closed outage lasted a period", () => {
    const closed = incident({ startedAt: NOW - 10_000, recoveredAt: NOW - 5_000 });

    expect(describeIncident(closed, NOW)).toBe("Lasted 5.0s");
  });

  it("says an open outage is still running", () => {
    const open = incident({ startedAt: NOW - 4_000, recoveredAt: null });

    expect(describeIncident(open, NOW)).toBe("Failing for 4.0s and counting");
  });

  it("never names a cause", () => {
    const text = describeIncident(incident(), NOW).toLowerCase();

    // The app measures that a connection did not open. It does not measure why,
    // so no description may imply a culprit.
    for (const word of ["mtn", "isp", "provider", "fault", "because", "congestion"]) {
      expect(text).not.toContain(word);
    }
  });
});

describe("summariseIncidents", () => {
  it("returns null for an empty list rather than a total of zero", () => {
    // Zero outages and nothing recorded are different claims, and only one of
    // them is supported by an empty database.
    expect(summariseIncidents([], NOW)).toBeNull();
  });

  it("totals and finds the longest across several incidents", () => {
    const summary = summariseIncidents(
      [
        incident({ id: 1, startedAt: NOW - 10_000, recoveredAt: NOW - 8_000 }),
        incident({ id: 2, startedAt: NOW - 30_000, recoveredAt: NOW - 10_000 }),
      ],
      NOW,
    );

    expect(summary).not.toBeNull();
    expect(summary?.count).toBe(2);
    expect(summary?.totalMs).toBe(2_000 + 20_000);
    expect(summary?.longestMs).toBe(20_000);
    expect(summary?.open).toBe(0);
  });

  it("counts an ongoing outage and measures it against now", () => {
    const summary = summariseIncidents(
      [
        incident({ id: 1, startedAt: NOW - 10_000, recoveredAt: NOW - 8_000 }),
        incident({ id: 2, startedAt: NOW - 6_000, recoveredAt: null }),
      ],
      NOW,
    );

    expect(summary?.open).toBe(1);
    expect(summary?.count).toBe(2);
    expect(summary?.longestMs).toBe(6_000);
  });
});
