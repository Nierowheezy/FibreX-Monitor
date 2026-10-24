import { useEffect, useState } from "react";

import { isTauri, NO_BACKEND_MESSAGE } from "../lib/tauri/environment";
import { getRouterReport } from "../lib/tauri/router";
import type { RouterReport } from "../types/overview";

export type RouterReportState =
  | { status: "loading" }
  | { status: "ready"; report: RouterReport }
  | { status: "error"; message: string };

function describeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "The router could not be read.";
}

/**
 * The most recent router reading.
 *
 * Polled far more slowly than the overview, because it mirrors the backend's
 * reader cadence: the backend re-reads the router every thirty seconds, so
 * polling faster would only re-fetch the same snapshot. A failure keeps the last
 * good report, for the same reason the chart keeps its last good series: a router
 * that answered a minute ago still has a real address, and blanking the screen
 * because one read timed out would overstate what happened.
 */
export function useRouterReport(pollMs = 30_000): RouterReportState {
  const [state, setState] = useState<RouterReportState>({ status: "loading" });

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
        const report = await getRouterReport();
        if (!cancelled) setState({ status: "ready", report });
      } catch (error: unknown) {
        if (cancelled) return;
        setState((previous) =>
          previous.status === "ready"
            ? previous
            : { status: "error", message: describeError(error) },
        );
        console.warn("router report refresh failed", error);
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
  }, [pollMs]);

  return state;
}
