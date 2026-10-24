import type { RouterReport, WanAddressKind } from "../../types/overview";

/**
 * How a router-reported figure is written.
 *
 * Kept beside the overview formatters rather than inside the screen, so the same
 * rules apply wherever router figures appear, and so the arithmetic below can be
 * tested without a DOM.
 */

/**
 * The optical receive power, in dBm.
 *
 * Always signed, because the sign is the reading: a GPON receiver sits at a
 * negative dBm, and dropping the minus would turn a healthy -18.8 into an
 * impossible +18.8. Two decimals because the firmware reports two, and they are
 * what a slowly drifting splice looks like over days.
 */
export function formatOpticalDbm(value: number): string {
  return value.toFixed(2);
}

/** A router utilisation percentage. Whole numbers: the firmware reports no more. */
export function formatPercent(value: number): string {
  return String(Math.round(value));
}

/**
 * A throughput rate, in the unit a reader would use.
 *
 * Bits per second, because that is what a connection is sold in. A rate of
 * zero is a real reading: the router's counters did not move, which is what an
 * idle line looks like, and it is different from having no rate at all.
 */
export function formatRate(bytesPerSecond: number | null): string {
  if (bytesPerSecond === null) return "No reading";

  const bitsPerSecond = bytesPerSecond * 8;

  if (bitsPerSecond >= 1_000_000) {
    return `${(bitsPerSecond / 1_000_000).toFixed(1)} Mbps`;
  }
  if (bitsPerSecond >= 1_000) {
    return `${(bitsPerSecond / 1_000).toFixed(0)} Kbps`;
  }
  return `${Math.round(bitsPerSecond)} bps`;
}

/**
 * A byte total, in the unit a reader would use.
 *
 * Decimal units, because that is how a router's counter is usually read and
 * because a total is a quantity of storage rather than a rate. A total of zero
 * is a real reading: no bytes have passed since the session began.
 */
export function formatBytes(bytes: number | null): string {
  if (bytes === null) return "No reading";

  if (bytes >= 1_000_000_000_000) {
    return `${(bytes / 1_000_000_000_000).toFixed(2)} TB`;
  }
  if (bytes >= 1_000_000_000) {
    return `${(bytes / 1_000_000_000).toFixed(2)} GB`;
  }
  if (bytes >= 1_000_000) {
    return `${(bytes / 1_000_000).toFixed(1)} MB`;
  }
  if (bytes >= 1_000) {
    return `${(bytes / 1_000).toFixed(1)} KB`;
  }
  return `${Math.round(bytes)} B`;
}

/**
 * How the router's WAN address is described.
 *
 * The words carry the claim. "Private" on a WAN interface means the carrier is
 * NATing as well as the router, which is a different situation from a directly
 * assigned public address, and a person debugging remote access needs to see
 * which one they have.
 */
export const ADDRESS_KIND_LABEL: Record<WanAddressKind, string> = {
  public: "Public address",
  carrierNat: "Carrier-grade NAT",
  private: "Carrier-side private address",
  local: "Local address",
  unknown: "Unrecognised address",
};

/**
 * How long the WAN session has been up, in words.
 *
 * A long up time is evidence the line has not dropped, so it is written out
 * rather than rounded to a count of minutes nobody would read. Null when the
 * router reported no up time, so a caller renders its own empty rather than
 * "0s", which would claim a session that had just started.
 */
export function formatDuration(seconds: number | null): string | null {
  if (seconds === null || seconds < 0) return null;

  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);

  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}

/**
 * One sentence for the whole router report, for the session card.
 *
 * Ordered by what the reader needs first: whether a reading exists, and if not,
 * why. An unconfigured app says so plainly rather than reporting a failure it
 * never had, because the fix for the two is completely different.
 */
export function describeRouterReport(report: RouterReport): string {
  if (report.snapshot !== null) {
    const host = report.snapshot.auth.host;
    return `Signed in to ${host} and read ${report.snapshot.sources.filter((source) => source.ok).length} of ${report.snapshot.sources.length} pages.`;
  }

  if (!report.configured) {
    return "No router credentials are configured, so the router is not being read. Set the FIBREX_ROUTER_* variables to enable it.";
  }

  if (!report.attempted) {
    return "Waiting for the first router read.";
  }

  return report.error?.message ?? "The router could not be read.";
}

/**
 * Which router page a screen should name when a specific figure is missing.
 *
 * Returns null when every attempted page answered, so a caller only explains a
 * gap that actually happened.
 */
export function firstFailedSource(report: RouterReport): string | null {
  if (report.snapshot === null) return null;
  return report.snapshot.sources.find((source) => !source.ok)?.name ?? null;
}
