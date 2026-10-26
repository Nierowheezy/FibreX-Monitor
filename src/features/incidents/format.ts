import type { IncidentSummary } from "../../types/history";

/**
 * How a recorded outage is described.
 *
 * Everything here takes evidence the backend actually stored and turns it into
 * words. Nothing infers a cause, and nothing fills a gap with a plausible
 * value, because the whole point of recording an outage is that the record can
 * be trusted later.
 */

/**
 * A duration in words, at the coarsest useful granularity.
 *
 * An outage of 4.2 seconds is not interestingly different from one of 4.8, and
 * the seconds are already in the start and recovery clock times beside it. What
 * a reader wants from this line is the magnitude, so it rounds once it is past
 * a minute.
 */
export function formatDuration(ms: number): string {
  // A clock that moved backwards would otherwise produce a negative duration,
  // which is not a thing that happened and must never be printed as one.
  if (ms < 0) return "Unknown";

  const seconds = ms / 1_000;
  if (seconds < 60) {
    // One decimal below ten seconds: an outage of 2.4 s and one of 2.0 s are
    // different enough to matter when a connection keeps flickering.
    return seconds < 10 ? `${seconds.toFixed(1)}s` : `${Math.round(seconds)}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainder = Math.round(seconds % 60);
  if (minutes < 60) {
    return remainder === 0 ? `${minutes}m` : `${minutes}m ${remainder}s`;
  }

  const hours = Math.floor(minutes / 60);
  const leftoverMinutes = minutes % 60;
  return leftoverMinutes === 0 ? `${hours}h` : `${hours}h ${leftoverMinutes}m`;
}

/** Whether the outage is still running. */
export function isOpen(incident: IncidentSummary): boolean {
  return incident.recoveredAt === null;
}

/**
 * Whether probing stopped before the recovery was seen.
 *
 * These incidents are closed where the evidence ends, so their duration is the
 * part of the outage that was watched — a lower bound, not a measurement of the
 * whole thing. Presenting that as a confirmed recovery would be a lie about
 * what the app saw.
 */
export function wasUnobserved(incident: IncidentSummary): boolean {
  return incident.closedBy === "unobserved";
}

/**
 * How long the outage ran, or has run so far.
 *
 * `now` is a parameter rather than read inside, so a component re-rendering
 * cannot produce a different answer for the same incident than the one the last
 * render produced, and so this is testable without waiting for real time.
 */
export function durationOf(incident: IncidentSummary, now: number): number {
  return (incident.recoveredAt ?? now) - incident.startedAt;
}

/**
 * What the router was doing while this outage ran.
 *
 * The distinction between "the router was up" and "the router was never read"
 * is the entire reason this field exists. A page of incidents all claiming the
 * router was unreachable, when it was simply not checked, would be a fabricated
 * diagnosis of the user's own equipment.
 */
export function describeRouter(incident: IncidentSummary): string {
  if (incident.routerReachable === null) return "Router not checked";
  return incident.routerReachable ? "Router stayed up" : "Router also unreachable";
}

/**
 * The one-line headline for an incident.
 *
 * Deliberately blunt and free of blame: it says probes failed, which is what
 * was measured, and it stops there.
 */
export function describeIncident(incident: IncidentSummary, now: number): string {
  const duration = formatDuration(durationOf(incident, now));
  if (isOpen(incident)) return `Failing for ${duration} and counting`;
  if (wasUnobserved(incident)) {
    return `Lasted at least ${duration} · monitoring stopped`;
  }
  return `Lasted ${duration}`;
}

/**
 * A summary of a whole list of incidents, for the screen's header.
 *
 * Returns null for an empty list rather than a total of zero, because "zero
 * outages" and "nothing recorded" are different claims and only one of them is
 * supported by an empty database. The caller decides which one it is.
 */
export function summariseIncidents(
  incidents: IncidentSummary[],
  now: number,
): { count: number; totalMs: number; longestMs: number; open: number; unobserved: number } | null {
  if (incidents.length === 0) return null;

  let totalMs = 0;
  let longestMs = 0;
  let open = 0;
  let unobserved = 0;

  for (const incident of incidents) {
    const duration = durationOf(incident, now);
    totalMs += duration;
    longestMs = Math.max(longestMs, duration);
    if (isOpen(incident)) open += 1;
    else if (wasUnobserved(incident)) unobserved += 1;
  }

  return { count: incidents.length, totalMs, longestMs, open, unobserved };
}
