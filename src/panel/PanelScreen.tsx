import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";

import Sparkline from "../components/chart/Sparkline";
import StatusBadge from "../components/status/StatusBadge";
import {
  DashboardIcon,
  DownloadIcon,
  PauseIcon,
  PlayIcon,
  PulseIcon,
  RouterIcon,
  UploadIcon,
  Wordmark,
} from "../components/ui/icons";
import {
  formatAge,
  formatLatency,
  formatProbeFailures,
} from "../features/overview/format";
import { HEALTH_LABEL, HEALTH_TONE } from "../features/overview/health";
import { formatRate } from "../features/router/format";
import { showMainWindow } from "../lib/tauri/overview";
import { setMonitoringPaused } from "../lib/tauri/overview";
import { useConnectionOverview } from "../hooks/useConnectionOverview";
import { useRouterReport } from "../hooks/useRouterReport";
import { useStoredSeries } from "../hooks/useStoredSeries";
import { isTauri } from "./tauri";

/** Matches the probe interval, so the popover never waits on a stale reading. */
const REFRESH_MS = 2_000;

/** How far back the panel's sparkline reaches. The live picture, not a day. */
const SPARK_WINDOW_MS = 5 * 60_000;

/**
 * The menu-bar panel.
 *
 * It reads the same commands as the dashboard, so a value cannot appear here
 * that the contract did not produce. It polls on the same interval, because the
 * popover is a monitor too: someone who glances at it expects the number there
 * to be the current one.
 *
 * The focus refresh is kept as well, so opening the panel shows a reading
 * immediately instead of waiting out the first interval.
 */
export default function PanelScreen() {
  const [refreshKey, setRefreshKey] = useState(0);
  const state = useConnectionOverview({ refreshKey, pollMs: REFRESH_MS });
  const router = useRouterReport();
  const stored = useStoredSeries(SPARK_WINDOW_MS, 5_000);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    const unlisten = getCurrentWindow().onFocusChanged((event) => {
      if (event.payload) {
        setRefreshKey((key) => key + 1);
      }
    });

    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  // Escape hides the panel the way a menu-bar popover is expected to behave.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isTauri()) {
        void getCurrentWindow().hide();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const overview = state.status === "ready" ? state.overview : null;
  const latency = overview?.latency.value ?? null;

  // One formatter for this figure on both surfaces, so the panel cannot show a
  // percentage where the dashboard shows counts.
  const failures =
    overview === null || overview.packetLoss.value === null
      ? null
      : formatProbeFailures(overview.packetLoss.value, overview.probeFailureBasis);

  // How long the displayed latency has been held, when it is being held at all.
  // A number from forty seconds ago must never sit here looking like one from
  // now, so the age travels with it.
  const heldFor =
    overview?.latency.state === "stale" && overview.latency.measuredAt !== null
      ? Date.now() - overview.latency.measuredAt
      : null;

  const routerReach = overview?.router ?? null;
  const traffic = router.status === "ready" ? router.report.traffic : null;
  const paused = overview?.paused ?? false;

  const togglePause = useCallback(() => {
    if (!isTauri()) {
      return;
    }
    void setMonitoringPaused(!paused);
    setRefreshKey((key) => key + 1);
  }, [paused]);

  return (
    <div className="flex h-full flex-col bg-canvas px-gutter pt-gutter pb-stack text-ink">
      <header className="flex items-center gap-gap">
        <Wordmark className="size-[17px] flex-none" />
        <h1 className="font-display text-body font-medium tracking-title">
          FibreX Monitor
        </h1>

        <div className="ml-auto flex items-center gap-gap">
          {/* The one live indicator, and never the only signal: the pause
              control beside it carries the same fact in words. */}
          <span
            className={`live-dot ${
              overview !== null && !paused ? "bg-ember" : "bg-idle"
            }`}
            aria-hidden="true"
          />
          <button
            type="button"
            onClick={togglePause}
            aria-label={paused ? "Resume monitoring" : "Pause monitoring"}
            title={paused ? "Resume monitoring" : "Pause monitoring"}
            className="grid size-7 place-items-center rounded-[9px] bg-surface-2 text-ink-muted transition-colors duration-[var(--motion)] hover:bg-surface-3 hover:text-ink"
          >
            {paused ? <PlayIcon className="size-3.5" /> : <PauseIcon className="size-3.5" />}
          </button>
        </div>
      </header>

      <div className="mt-gutter">
        <div className="flex items-end justify-between gap-row">
          <div>
            <p className="label-caps mb-1.5">Latency</p>
            {latency === null ? (
              <p className="font-display text-[1.75rem] leading-[1.1] tracking-figure text-ink-faint">
                No reading
              </p>
            ) : (
              <p className="flex items-baseline gap-1">
                <span className="tnum font-display text-[3rem] font-light leading-[0.95] tracking-figure">
                  {formatLatency(latency)}
                </span>
                <span className="font-display text-body text-ink-faint">ms</span>
              </p>
            )}
          </div>

          {overview !== null && (
            <StatusBadge tone={HEALTH_TONE[overview.healthState]}>
              {HEALTH_LABEL[overview.healthState]}
            </StatusBadge>
          )}
        </div>

        <p className="mt-gap flex items-center justify-between gap-row font-mono text-micro text-ink-faint">
          <span>
            {overview === null
              ? "never probed"
              : overview.lastProbeAt === null
                ? "never probed"
                : `probed ${formatAge(Date.now() - overview.lastProbeAt)}`}
          </span>
          <span>{overview?.primaryTarget?.host ?? "no target"}</span>
        </p>
      </div>

      {/* The shape of the connection, behind the numbers.
          The chart is taken out of the flow, because an SVG with a view box has
          an intrinsic width, and that width would become the minimum width of
          the column and push the panel's content past the edge of the window. */}
      <div className="relative -mx-gutter mt-gutter h-[62px]">
        <Sparkline points={stored.status === "ready" ? stored.points : []} />
      </div>

      <div className="mt-stack flex-1 overflow-auto">
        {state.status === "error" && (
          <p role="alert" className="py-row text-micro text-bad">
            {state.message}
          </p>
        )}

        {overview !== null && (
          <>
            <Row icon={<PulseIcon className="size-3.5" />} label="Probe failures">
              {failures === null ? <Empty /> : <>{failures}</>}
            </Row>

            <Row icon={<DownloadIcon className="size-3.5" />} label="Download">
              {traffic?.receivedBytesPerSecond === null ||
              traffic?.receivedBytesPerSecond === undefined ? (
                <Empty />
              ) : (
                <>{formatRate(traffic.receivedBytesPerSecond)}</>
              )}
            </Row>

            <Row icon={<UploadIcon className="size-3.5" />} label="Upload">
              {traffic?.sentBytesPerSecond === null ||
              traffic?.sentBytesPerSecond === undefined ? (
                <Empty />
              ) : (
                <>{formatRate(traffic.sentBytesPerSecond)}</>
              )}
            </Row>

            {/* Kept separate from the internet reading and never merged with it:
                a reachable router and an unreachable internet are different
                facts, and only one of them is about the provider. */}
            <Row icon={<RouterIcon className="size-3.5" />} label="Router">
              {routerReach === null || routerReach.reachable === null ? (
                <Empty />
              ) : (
                <>
                  {routerReach.reachable ? "Reachable" : "Unreachable"}
                  {routerReach.reachable && routerReach.latency.value !== null && (
                    <span className="ml-2 font-mono text-micro text-ink-faint">
                      {formatLatency(routerReach.latency.value)} ms
                    </span>
                  )}
                </>
              )}
            </Row>

            {/* There is no device row. The router's device list is behind an
                endpoint that answers "NONE" on this firmware, because the table
                is built lazily from an ARP scan. A row that could only ever say
                "no reading" would be a permanent apology for a screen that is
                not there. */}
          </>
        )}

        {heldFor !== null && (
          <p className="py-row text-micro text-warn">
            Held reading, measured {formatAge(heldFor)}
          </p>
        )}
      </div>

      <div className="mt-stack flex gap-gap">
        <button
          type="button"
          onClick={() => {
            if (isTauri()) {
              void showMainWindow();
            }
          }}
          className="inline-flex h-[38px] flex-1 cursor-pointer items-center justify-center gap-gap rounded-inner bg-ember text-micro font-semibold tracking-label text-on-ember transition-colors duration-[var(--motion)] hover:bg-ember-bright"
        >
          <DashboardIcon className="size-3.5" />
          Open dashboard
        </button>

        <button
          type="button"
          onClick={togglePause}
          className="inline-flex h-[38px] w-24 flex-none cursor-pointer items-center justify-center gap-gap rounded-inner bg-surface-2 text-micro font-semibold tracking-label text-ink-muted transition-colors duration-[var(--motion)] hover:bg-surface-3 hover:text-ink"
        >
          {paused ? <PlayIcon className="size-3.5" /> : <PauseIcon className="size-3.5" />}
          {paused ? "Resume" : "Pause"}
        </button>
      </div>

      <p className="mt-stack flex items-center justify-between gap-row font-mono text-micro text-ink-faint">
        <span>every 2s &middot; local only</span>
      </p>
    </div>
  );
}

function Empty() {
  return <span className="font-regular text-ink-faint">No reading</span>;
}

function Row({
  icon,
  label,
  children,
}: {
  icon: React.ReactNode;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-row border-b border-hairline py-[9px] last:border-b-0">
      <span className="flex-none text-ink-faint">{icon}</span>
      <span className="flex-1 text-small text-ink-muted">{label}</span>
      <span className="text-right font-mono text-small font-medium">{children}</span>
    </div>
  );
}
