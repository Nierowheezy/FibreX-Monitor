import { useEffect, useState } from "react";

import { isTauri, NO_BACKEND_MESSAGE } from "../lib/tauri/environment";
import { getHistoryInfo, getIncidents, getUptime } from "../lib/tauri/history";
import type { HistoryInfo, IncidentSummary, UptimeSummary } from "../types/history";

export type HistoryState =
  | { status: "loading" }
  | {
      status: "ready";
      incidents: IncidentSummary[];
      info: HistoryInfo;
      uptime: UptimeSummary;
    }
  | { status: "error"; message: string };

function describeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "The incident history could not be loaded.";
}

/**
 * Reads the recorded incidents and what history covers.
 *
 * Both in one hook because they are only ever read together: a list of
 * incidents with no idea whether history is switched on cannot be rendered
 * honestly, because an empty list means two very different things depending on
 * the answer.
 *
 * This is deliberately slower than the live overview poll. Incidents change
 * when an outage starts or ends, which is a minutes-scale event, so polling it
 * every two seconds would only re-fetch a list that cannot have changed.
 */
export function useHistory(pollMs = 15_000): HistoryState {
  const [state, setState] = useState<HistoryState>({ status: "loading" });

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
        const [incidents, info, uptime] = await Promise.all([
          getIncidents(),
          getHistoryInfo(),
          getUptime(),
        ]);
        if (!cancelled) setState({ status: "ready", incidents, info, uptime });
      } catch (error: unknown) {
        if (cancelled) return;
        // A failed poll keeps the last good list, for the same reason the
        // chart does: throwing away real recorded outages because one read
        // failed would be a worse lie than showing them a moment late.
        setState((previous) =>
          previous.status === "ready"
            ? previous
            : { status: "error", message: describeError(error) },
        );
        console.warn("incident history refresh failed", error);
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
