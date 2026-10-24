import { useEffect, useState } from "react";

import { isTauri, NO_BACKEND_MESSAGE } from "../lib/tauri/environment";
import { getLatencySeries } from "../lib/tauri/overview";
import type { LatencyPoint } from "../types/latency";

export type LatencySeriesState =
  | { status: "loading" }
  | { status: "ready"; points: LatencyPoint[] }
  | { status: "error"; message: string };

function describeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "The latency history could not be loaded.";
}

/**
 * Reads the latency series on mount and on a timer.
 *
 * Separate from the overview hook on purpose: the panel needs a current number
 * and no history, and folding a growing array into the overview poll would put
 * chart data into every snapshot the panel reads.
 *
 * The series is owned by the backend. Keeping the buffer there rather than
 * accumulating polls here means a dropped or slow poll cannot silently punch a
 * hole in the line, and feature 5 can seed it from storage on startup.
 */
export function useLatencySeries(pollMs?: number): LatencySeriesState {
  const [state, setState] = useState<LatencySeriesState>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;

    // No Rust behind a browser tab, so say that rather than let `invoke` throw.
    if (!isTauri()) {
      setState({ status: "error", message: NO_BACKEND_MESSAGE });
      return;
    }

    const tick = async () => {
      if (cancelled) return;
      if (document.visibilityState !== "visible") return;

      try {
        const points = await getLatencySeries();
        if (!cancelled) setState({ status: "ready", points });
      } catch (error: unknown) {
        if (cancelled) return;
        // A failed poll keeps the last good series: blanking a chart because
        // one read failed would throw away real history over a dropped frame.
        // A failure with nothing to keep is a different thing, and says so
        // rather than reading as "still waiting for the first probe".
        setState((previous) =>
          previous.status === "ready"
            ? previous
            : { status: "error", message: describeError(error) },
        );
        console.warn("latency series refresh failed", error);
      }
    };

    void tick();

    if (pollMs === undefined) return;

    const timer = window.setInterval(() => {
      void tick();
    }, pollMs);

    const onVisible = () => {
      if (document.visibilityState === "visible") void tick();
    };
    document.addEventListener("visibilitychange", onVisible);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [pollMs]);

  return state;
}
