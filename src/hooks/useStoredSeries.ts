import { useEffect, useState } from "react";

import { isTauri, NO_BACKEND_MESSAGE } from "../lib/tauri/environment";
import { getStoredSeries } from "../lib/tauri/history";
import type { LatencyPoint } from "../types/latency";

export type StoredSeriesState =
  | { status: "loading" }
  | { status: "ready"; points: LatencyPoint[] }
  | { status: "error"; message: string };

function describeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "The stored history could not be read.";
}

/**
 * The latency history for a range, read from storage.
 *
 * The in-memory series only covers the last few minutes and dies with the
 * process. This is the stored history, so it is what backs a range wider than
 * the current launch.
 *
 * A failed read keeps the last good series, for the same reason the live chart
 * does: throwing away real recorded history because one read failed would be a
 * worse lie than showing it a moment late.
 */
export function useStoredSeries(
  windowMs: number,
  pollMs = 5_000,
): StoredSeriesState {
  const [state, setState] = useState<StoredSeriesState>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;

    if (!isTauri()) {
      setState({ status: "error", message: NO_BACKEND_MESSAGE });
      return;
    }

    const tick = async () => {
      if (cancelled) return;
      if (document.visibilityState !== "visible") return;

      try {
        const points = await getStoredSeries(windowMs);
        if (cancelled) return;
        setState({ status: "ready", points });
      } catch (error: unknown) {
        if (cancelled) return;
        setState((previous) =>
          previous.status === "ready"
            ? previous
            : { status: "error", message: describeError(error) },
        );
        console.warn("stored latency series refresh failed", error);
      }
    };

    void tick();

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
  }, [windowMs, pollMs]);

  return state;
}
