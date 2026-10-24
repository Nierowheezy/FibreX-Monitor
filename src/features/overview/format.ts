import { usesTwelveHours } from "../../app/clock";
import type { ProbeFailureBasis, ValueState } from "../../types/overview";

/**
 * How a measurement is written, in one place.
 *
 * The dashboard tiles and the menu-bar panel show the same four numbers, so
 * their formatting has to be the same code. Two copies would drift, and a panel
 * quietly rounding differently from the dashboard is the kind of discrepancy
 * that destroys trust in both.
 */

export function formatLatency(value: number): string {
  // One decimal below 10 ms, whole numbers above. A router connect of 1.4 ms is
  // a different reading from 1 ms, and rounding it to 1 would throw away the
  // only digit that was interesting; once a value is in the tens, the decimal is
  // noise on a live number that changes every probe.
  return value < 10 ? value.toFixed(1) : String(Math.round(value));
}

/**
 * The counts behind the failure figure, in the words both surfaces must show.
 *
 * There is deliberately no formatter for a bare percentage. The wire field is
 * named `packetLoss`, but that name is historical: this app measures TCP
 * connects, so a lone "12.5" with a percent sign would be a packet-loss claim
 * it never took. The counts are the same fact stated honestly.
 *
 * `_value` is unused on purpose. It is the same information as the ratio of the
 * two counts, and taking the text from the counts keeps the counting rule in
 * one place instead of letting a display round its own way to a different
 * number.
 *
 * Returns `null` rather than some stand-in text when the counts are missing, so
 * that a caller cannot render a half-figure with nothing behind it. The tile
 * turns `null` into its own "No reading".
 */
export function formatProbeFailures(
  _value: number,
  basis: ProbeFailureBasis | null,
): string | null {
  // The counts travel with the figure by contract, so this branch should be
  // unreachable. If it ever is reached, there is no figure here the app can
  // vouch for.
  if (basis === null || basis.total === 0) {
    return null;
  }

  return `${basis.failed} of ${basis.total}`;
}

/**
 * How old a reading is, in words a glance can read.
 *
 * Used wherever a number is being held rather than being current, so that the
 * age is never left for the reader to guess. Everything under ten seconds reads
 * as "just now" because the probe loop runs every two seconds and "2s ago" would
 * be noise.
 */
export function formatAge(ageMs: number): string {
  // A clock that has drifted backwards reads as "just now" rather than as a
  // negative age, because a number the app cannot vouch for is not shown.
  if (ageMs < 10_000) return "just now";

  const seconds = Math.round(ageMs / 1_000);
  if (seconds < 60) return `${seconds}s ago`;

  const minutes = Math.round(ageMs / 60_000);
  if (minutes < 60) return `${minutes}m ago`;

  return `${Math.round(ageMs / 3_600_000)}h ago`;
}

/**
 * The stretch of time a figure covers.
 *
 * Read from the window on the wire rather than written into the component, so
 * the foot of a tile cannot drift away from the figure sitting above it.
 */
export function formatWindow(windowMs: number | null): string {
  if (windowMs === null) return "Unknown window";

  const seconds = Math.round(windowMs / 1_000);
  if (seconds < 60) {
    return `Last ${seconds} second${seconds === 1 ? "" : "s"}`;
  }

  const minutes = Math.round(windowMs / 60_000);
  return `Last ${minutes} minute${minutes === 1 ? "" : "s"}`;
}

/** Rates arrive as bits per second and are shown in Mbps. */
export function formatRate(value: number): string {
  return (value / 1_000_000).toFixed(1);
}

export function formatTimestamp(value: number | null): string {
  if (value === null) return "Never";
  return new Date(value).toLocaleString();
}

export const VALUE_STATE_LABEL: Record<ValueState, string> = {
  measured: "Measured",
  calculated: "Calculated",
  stale: "Stale",
  unavailable: "Unavailable",
};

/**
 * A wall-clock time, for a probe or an incident.
 *
 * Seconds matter here: the probe loop runs every two seconds and an incident
 * start time is read as evidence, so a minute-resolution timestamp would round
 * away the part being looked at.
 *
 * The hour follows the clock format setting, so a reader who wants a
 * twelve-hour clock gets one everywhere rather than in some places.
 */
export function formatClock(value: number | null): string {
  if (value === null) return "Never";
  return new Date(value).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: usesTwelveHours(),
  });
}

/**
 * A wall-clock time to the minute, for the span a chart covers.
 *
 * Minutes, not seconds: a range that spans hours or days would be unreadable
 * with seconds on it, and the point of the label is to say when the data is
 * from rather than to time a single probe.
 */
export function formatClockMinute(value: number): string {
  return new Date(value).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    hour12: usesTwelveHours(),
  });
}

/**
 * The router sentence, from the three-state value the backend sends.
 *
 * "No reading" is deliberately distinct from "Unreachable". One means the
 * router was never probed; the other means it was probed and did not answer, and
 * treating them as the same would be a claim about the local network that
 * nothing measured.
 */
export function formatReachability(reachable: boolean | null): string {
  if (reachable === null) return "No reading";
  return reachable ? "Reachable" : "Unreachable";
}