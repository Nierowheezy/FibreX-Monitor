import { useEffect, useState } from "react";

import { isTauri, NO_BACKEND_MESSAGE } from "../lib/tauri/environment";
import { getConnectionOverview } from "../lib/tauri/overview";
import type { ConnectionOverview } from "../types/overview";

export type ConnectionOverviewState =
  | { status: "loading" }
  | { status: "ready"; overview: ConnectionOverview }
  | { status: "error"; message: string };

function describeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "The connection overview could not be loaded.";
}

export type OverviewOptions = {
  /** Re-reads whenever this changes. */
  refreshKey?: unknown;
  /**
   * Re-reads on this interval. The monitoring loop probes on its own schedule
   * in Rust, so the frontend's only job is to notice.
   */
  pollMs?: number;
};

/**
 * Reads the overview on mount, whenever `refreshKey` changes, and on a timer
 * when `pollMs` is set.
 *
 * Polling the one snapshot command is what keeps the payload coherent: a
 * response can never pair a fresh latency with a stale timestamp, because both
 * come from the same read.
 *
 * The menu-bar panel passes no interval. It is only on screen while someone is
 * looking at it and already re-reads on focus, so a second poller there would
 * be pure waste.
 */
export function useConnectionOverview(options: OverviewOptions = {}): ConnectionOverviewState {
  const { refreshKey, pollMs } = options;
  const [state, setState] = useState<ConnectionOverviewState>({
    status: "loading",
  });

  useEffect(() => {
    let cancelled = false;

    // Opening the built dashboard in a browser has no Rust behind it. Saying so
    // is truthful and actionable; letting `invoke` throw here would surface a
    // "reading 'invoke'" error that reads like a fault in the app.
    if (!isTauri()) {
      setState({ status: "error", message: NO_BACKEND_MESSAGE });
      return;
    }

    getConnectionOverview()
      .then((overview) => {
        if (!cancelled) setState({ status: "ready", overview });
      })
      .catch((error: unknown) => {
        if (!cancelled) setState({ status: "error", message: describeError(error) });
      });

    return () => {
      cancelled = true;
    };
  }, [refreshKey]);

  useEffect(() => {
    if (pollMs === undefined) return;

    let cancelled = false;

    const tick = async () => {
      // Only poll while the window is actually visible. A background window
      // quietly updating is waste; worse, a poll landing mid-hide can paint a
      // reading the moment it stops mattering.
      if (document.visibilityState !== "visible") return;

      try {
        const overview = await getConnectionOverview();
        if (!cancelled) setState({ status: "ready", overview });
      } catch (error: unknown) {
        // A failed poll keeps the last good reading on screen. Replacing a real
        // measurement with an error because one frame was dropped would be a
        // worse lie than a number a few seconds old.
        if (!cancelled) console.warn("overview refresh failed", error);
      }
    };

    const timer = window.setInterval(() => {
      void tick();
    }, pollMs);

    // Coming back to a hidden window should show the present, not whatever was
    // on screen when it was last looked at.
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
