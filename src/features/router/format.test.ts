import { describe, expect, it } from "vitest";

import type { RouterReport, RouterSnapshot } from "../../types/overview";
import {
  ADDRESS_KIND_LABEL,
  describeRouterReport,
  firstFailedSource,
  formatBytes,
  formatDuration,
  formatOpticalDbm,
  formatPercent,
  formatRate,
} from "./format";

function snapshot(overrides: Partial<RouterSnapshot> = {}): RouterSnapshot {
  return {
    readAt: 1_700_000_000_000,
    auth: { host: "192.168.100.1", port: 80, username: "root", hasPassword: true },
    wan: {
      status: "Connected",
      ipAddress: "10.54.23.105",
      addressKind: "private",
      gateway: "10.54.0.1",
      dnsServers: ["102.88.158.7"],
      mac: null,
      mtu: 1492,
      bras: null,
      accessType: null,
      serviceList: null,
      uptimeSeconds: 457,
    },
    health: {
      opticalRxDbm: { value: -18.79, state: "measured", measuredAt: 1_700_000_000_000 },
      cpuPercent: { value: 30, state: "measured", measuredAt: 1_700_000_000_000 },
      memoryPercent: { value: 67, state: "measured", measuredAt: 1_700_000_000_000 },
      ponMode: "gpon",
      reportsOptical: true,
      reportsUtilisation: true,
    },
    counters: null,
    network: {
      wifiRadios: [],
      wanAccessType: "GPON",
      ipv6Status: "Invalid",
      supportsIpv6: true,
      ipv6Prefixes: [],
    },
    sources: [
      { name: "Device information", path: "/device", ok: true, error: null },
      { name: "WAN connection", path: "/wan", ok: true, error: null },
    ],
    ...overrides,
  };
}

function report(overrides: Partial<RouterReport> = {}): RouterReport {
  return {
    snapshot: null,
    error: null,
    configured: false,
    attempted: false,
    traffic: null,
    ...overrides,
  };
}

describe("formatOpticalDbm", () => {
  it("keeps the sign, because the sign is the reading", () => {
    expect(formatOpticalDbm(-18.79)).toBe("-18.79");
    expect(formatOpticalDbm(-18.79).startsWith("-")).toBe(true);
  });

  it("keeps two decimals, which is what the firmware reports", () => {
    expect(formatOpticalDbm(-8)).toBe("-8.00");
  });

  it("never writes a positive figure as a positive dBm silently", () => {
    // A positive receive power cannot happen on this hardware. If one ever
    // reaches here it is shown as it arrived, sign and all, rather than being
    // quietly flipped into a plausible-looking negative.
    expect(formatOpticalDbm(3.5)).toBe("3.50");
  });
});

describe("formatPercent", () => {
  it("rounds to whole numbers, because the firmware reports no more", () => {
    expect(formatPercent(30)).toBe("30");
    expect(formatPercent(66.6)).toBe("67");
  });
});

describe("ADDRESS_KIND_LABEL", () => {
  it("never calls the observed carrier-side address public", () => {
    // The live connection reports 10.54.23.105, which is RFC1918 private space.
    // Presenting it as a public address would be the exact fabrication this app
    // exists to avoid.
    expect(ADDRESS_KIND_LABEL.private).not.toMatch(/public/i);
    expect(ADDRESS_KIND_LABEL.private).toMatch(/private/i);
  });

  it("has a label for every kind the backend can send", () => {
    expect(Object.values(ADDRESS_KIND_LABEL).every((label) => label.length > 0)).toBe(true);
  });
});

describe("formatDuration", () => {
  it("returns null for an unreported up time rather than 0s", () => {
    // A session that has run for no seconds and a session whose up time the
    // router did not report are different facts.
    expect(formatDuration(null)).toBeNull();
  });

  it("reads seconds, minutes, hours and days", () => {
    expect(formatDuration(45)).toBe("45s");
    expect(formatDuration(457)).toBe("7m");
    expect(formatDuration(7_200)).toBe("2h 0m");
    expect(formatDuration(90_000)).toBe("1d 1h");
  });

  it("treats a negative up time as no reading", () => {
    expect(formatDuration(-1)).toBeNull();
  });
});

describe("formatRate", () => {
  it("reads a rate in bits per second, because that is how a line is sold", () => {
    // 125000 bytes a second is 1000000 bits a second.
    expect(formatRate(125_000)).toBe("1.0 Mbps");
  });

  it("drops to kilobits for a slow rate", () => {
    expect(formatRate(1_250)).toBe("10 Kbps");
  });

  it("reads a very slow rate in bits", () => {
    expect(formatRate(10)).toBe("80 bps");
  });

  it("shows a rate of zero as zero, because that is a real reading", () => {
    // The counters did not move. That is what an idle line looks like, and it
    // is different from having no rate at all.
    expect(formatRate(0)).toBe("0 bps");
  });

  it("says so when there is no rate", () => {
    expect(formatRate(null)).toBe("No reading");
  });
});

describe("formatBytes", () => {
  it("reads a total in the unit a reader would use", () => {
    expect(formatBytes(1_500_000_000)).toBe("1.50 GB");
    expect(formatBytes(1_500_000)).toBe("1.5 MB");
    expect(formatBytes(1_500)).toBe("1.5 KB");
    expect(formatBytes(900)).toBe("900 B");
  });

  it("reads a terabyte total", () => {
    expect(formatBytes(2_000_000_000_000)).toBe("2.00 TB");
  });

  it("shows a total of zero as zero, because that is a real reading", () => {
    expect(formatBytes(0)).toBe("0 B");
  });

  it("says so when there is no total", () => {
    expect(formatBytes(null)).toBe("No reading");
  });
});

describe("describeRouterReport", () => {
  it("says a router with no credentials is unconfigured, not broken", () => {
    const sentence = describeRouterReport(report());
    expect(sentence).toMatch(/no router credentials/i);
    expect(sentence).not.toMatch(/could not be read/i);
  });

  it("says waiting rather than failed before the first attempt", () => {
    const sentence = describeRouterReport(report({ configured: true }));
    expect(sentence).toMatch(/waiting/i);
  });

  it("repeats the backend's reason when a read failed", () => {
    const sentence = describeRouterReport(
      report({
        configured: true,
        attempted: true,
        error: { kind: "unreachable", message: "Could not reach the router at 192.168.100.1." },
      }),
    );
    expect(sentence).toBe("Could not reach the router at 192.168.100.1.");
  });

  it("reports how many pages were read when there is a snapshot", () => {
    const sentence = describeRouterReport(
      report({ configured: true, attempted: true, snapshot: snapshot() }),
    );
    expect(sentence).toContain("192.168.100.1");
    expect(sentence).toContain("2 of 2");
  });

  it("counts only the pages that actually answered", () => {
    const partial = snapshot({
      sources: [
        { name: "Device information", path: "/device", ok: true, error: null },
        {
          name: "WAN connection",
          path: "/wan",
          ok: false,
          error: { kind: "unexpectedResponse", message: "no data" },
        },
      ],
    });
    expect(
      describeRouterReport(report({ configured: true, attempted: true, snapshot: partial })),
    ).toContain("1 of 2");
  });
});

describe("firstFailedSource", () => {
  it("returns null when every page answered", () => {
    expect(firstFailedSource(report({ snapshot: snapshot() }))).toBeNull();
  });

  it("returns null when there is no snapshot to describe", () => {
    expect(firstFailedSource(report())).toBeNull();
  });

  it("names the first page that did not answer", () => {
    const partial = snapshot({
      sources: [
        { name: "Device information", path: "/device", ok: true, error: null },
        {
          name: "WAN connection",
          path: "/wan",
          ok: false,
          error: { kind: "unexpectedResponse", message: "no data" },
        },
      ],
    });
    expect(firstFailedSource(report({ snapshot: partial }))).toBe("WAN connection");
  });
});
