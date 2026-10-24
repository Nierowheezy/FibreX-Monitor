/**
 * One latency reading. `value` is null when the sample exists but carries no
 * reading, which is not the same as the sample being absent.
 */
export type LatencyPoint = {
  /** UTC epoch milliseconds. */
  at: number;
  /** Milliseconds, or null when nothing was measured at this time. */
  value: number | null;
};