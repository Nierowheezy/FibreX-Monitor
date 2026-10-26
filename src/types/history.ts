/**
 * The local history vocabulary. Mirrors `src-tauri/src/models/history.rs` and
 * `src-tauri/src/monitoring/store.rs`; keep both sides in step.
 */

/**
 * One recorded outage.
 *
 * `recoveredAt` is null while the outage is still running, which is a real and
 * different state from a recovered one: an open incident has no duration yet,
 * only a duration so far.
 *
 * There is deliberately no `cause` field. A failed TCP connect says a
 * connection did not open. It does not say why.
 */
export interface IncidentSummary {
  id: number;
  targetId: number;
  /** UTC epoch milliseconds of the first failed probe. */
  startedAt: number;
  /**
   * UTC epoch milliseconds of the first success after the outage, or null if
   * it is still running. When `closedBy` is `"unobserved"` this is the last
   * probe that was actually seen, and the duration is the part of the outage
   * the app watched rather than the whole of it.
   */
  recoveredAt: number | null;
  /** How many failed probes were recorded. */
  probeCount: number;
  /**
   * Whether the router answered while this outage was running. Null when no
   * router reading covers the window, which is not the same as the router being
   * down.
   */
  routerReachable: boolean | null;
  /**
   * How the incident closed: `"probe"` when a successful probe was seen,
   * `"unobserved"` when probing stopped first (pause, quit, sleep), null while
   * it is still open.
   *
   * This is what stops an unobserved gap reading as confirmed downtime.
   */
  closedBy: "probe" | "unobserved" | null;
}

/**
 * What the history database can currently answer.
 *
 * `enabled: false` means history is switched off, which the interface must say
 * plainly rather than showing an empty list that would read as "no outages ever
 * happened".
 */
export interface HistoryInfo {
  enabled: boolean;
  /** UTC epoch milliseconds of the oldest stored sample, or null. */
  earliestSampleAt: number | null;
  sampleCount: number;
  /** How long raw samples are kept before the oldest are dropped. */
  retentionMs: number;
}

/**
 * How much of a period the connection was up, over stored probes.
 *
 * "Uptime" here is the share of probes that connected. It is a count of
 * observations, not a stopwatch: the app knows what it measured and when, not
 * what the connection was doing between measurements.
 *
 * `windowStart` and `windowEnd` exist so the interface states the period the
 * figure covers instead of attaching it to "today", which the stored data may
 * not span at all.
 */
export interface UptimeSummary {
  /** Share of probes that connected, 0 to 100. Null when nothing is stored. */
  percent: number | null;
  connected: number;
  total: number;
  /** UTC epoch milliseconds of the oldest sample in the window, or null. */
  windowStart: number | null;
  /** UTC epoch milliseconds of the newest sample in the window, or null. */
  windowEnd: number | null;
}
