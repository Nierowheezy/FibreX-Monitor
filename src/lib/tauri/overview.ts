import { invoke } from "@tauri-apps/api/core";

import type { LatencyPoint } from "../../types/latency";
import type { ConnectionOverview } from "../../types/overview";

/**
 * The only place the overview command is called from. Components go through
 * here so the command name and its contract stay in one file.
 */
export function getConnectionOverview(): Promise<ConnectionOverview> {
  return invoke<ConnectionOverview>("get_connection_overview");
}

/**
 * Recent latency points for the chart, oldest first.
 *
 * A point with a null value is a probe that ran and failed. It is not the same
 * as a missing point: the gap is the observation.
 */
export function getLatencySeries(): Promise<LatencyPoint[]> {
  return invoke<LatencyPoint[]>("get_latency_series");
}

/**
 * Raises the dashboard window. The panel has no other way to open it, and the
 * dashboard is kept alive while hidden, so this only shows and focuses.
 */
export function showMainWindow(): Promise<void> {
  return invoke<void>("show_main_window");
}

/**
 * Pauses or resumes the probe loop, and returns the state it ended in.
 *
 * The state comes back from the backend rather than being assumed here, so a
 * control that asked to pause can tell whether it actually did.
 */
export function setMonitoringPaused(paused: boolean): Promise<boolean> {
  return invoke<boolean>("set_monitoring_paused", { paused });
}

/**
 * Whether the probe loop is paused right now.
 */
export function isMonitoringPaused(): Promise<boolean> {
  return invoke<boolean>("is_monitoring_paused");
}