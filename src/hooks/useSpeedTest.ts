import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import { runSpeedTest } from "../lib/tauri/speedtest";
import { isTauri } from "../lib/tauri/environment";
import type { SpeedTestOutcome, SpeedTestPhase, SpeedTestProgress } from "../types/speedtest";

/** What the test screen is showing right now. */
export type SpeedTestState =
  | { status: "idle" }
  | {
      status: "running";
      phase: SpeedTestPhase;
      megabits: number | null;
      latencyMs: number | null;
    }
  | { status: "done"; outcome: SpeedTestOutcome };

/**
 * Runs the speed test and follows it while it runs.
 *
 * The needle and the figures are moved from the backend's own progress
 * reports, so they point at a rate that was really transferred rather than at
 * an animation that only looks like a measurement. Each figure stays on the
 * screen until the phase that replaces it starts: the ping samples count up
 * during the ping, the rate takes over for the download, then for the upload.
 */
export function useSpeedTest(): {
  state: SpeedTestState;
  run: () => void;
} {
  const [state, setState] = useState<SpeedTestState>({ status: "idle" });

  // The progress listener, so the dial follows the test while it runs.
  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listen<SpeedTestProgress>("speedtest://progress", (event) => {
      if (cancelled) return;
      const progress = event.payload;
      setState((previous) => {
        // A report that arrives after the test has finished is not something to
        // show: the finished figures are the ones that matter.
        if (previous.status !== "running") {
          return previous;
        }

        // A figure from another phase does not overwrite this one. Rates come
        // only from the transfer phases, samples only from the ping.
        const megabits =
          progress.phase === "ping" || progress.bytesPerSecond === null
            ? previous.megabits
            : (progress.bytesPerSecond * 8) / 1_000_000;
        const latencyMs =
          progress.latencyMs === null || progress.latencyMs === undefined
            ? previous.latencyMs
            : progress.latencyMs;

        return {
          status: "running",
          phase: progress.phase,
          megabits,
          latencyMs,
        };
      });
    }).then((stop) => {
      if (cancelled) {
        stop();
        return;
      }
      unlisten = stop;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const run = useCallback(() => {
    setState({ status: "running", phase: "ping", megabits: null, latencyMs: null });

    void runSpeedTest()
      .then((outcome) => setState({ status: "done", outcome }))
      .catch((error: unknown) =>
        setState({
          status: "done",
          outcome: {
            kind: "failed",
            message:
              typeof error === "string"
                ? error
                : "The speed test could not be run.",
          },
        }),
      );
  }, []);

  return { state, run };
}
