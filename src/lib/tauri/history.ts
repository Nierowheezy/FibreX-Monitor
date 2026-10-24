import { invoke } from "@tauri-apps/api/core";

import type { HistoryInfo, IncidentSummary, UptimeSummary } from "../../types/history";
import type { LatencyPoint } from "../../types/latency";

/**
 * The only place the history commands are called from. Components go through
 * here so the command names and their contracts stay in one file.
 */

/**
 * Recorded outages, newest first.
 *
 * Returns an empty array rather than throwing when history is switched off.
 * That is not a failure: nothing has gone wrong, there is simply nothing
 * stored. The screen distinguishes the two with [`getHistoryInfo`].
 */
export function getIncidents(): Promise<IncidentSummary[]> {
  return invoke<IncidentSummary[]>("get_incidents");
}

/** What the history database currently holds. */
export function getHistoryInfo(): Promise<HistoryInfo> {
  return invoke<HistoryInfo>("get_history_info");
}

/**
 * Latency points read from storage rather than from the in-memory buffer.
 *
 * The buffer is windowed to the last few minutes and dies with the process.
 * This is what backs any range wider than this launch.
 */
export function getStoredSeries(sinceMs: number): Promise<LatencyPoint[]> {
  return invoke<LatencyPoint[]>("get_stored_series", { sinceMs });
}

/**
 * How much of the stored history the connection was up.
 *
 * The share of stored probes that connected, over every probe the app has
 * recorded. It returns nulls rather than a zero percent when nothing is stored,
 * because zero percent uptime is a claim that the connection never worked.
 */
export function getUptime(): Promise<UptimeSummary> {
  return invoke<UptimeSummary>("get_uptime");
}
