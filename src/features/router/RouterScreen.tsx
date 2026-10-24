import LatencyChart from "../../components/chart/LatencyChart";
import StatusBadge from "../../components/status/StatusBadge";
import {
  Bento,
  Card,
  CardBody,
  CardHead,
  EmptyNote,
  Figure,
} from "../../components/ui/Card";
import {
  DownloadIcon,
  GlobeIcon,
  OpticalIcon,
  RouterIcon,
  SignalIcon,
  SystemIcon,
  TrafficIcon,
  UploadIcon,
} from "../../components/ui/icons";
import { useConnectionOverview } from "../../hooks/useConnectionOverview";
import { useLatencySeries } from "../../hooks/useLatencySeries";
import { useRouterReport } from "../../hooks/useRouterReport";
import type { ConnectionOverview, RouterReport, WanConnection } from "../../types/overview";
import { formatAge, formatClock, formatLatency, formatProbeFailures } from "../overview/format";
import MetricCard from "../overview/MetricCard";
import {
  ADDRESS_KIND_LABEL,
  describeRouterReport,
  firstFailedSource,
  formatBytes,
  formatDuration,
  formatOpticalDbm,
  formatPercent,
  formatRate,
} from "./format";

const REFRESH_MS = 2_000;

/**
 * The report shape before anything has been read.
 *
 * The same shape the backend returns for an unconfigured router, so a screen
 * that has no report yet renders the same way as one whose router is not
 * configured. Both mean the same thing to a reader: there is no reading.
 */
const EMPTY_REPORT: RouterReport = {
  snapshot: null,
  error: null,
  configured: false,
  attempted: false,
  traffic: null,
};

function RouterBadge({ reachable }: { reachable: boolean | null }) {
  if (reachable === null) return <StatusBadge tone="neutral">No reading</StatusBadge>;
  return reachable ? (
    <StatusBadge tone="ok">Reachable</StatusBadge>
  ) : (
    <StatusBadge tone="error">Unreachable</StatusBadge>
  );
}

/** A labelled row of router detail, or an honest empty when the router omitted it. */
function DetailRow({ label, value }: { label: string; value: string | null }) {
  return (
    <div className="flex items-baseline justify-between gap-row py-1">
      <span className="text-small text-ink-faint">{label}</span>
      {value === null ? (
        <span className="text-small text-ink-faint">No reading</span>
      ) : (
        <span className="text-small tabular-nums text-ink">{value}</span>
      )}
    </div>
  );
}

/** Which half of the router read a WAN card is describing, for its foot line. */
function wanFoot(report: RouterReport): string {
  if (report.snapshot === null) return "No router reading";
  return `Read ${formatAge(Date.now() - report.snapshot.readAt)}`;
}

/**
 * The WAN connection card, from the router's own report.
 *
 * Every field is separately optional, so the card shows what the router gave and
 * an explicit "No reading" line for what it did not. Nothing here is defaulted:
 * a missing gateway is not rendered as `0.0.0.0`, and a missing up time is not
 * rendered as a fresh zero-second session.
 */
function WanCard({ report, span }: { report: RouterReport; span: 6 | 12 }) {
  const wan: WanConnection | null = report.snapshot?.wan ?? null;
  const connected = wan?.status?.toLowerCase() === "connected";

  const badge = (() => {
    if (wan?.status == null) return <StatusBadge tone="neutral">No reading</StatusBadge>;
    return connected ? (
      <StatusBadge tone="ok">{wan.status}</StatusBadge>
    ) : (
      <StatusBadge tone="warn">{wan.status}</StatusBadge>
    );
  })();

  return (
    <Card span={span}>
      <CardHead
        icon={<GlobeIcon className="size-5" />}
        title="WAN connection"
        sub={report.snapshot === null ? "Read-only" : `Reported by ${report.snapshot.auth.host}`}
        end={badge}
      />
      <CardBody>
        {wan === null ? (
          <EmptyNote>{describeRouterReport(report)}</EmptyNote>
        ) : (
          <div className="divide-y divide-hairline">
            <DetailRow
              label="Address"
              value={
                wan.ipAddress === null
                  ? null
                  : `${wan.ipAddress}${wan.addressKind === null ? "" : ` (${ADDRESS_KIND_LABEL[wan.addressKind]})`}`
              }
            />
            <DetailRow label="Gateway" value={wan.gateway} />
            <DetailRow
              label="DNS"
              value={wan.dnsServers.length > 0 ? wan.dnsServers.join(", ") : null}
            />
            <DetailRow label="MTU" value={wan.mtu === null ? null : String(wan.mtu)} />
            <DetailRow label="Access server" value={wan.bras} />
            {wan.accessType !== null && (
              <DetailRow label="Access type" value={wan.accessType} />
            )}
            <DetailRow label="Session up" value={formatDuration(wan.uptimeSeconds)} />
          </div>
        )}
        <p className="mt-stack text-micro text-ink-faint">{wanFoot(report)}</p>
      </CardBody>
    </Card>
  );
}

/**
 * The router's own health figures.
 *
 * The optical receive power is the one optical figure the Observed firmware
 * reports; transmit power, temperature, voltage and bias current are not
 * reported at all, so no field exists for them and none is shown. CPU and memory
 * appear only when the firmware says its utilisation section is switched on,
 * which is why they are labelled as conditional rather than always present.
 */
function HealthCard({ report, span }: { report: RouterReport; span: 6 | 12 }) {
  const health = report.snapshot?.health ?? null;
  const failedSource = firstFailedSource(report);

  return (
    <Card span={span}>
      <CardHead
        icon={<OpticalIcon className="size-5" />}
        title="Optical signal"
        sub="Receive power as the router reports it"
      />
      <CardBody>
        {health === null ? (
          <EmptyNote>{describeRouterReport(report)}</EmptyNote>
        ) : (
          <>
            <Figure
              value={
                health.opticalRxDbm.value === null
                  ? null
                  : formatOpticalDbm(health.opticalRxDbm.value)
              }
              unit="dBm"
              caption={
                health.opticalRxDbm.value === null
                  ? undefined
                  : "Receive power"
              }
            />
            <div className="mt-stack flex items-center justify-between gap-row text-micro text-ink-faint">
              <span>
                {health.opticalRxDbm.value !== null
                  ? wanFoot(report)
                  : health.reportsOptical === false
                    ? "This firmware does not report an optical figure"
                    : failedSource !== null
                      ? `${failedSource} could not be read`
                      : "Nothing measured"}
              </span>
              <StatusBadge
                tone={health.opticalRxDbm.value !== null ? "ok" : "neutral"}
              >
                {health.opticalRxDbm.value !== null ? "Measured" : "No reading"}
              </StatusBadge>
            </div>
            {health.ponMode !== null && (
              <p className="mt-gap text-small text-ink-faint">
                PON mode: <span className="text-ink-muted">{health.ponMode}</span>
              </p>
            )}
          </>
        )}
      </CardBody>
    </Card>
  );
}

/**
 * The router's own view of the network around it.
 *
 * The Wi-Fi radios, the access type of the WAN, and the IPv6 state. Every
 * figure here is the router's own word for it, so the IPv6 status is shown as
 * the router says it rather than as an interpretation of it.
 */
function NetworkCard({ report, span }: { report: RouterReport; span: 6 | 12 }) {
  const network = report.snapshot?.network ?? null;

  const radios = network?.wifiRadios ?? [];
  const radioText =
    radios.length === 0
      ? null
      : radios.map((radio) => `${radio.name}${radio.enabled ? "" : " (off)"}`).join(", ");

  return (
    <Card span={span}>
      <CardHead
        icon={<SignalIcon className="size-5" />}
        title="Network"
        sub="The router's own view of the line and its radios"
      />
      <CardBody>
        {network === null ? (
          <EmptyNote>{describeRouterReport(report)}</EmptyNote>
        ) : (
          <div className="divide-y divide-hairline">
            <DetailRow label="WAN access type" value={network.wanAccessType} />
            <DetailRow label="Wi-Fi radios" value={radioText} />
            <DetailRow
              label="IPv6"
              value={
                network.ipv6Status === null
                  ? network.supportsIpv6 === true
                    ? "Supported, not reported"
                    : null
                  : network.ipv6Status
              }
            />
            {network.ipv6Prefixes.length > 0 && (
              <DetailRow
                label="IPv6 prefixes"
                value={network.ipv6Prefixes.join(", ")}
              />
            )}
          </div>
        )}
        <p className="mt-stack text-micro text-ink-faint">
          Read from the router, read-only. Nothing here was changed.
        </p>
      </CardBody>
    </Card>
  );
}

export default function RouterScreen() {
  const state = useConnectionOverview({ pollMs: REFRESH_MS });
  const series = useLatencySeries(REFRESH_MS);
  const routerReport = useRouterReport(30_000);

  if (state.status !== "ready") {
    return (
      <Card span={12}>
        <CardHead
          icon={<RouterIcon className="size-5" />}
          title="Fibre and router"
          sub={
            state.status === "loading"
              ? "Reading router status..."
              : "The monitoring backend did not answer."
          }
        />
        {state.status === "error" && (
          <CardBody>
            <p className="text-small text-ink-muted">{state.message}</p>
          </CardBody>
        )}
      </Card>
    );
  }

  const overview: ConnectionOverview = state.overview;
  const router = overview.router;
  const target = overview.primaryTarget;
  const report = routerReport.status === "ready" ? routerReport.report : null;

  return (
    <Bento>
      <MetricCard
        span={4}
        icon={<RouterIcon className="size-5" />}
        title="Router reachability"
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
        span={4}
        icon={<GlobeIcon className="size-5" />}
        title="Internet target"
        sub={
          target === null
            ? "No target configured"
            : `${target.method} ${target.host}:${target.port}`
        }
        metric={overview.latency}
        unit="ms"
        foot={`Last successful probe ${formatClock(overview.lastProbeAt)}`}
        format={formatLatency}
      />

      <MetricCard
        span={4}
        icon={<SignalIcon className="size-5" />}
        title="Probe failures"
        sub="Connects that did not open"
        metric={overview.packetLoss}
        caption="failed"
        foot="Last 5 minutes"
        basis={overview.probeFailureBasis}
        format={formatProbeFailures}
      />

      <Card span={12}>
        <CardHead
          icon={<SignalIcon className="size-5" />}
          title="Line latency"
          sub="Round trip to the internet target. Gaps are probes that failed."
        />
        <CardBody>
          <LatencyChart points={series.status === "ready" ? series.points : []} />
          <p className="mt-stack text-micro text-ink-faint">
            The live probe series, since this launch. The stored history over
            longer ranges is on the Overview.
          </p>
        </CardBody>
      </Card>

      <WanCard
        report={report ?? EMPTY_REPORT}
        span={6}
      />

      <HealthCard report={report ?? EMPTY_REPORT} span={6} />

      <NetworkCard report={report ?? EMPTY_REPORT} span={6} />

      <RouterSessionCard report={report} />

      <Card span={6}>
        <CardHead
          icon={<TrafficIcon className="size-5" />}
          title="WAN traffic"
          sub="From the router's byte counters"
        />
        <CardBody>
          <TrafficBody report={report} />
        </CardBody>
      </Card>

      <SystemCard report={report} />
    </Bento>
  );
}

/**
 * The traffic figures, or the reason there are none.
 *
 * The reason matters as much as the number. "Waiting for a second reading",
 * "the router reset the counter" and "no router session" are different facts
 * that need different actions, so each is named rather than collapsed into a
 * blank.
 */
function TrafficBody({ report }: { report: RouterReport | null }) {
  const traffic = report?.traffic ?? null;
  const counters = report?.snapshot?.counters ?? null;

  if (traffic === null) {
    return (
      <EmptyNote>
        {report === null
          ? "Not measured. Traffic needs a signed-in read of the router's counters."
          : "Waiting for a second reading. A rate needs two readings of the router's counters."}
      </EmptyNote>
    );
  }

  if (traffic.receivedBytesPerSecond === null) {
    return <EmptyNote>{traffic.note ?? "No rate available."}</EmptyNote>;
  }

  return (
    <>
      <div className="flex items-baseline justify-between gap-row">
        <span className="flex items-center gap-gap text-small text-ink-muted">
          <DownloadIcon className="size-4" />
          Download
        </span>
        <span className="tnum font-display text-figure font-light text-ink">
          {formatRate(traffic.receivedBytesPerSecond)}
        </span>
      </div>
      <div className="mt-row flex items-baseline justify-between gap-row">
        <span className="flex items-center gap-gap text-small text-ink-muted">
          <UploadIcon className="size-4" />
          Upload
        </span>
        <span className="tnum font-display text-figure font-light text-ink">
          {formatRate(traffic.sentBytesPerSecond)}
        </span>
      </div>
      {counters !== null && (
        <p className="mt-stack text-micro text-ink-faint">
          {formatBytes(counters.bytesReceived)} down, {formatBytes(counters.bytesSent)} up
          since the WAN session began. The rate is calculated from two readings of
          those counters.
        </p>
      )}
    </>
  );
}

/**
 * Whether the app is signed in, and what it read.
 *
 * The sentence is the point. "No credentials configured" and "the router refused
 * the sign-in" both leave the screen blank, but they need completely different
 * fixes, so the card names which one happened rather than showing a generic
 * failure.
 */
function RouterSessionCard({ report }: { report: RouterReport | null }) {
  const snapshot = report?.snapshot ?? null;

  return (
    <Card span={6}>
      <CardHead
        icon={<RouterIcon className="size-5" />}
        title="Router session"
        sub="Read-only"
        end={
          snapshot !== null ? (
            <StatusBadge tone="ok">Signed in</StatusBadge>
          ) : report?.configured ? (
            <StatusBadge tone="neutral">No session</StatusBadge>
          ) : (
            <StatusBadge tone="neutral">Not configured</StatusBadge>
          )
        }
      />
      <CardBody>
        {report === null ? (
          <EmptyNote>Reading the router session...</EmptyNote>
        ) : (
          <>
            <p className="text-small text-ink-muted">{describeRouterReport(report)}</p>
            {snapshot !== null && (
              <div className="mt-stack divide-y divide-hairline">
                <DetailRow label="Address" value={`${snapshot.auth.host}:${snapshot.auth.port}`} />
                <DetailRow label="User" value={snapshot.auth.username} />
                <DetailRow
                  label="Password"
                  value={snapshot.auth.hasPassword ? "Configured" : "Not set"}
                />
                <DetailRow label="Last read" value={formatClock(snapshot.readAt)} />
              </div>
            )}
          </>
        )}
        <p className="mt-stack text-micro text-ink-faint">
          Reachability above needs no session and keeps working without one. The
          app never writes to the router.
        </p>
      </CardBody>
    </Card>
  );
}

/**
 * The router's own system figures, where it reports them.
 *
 * Kept as its own card because both figures are conditional on a firmware flag.
 * When the flag is off the card says the router does not report them, which is a
 * different sentence from "we could not read them", and only one of the two is
 * true at any time.
 */
function SystemCard({ report }: { report: RouterReport | null }) {
  const health = report?.snapshot?.health ?? null;
  const failedSource = report === null ? null : firstFailedSource(report);

  const note = (() => {
    if (report === null) return "Reading the router...";
    if (health === null) return describeRouterReport(report);
    if (health.reportsUtilisation === false) {
      return "This firmware has its CPU and memory section switched off, so it does not report them.";
    }
    if (failedSource !== null) return `${failedSource} could not be read.`;
    return "Nothing measured.";
  })();

  return (
    <Card span={6}>
      <CardHead
        icon={<SystemIcon className="size-5" />}
        title="Router system"
        sub="CPU and memory, where reported"
      />
      <CardBody>
        {health === null || health.cpuPercent.value === null ? (
          <EmptyNote>{note}</EmptyNote>
        ) : (
          <div className="divide-y divide-hairline">
            <DetailRow label="CPU" value={`${formatPercent(health.cpuPercent.value)}%`} />
            <DetailRow
              label="Memory"
              value={
                health.memoryPercent.value === null
                  ? null
                  : `${formatPercent(health.memoryPercent.value)}%`
              }
            />
          </div>
        )}
      </CardBody>
    </Card>
  );
}
