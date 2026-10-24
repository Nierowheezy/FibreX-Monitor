/**
 * The speed test's vocabulary. Mirrors `src-tauri/src/speedtest/mod.rs`; keep
 * both sides in step.
 */

/** Which part of the test is running. */
export type SpeedTestPhase = "ping" | "download" | "upload";

/** What the running test has got so far. */
export interface SpeedTestProgress {
  phase: SpeedTestPhase;
  /** The rate so far in this phase, in bytes per second. Null in the ping phase. */
  bytesPerSecond: number | null;
  /** How many bytes have moved in this phase so far. */
  bytesSoFar: number | null;
  /** The latest round-trip sample, in milliseconds. Non-null only in the ping phase. */
  latencyMs: number | null;
}

/** What a finished test measured. */
export interface SpeedTestResult {
  /** Round trip time to the test host, in milliseconds. */
  latencyMs: number | null;
  /** How much the round trip varied between the samples, in milliseconds. */
  jitterMs: number | null;
  /** Bytes per second coming down. */
  downloadBytesPerSecond: number | null;
  /** Bytes per second going up. */
  uploadBytesPerSecond: number | null;
  /** The host the test ran against. */
  host: string;
  /** UTC epoch milliseconds of when it finished. */
  finishedAt: number;
}

/**
 * How a test ended.
 *
 * `completed` carries the figures, and any of them may be null: a line that
 * uploads at nothing but downloads at 200 Mbps is a real situation, and a
 * result that dropped the whole test because one of the three failed would be
 * throwing away the two that worked.
 */
export type SpeedTestOutcome =
  | { kind: "completed"; result: SpeedTestResult }
  | { kind: "failed"; message: string };
