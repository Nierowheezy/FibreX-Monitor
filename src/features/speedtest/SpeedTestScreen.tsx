import SpeedGauge from "../../components/chart/SpeedGauge";
import { Card, CardBody, CardHead, EmptyNote } from "../../components/ui/Card";
import {
  DownloadIcon,
  GaugeIcon,
  PauseIcon,
  PlayIcon,
  PulseIcon,
  UploadIcon,
} from "../../components/ui/icons";
import { useSpeedTest } from "../../hooks/useSpeedTest";
import { formatClock } from "../overview/format";
import type { SpeedTestPhase } from "../../types/speedtest";

/**
 * The speed test.
 *
 * A dedicated screen, because a speed test is a different job from watching
 * the connection: it is a measurement you ask for and wait for, not a reading
 * that is always there. Putting it on its own screen means running one never
 * hides the monitoring behind it.
 *
 * The figures come from a real transfer against a real host. Nothing here is
 * estimated, and a measurement that could not be taken says so rather than
 * showing a number that would be a guess.
 */
export default function SpeedTestScreen() {
  const { state, run } = useSpeedTest();

  const running = state.status === "running";
  const phase = running ? state.phase : null;
  const result = state.status === "done" && state.outcome.kind === "completed"
    ? state.outcome.result
    : null;

  // The needle shows the rate of whatever phase is running, and the download
  // rate once the test has finished. It has no rate to point at nothing
  // during the ping.
  const megabits = running
    ? state.megabits
    : result?.downloadBytesPerSecond !== null && result?.downloadBytesPerSecond !== undefined
      ? (result.downloadBytesPerSecond * 8) / 1_000_000
      : null;

  return (
    <div className="flex flex-col gap-stack">
      <Card span={12}>
        <CardHead
          icon={<GaugeIcon className="size-5" />}
          title="Speed test"
          sub="Measured against a real host over the internet"
          end={
            <button
              type="button"
              onClick={run}
              disabled={running}
              className="inline-flex cursor-pointer items-center gap-gap rounded-control bg-ember px-4 py-2 text-small font-semibold text-on-ember transition-colors duration-[var(--motion)] hover:bg-ember-bright disabled:cursor-default disabled:opacity-60"
            >
              {running ? <PauseIcon className="size-4" /> : <PlayIcon className="size-4" />}
              {running ? "Testing" : "Run test"}
            </button>
          }
        />
        <CardBody>
          <div className="flex flex-col items-center gap-stack lg:flex-row lg:items-stretch lg:gap-gutter">
            <div className="flex flex-1 flex-col items-center justify-center">
              <SpeedGauge
                megabits={megabits}
                running={running}
                unit={phase === "ping" ? "ms" : "Mbps"}
                value={
                  running && phase === "ping"
                    ? state.latencyMs === null
                      ? "--"
                      : String(Math.round(state.latencyMs))
                    : running
                      ? megabits === null
                        ? "--"
                        : formatMegabits(megabits * 1_000_000)
                      : result === null
                        ? "--"
                        : result.downloadBytesPerSecond === null
                          ? "--"
                          : formatMegabits(result.downloadBytesPerSecond * 8)
                }
                caption={
                  running && phase !== null
                    ? phase === "ping"
                      ? "Ping"
                      : phase === "download"
                        ? "Download"
                        : "Upload"
                    : "Download"
                }
              />
            </div>

            <div className="flex flex-col justify-center gap-row lg:w-[280px]">
              <Metric
                icon={<PulseIcon className="size-4" />}
                label="Ping"
                value={
                  result === null || result.latencyMs === null
                    ? null
                    : `${Math.round(result.latencyMs)}`
                }
                unit="ms"
              />
              <Metric
                icon={<PulseIcon className="size-4" />}
                label="Jitter"
                value={
                  result === null || result.jitterMs === null
                    ? null
                    : `${Math.round(result.jitterMs)}`
                }
                unit="ms"
              />
              <Metric
                icon={<DownloadIcon className="size-4" />}
                label="Download"
                value={
                  result === null || result.downloadBytesPerSecond === null
                    ? null
                    : formatMegabits(result.downloadBytesPerSecond * 8)
                }
                unit="Mbps"
              />
              <Metric
                icon={<UploadIcon className="size-4" />}
                label="Upload"
                value={
                  result === null || result.uploadBytesPerSecond === null
                    ? null
                    : formatMegabits(result.uploadBytesPerSecond * 8)
                }
                unit="Mbps"
              />
            </div>
          </div>

          {running && (
            <p role="status" className="mt-stack text-center text-small text-ink-muted">
              {phaseName(state.phase)}. Each transfer phase runs for ten
              seconds, so the whole test takes about half a minute.
            </p>
          )}

          {state.status === "idle" && (
            <p className="mt-stack text-center text-small text-ink-faint">
              No test has been run yet. One takes about twenty-five seconds,
              opens several connections at once the way speed tests do, and
              moves tens of megabytes, depending on the line.
            </p>
          )}

          {state.status === "done" && state.outcome.kind === "failed" && (
            <div className="mt-stack">
              <EmptyNote>{state.outcome.message}</EmptyNote>
            </div>
          )}

          {result !== null && (
            <p className="mt-stack text-center text-micro text-ink-faint">
              Measured against {result.host} at {formatClock(result.finishedAt)}.
              This is the line to the internet, not the router.
            </p>
          )}
        </CardBody>
      </Card>
    </div>
  );
}

/** The name of the phase, for the person waiting. */
function phaseName(phase: SpeedTestPhase): string {
  switch (phase) {
    case "ping":
      return "measuring the round trip";
    case "download":
      return "measuring the download";
    case "upload":
      return "measuring the upload";
  }
}

/**
 * One measured figure, or the reason there is not one.
 *
 * A figure that is missing says so. A zero would be a claim that the line
 * moved nothing at all, which is a different fact from the measurement not
 * having been taken.
 */
function Metric({
  icon,
  label,
  value,
  unit,
}: {
  icon: React.ReactNode;
  label: string;
  value: string | null;
  unit: string;
}) {
  return (
    <div className="hairline rounded-inner px-card-pad py-row">
      <p className="flex items-center gap-gap text-micro uppercase tracking-[0.1em] text-ink-faint">
        {icon}
        {label}
      </p>
      {value === null ? (
        <p className="mt-1 text-body text-ink-faint">No reading</p>
      ) : (
        <p className="mt-1 flex items-baseline gap-row">
          <span className="tnum font-display text-[1.75rem] font-light leading-none tracking-figure text-ink">
            {value}
          </span>
          <span className="text-small text-ink-faint">{unit}</span>
        </p>
      )}
    </div>
  );
}

/** A rate in bits per second, in the unit a reader would use. */
function formatMegabits(bitsPerSecond: number): string {
  if (bitsPerSecond >= 1_000_000) {
    return (bitsPerSecond / 1_000_000).toFixed(1);
  }
  if (bitsPerSecond >= 1_000) {
    return (bitsPerSecond / 1_000).toFixed(0);
  }
  return bitsPerSecond.toFixed(0);
}
