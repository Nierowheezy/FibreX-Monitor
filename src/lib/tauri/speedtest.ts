import { invoke } from "@tauri-apps/api/core";

import type { SpeedTestOutcome } from "../../types/speedtest";

/**
 * The only place the speed test command is called from.
 */
export function runSpeedTest(): Promise<SpeedTestOutcome> {
  return invoke<SpeedTestOutcome>("run_speed_test");
}
