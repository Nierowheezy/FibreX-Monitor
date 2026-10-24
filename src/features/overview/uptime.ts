import type { UptimeSummary } from "../../types/history";

/**
 * How uptime is stated.
 *
 * The rule this file exists to enforce: a percentage never appears without the
 * period it covers. "99.2 percent uptime" with no window is not a measurement,
 * it is a number that invites the reader to assume whatever period they had in
 * mind. The stored history may span minutes or weeks, so the window comes from
 * the data rather than from a label someone typed.
 */

/** One decimal, because a live connection's uptime moves in small steps. */
export function formatUptimePercent(percent: number): string {
  // Clamped at both ends: a rounding artefact must never print 100.0 percent of
  // probes connected when some of them failed, and no probe count can produce
  // more connections than attempts.
  const bounded = Math.min(100, Math.max(0, percent));
  const rounded = Number(bounded.toFixed(1));

  // The one correction that matters. A connection that failed even once is not
  // at 100 percent, and one decimal place would round 99.96 up to a figure that
  // reads as a perfect record. Floored just below the ceiling instead, so the
  // display stays honest about the failures that did happen.
  if (rounded >= 100 && bounded < 100) return "99.9%";

  return `${rounded.toFixed(1)}%`;
}

/**
 * The period the uptime figure covers, in words.
 *
 * Returns null when the window is unknown, so the caller can decline to show a
 * percentage rather than pair it with a period it does not have.
 */
export function describeUptimeWindow(uptime: UptimeSummary): string | null {
  const { windowStart, windowEnd } = uptime;

  if (windowStart === null || windowEnd === null) return null;

  const spanMs = windowEnd - windowStart;

  // A single stored sample has a window of zero width. Saying "over 0 seconds"
  // would be true but useless; the honest statement is that the period is too
  // short to describe.
  if (spanMs < 60_000) {
    return uptime.total === 1
      ? "Since the first probe, moments ago"
      : "Since the first probe, under a minute";
  }

  const minutes = Math.floor(spanMs / 60_000);
  if (minutes < 60) return `Over the last ${minutes} min`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Over the last ${hours} h`;

  const days = Math.floor(hours / 24);
  return days === 1 ? "Over the last day" : `Over the last ${days} days`;
}
