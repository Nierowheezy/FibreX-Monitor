import { useState } from "react";

import LatencyChart from "../../components/chart/LatencyChart";
import StatusBadge from "../../components/status/StatusBadge";
import { Bento, Card, CardBody, CardHead, EmptyNote, Figure } from "../../components/ui/Card";
import {
  ClockIcon,
  DatabaseIcon,
  DownloadIcon,
  IncidentsIcon,
  OpticalIcon,
  PulseIcon,
  RouterIcon,
  SignalIcon,
  TrafficIcon,
  UploadIcon,
} from "../../components/ui/icons";
import { useConnectionOverview } from "../../hooks/useConnectionOverview";
import { useHistory } from "../../hooks/useHistory";
import { useRouterReport } from "../../hooks/useRouterReport";
import { useStoredSeries } from "../../hooks/useStoredSeries";
import type { ConnectionOverview } from "../../types/overview";
import ConnectionHero from "./ConnectionHero";
import { formatAge, formatLatency, formatProbeFailures } from "./format";
import { HEALTH_LABEL, HEALTH_TONE } from "./health";
import MetricCard from "./MetricCard";
import { DEFAULT_RANGE_INDEX, rangeAt } from "./ranges";
import { describeUptimeWindow, formatUptimePercent } from "./uptime";
import { formatBytes, formatOpticalDbm, formatRate } from "../router/format";
import LastIncidentCard from "../incidents/LastIncidentCard";

/**
 * How often the screen re-reads. The probe loop runs every two seconds, so this
 * matches it: polling faster would only re-fetch a number that has not changed,
 * and polling slower would show readings the loop has already replaced.
 */
const REFRESH_MS = 2_000;

/**
 * The router verdict as a labelled pill.
 *
 * Three states, because two would be a lie: a router that was never probed is
 * not a router that is down, and "Unreachable" would claim the local network was
 * checked when it was not.
 */
function RouterBadge({ reachable }: { reachable: boolean | null }) {
  if (reachable === null) {
    return <StatusBadge tone="neutral">No reading</StatusBadge>;
  }
  return reachable ? (
    <StatusBadge tone="ok">Reachable</StatusBadge>
  ) : (
    <StatusBadge tone="error">Unreachable</StatusBadge>
  );
}

export default function OverviewScreen() {
  const state = useConnectionOverview({ pollMs: REFRESH_MS });
  const history = useHistory();
  const routerReport = useRouterReport();

  // The range in force, and the stored history for it.
  //
  // The stored series is the one the range control reads. The in-memory series
  // only covers the last few minutes and dies with the process, so it cannot
  // back a range.
  const [rangeIndex, setRangeIndex] = useState(DEFAULT_RANGE_INDEX);
  const range = rangeAt(rangeIndex);
  const stored = useStoredSeries(range.windowMs);

  const storedPoints = stored.status === "ready" ? stored.points : [];

  if (state.status === "loading") {
    return (
      <p className="text-body text-ink-muted" role="status">
        Reading connection status...
      </p>
    );
  }

  if (state.status === "error") {
    return (
      <Card span={12}>
        <CardHead
          icon={<IncidentsIcon className="size-5" />}
          title="Connection overview unavailable"
          sub="The monitoring backend did not answer."        />
        <CardBody>
          <p className="text-small text-ink-muted">{state.message}</p>
        </CardBody>
      </Card>
    );
  }

  const overview: ConnectionOverview = state.overview;
  const router = overview.router;

  // The router's traffic, when two counter readings have produced a rate.
  const traffic = routerReport.status === "ready" ? routerReport.report.traffic : null;
  const counters =
    routerReport.status === "ready"
      ? (routerReport.report.snapshot?.counters ?? null)
      : null;

  // The optical receive power, when the router was read. The design gives
  // this its own small card beside the router card.
  const optical =
    routerReport.status === "ready" && routerReport.report.snapshot !== null
      ? routerReport.report.snapshot.health.opticalRxDbm
      : { value: null, state: "unavailable" as const, measuredAt: null };

  return (
    <Bento>
      <ConnectionHero
        overview={overview}
        points={storedPoints}
        rangeIndex={rangeIndex}
        onRangeChange={setRangeIndex}
      />

      {/* The design puts the most recent outage in the second most prominent
          place on the screen, and it is the right place for it: an outage is
          what a person checking their connection most wants to know. */}
      <LastIncidentCard
        incident={history.status === "ready" ? (history.incidents[0] ?? null) : null}
        now={Date.now()}
      />

      <MetricCard
        span={2}
        icon={<RouterIcon className="size-5" />}
        title="Router"
        sub={router.host ?? "No router configured"}
        metric={router.latency}
        unit="ms"
        foot={
          router.checkedAt === null
            ? "Not probed yet"
            : `Checked ${formatAge(Date.now() - router.checkedAt)}`
        }
        badge={<RouterBadge reachable={router.reachable} />}
        format={formatLatency}
      />

      <MetricCard
        span={2}
        icon={<OpticalIcon className="size-5" />}
        title="Optical Rx"
        sub="ONT receive power"
        metric={optical}
        unit="dBm"
        foot={
          optical.value === null
            ? "Router statistics not read"
            : "Receive power, from the router"
        }
        badge={
          optical.value === null ? (
            <StatusBadge tone="neutral">Unavailable</StatusBadge>
          ) : (
            <StatusBadge tone="ok">Good</StatusBadge>
          )
        }
        format={formatOpticalDbm}
      />

      <MetricCard
        span={4}
        icon={<PulseIcon className="size-5" />}
        title="Probe failures"
        sub="Connects that did not open"
        metric={overview.packetLoss}
        caption="failed"
        foot="Last 5 minutes"
        basis={overview.probeFailureBasis}
        format={formatProbeFailures}
      />

      {/* The traffic rates come from the router's own byte counters, read twice.
          The reason each figure is missing is named, because "no router
          session", "waiting for a second reading" and "the router reset the
          counter" are different facts that need different actions. */}
      <Card span={4}>
        <CardHead
          icon={<TrafficIcon className="size-5" />}
          title="WAN traffic"
          sub="From the router's byte counters"
        />
        <CardBody>
          {traffic === null || traffic.receivedBytesPerSecond === null ? (
            <EmptyNote>
              {traffic?.note ??
                "No rate yet. A rate needs two readings of the router's counters."}
            </EmptyNote>
          ) : (
            <>
              <div className="flex items-baseline justify-between gap-row">
                <span className="flex items-center gap-gap text-small text-ink-muted">
                  <DownloadIcon className="size-4" />
                  Download
                </span>
                <span className="tnum font-display text-title font-light text-ink">
                  {formatRate(traffic.receivedBytesPerSecond)}
                </span>
              </div>
              <div className="mt-row flex items-baseline justify-between gap-row">
                <span className="flex items-center gap-gap text-small text-ink-muted">
                  <UploadIcon className="size-4" />
                  Upload
                </span>
                <span className="tnum font-display text-title font-light text-ink">
                  {formatRate(traffic.sentBytesPerSecond)}
                </span>
              </div>
              {counters !== null && (
                <p className="mt-row flex items-center gap-gap text-micro text-ink-faint">
                  <DatabaseIcon className="size-4" />
                  {formatBytes(counters.bytesReceived)} down,{" "}
                  {formatBytes(counters.bytesSent)} up since the session began
                </p>
              )}
            </>
          )}
        </CardBody>
      </Card>

      {/* Uptime is a real figure now, but only with its period attached. The
          card states the window the stored data actually covers instead of
          calling it "today", which the history may not span. */}
      <Card span={4}>
        <CardHead
          icon={<ClockIcon className="size-5" />}
          title="Uptime"
          sub="From stored probes"
        />
        <CardBody>
          {history.status !== "ready" || !history.info.enabled ? (
            <EmptyNote>
              History is not being recorded, so no uptime figure can be
              calculated.
            </EmptyNote>
          ) : history.uptime.percent === null || describeUptimeWindow(history.uptime) === null ? (
            <EmptyNote>
              Not enough stored yet. Uptime appears once probes have been
              recorded.
            </EmptyNote>
          ) : (
            <>
              <Figure
                value={formatUptimePercent(history.uptime.percent)}
                caption={describeUptimeWindow(history.uptime) ?? undefined}
              />
              <p className="mt-stack text-micro text-ink-faint">
                {history.uptime.connected} of {history.uptime.total} stored probes
                connected. This counts observations, not the time between them.
              </p>
            </>
          )}
        </CardBody>
      </Card>

      {/* The closing row: the detailed history chart across the full width.
          It is the one thing on this screen that is read rather than glanced
          at, so it gets the whole row to itself. */}
      <Card span={12}>
        <CardHead
          icon={<SignalIcon className="size-5" />}
          title="Latency history"
          sub={`Stored probes over the last ${range.label.toLowerCase()}. Gaps are probes that failed.`}
          end={
            <StatusBadge tone={HEALTH_TONE[overview.healthState]}>
              {HEALTH_LABEL[overview.healthState]}
            </StatusBadge>
          }
        />
        <CardBody>
          <LatencyChart points={storedPoints} />
          {stored.status === "error" && (
            <p className="mt-stack text-micro text-bad">
              History unavailable: {stored.message}
            </p>
          )}
          {/* The window the samples actually cover, rather than the window that
              was asked for. A range wider than the history is not a claim that
              the connection was quiet for the whole of it. */}
          <p className="mt-stack text-micro text-ink-faint">
            {storedPoints.length === 0
              ? `No stored probes in the last ${range.label.toLowerCase()} yet.`
              : `${storedPoints.length} stored probes over the last ${range.label.toLowerCase()}.`}
          </p>
        </CardBody>
      </Card>
    </Bento>
  );
}
