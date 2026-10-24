import HeroChart from "../../components/chart/HeroChart";
import StatusBadge from "../../components/status/StatusBadge";
import type { ConnectionOverview } from "../../types/overview";
import RangeStepper from "./RangeStepper";
import { formatAge, formatLatency } from "./format";
import { HEALTH_LABEL, HEALTH_SENTENCE, HEALTH_TONE } from "./health";
import type { PlotPoint } from "../../components/chart/series";

/**
 * The dashboard's focal card: the headline latency, what it was measured
 * against, and the plain-language verdict.
 *
 * The verdict is the connection's, not the number's. It used to read "Healthy"
 * whenever a latency existed, which is a claim one reading cannot support and
 * which would be false the moment probes started failing.
 *
 * The layout is the design's, spacing and all. The card carries no padding of
 * its own: the chart is full bleed behind everything, and the padding belongs
 * to the two layers that sit over it, so the line runs edge to edge and the
 * words do not. The figure sits directly under the top row rather than being
 * pushed to the bottom, which is what leaves the lower two thirds of the card
 * for the chart to be seen in.
 */
export default function ConnectionHero({
  overview,
  points,
  rangeIndex,
  onRangeChange,
}: {
  overview: ConnectionOverview;
  points: PlotPoint[];
  rangeIndex: number;
  onRangeChange: (next: number) => void;
}) {
  const latency = overview.latency;
  const hasReading = latency.value !== null;

  // A held reading states its age, so a number from forty seconds ago cannot be
  // mistaken for one from now. Measured at render, which is when it is true.
  const age = latency.measuredAt === null ? 0 : Date.now() - latency.measuredAt;

  // The subject of the number. Never a bare latency: what it was measured
  // against is part of the reading.
  const target = overview.primaryTarget;
  const subject =
    target === null
      ? "No probe target configured"
      : `${target.method} connect to ${target.host}:${target.port}`;

  // When the readings in the range were taken. The stepper names the span
  // rather than its length, because a latency in milliseconds invites the
  // reader to treat it as a number with no time attached.
  const from = points.length > 0 ? points[0].at : null;
  const to = points.length > 0 ? points[points.length - 1].at : null;

  return (
    <article className="hero-surface relative col-span-2 flex min-h-[520px] min-w-0 flex-col overflow-hidden rounded-card border border-hairline sm:col-span-6 lg:col-span-8 lg:row-span-2">
      {/* The chart sits behind everything else in the card, masked away from
          the top-left where the headline lives, so the line never competes with
          the words. */}
      <div className="hero-chart-mask pointer-events-none absolute inset-0">
        <HeroChart points={points} />
      </div>

      <div className="relative z-[2] flex items-start justify-between gap-stack p-card-pad">
        <span className="glass-badge">
          {/* The live indicator, and never the only signal: the word beside it
              carries the same fact. */}
          <span
            className={`live-dot ${
              overview.monitoringActive && !overview.paused ? "bg-ember" : "bg-idle"
            }`}
            aria-hidden="true"
          />
          {overview.paused
            ? "Paused"
            : overview.monitoringActive
              ? "Monitoring live"
              : "Monitoring idle"}
        </span>

        <RangeStepper
          index={rangeIndex}
          onChange={onRangeChange}
          from={from}
          to={to}
        />
      </div>

      <div className="relative z-[2] mt-card-pad px-card-pad">
        <p className="text-micro uppercase tracking-[0.1em] text-ink-faint">
          Round-trip latency
        </p>

        <p className="mt-gap flex items-baseline gap-row">
          {hasReading ? (
            <>
              <span className="tnum font-display text-figure-lg font-light leading-none tracking-[-0.045em] text-ink">
                {formatLatency(latency.value as number)}
              </span>
              <span className="font-display text-title font-regular text-ink-faint">
                ms
              </span>
            </>
          ) : (
            <span className="font-display text-[1.75rem] leading-[1.2] tracking-[-0.02em] text-ink-faint">
              No reading
            </span>
          )}
        </p>

        <p className="mt-gap flex flex-wrap items-center gap-row text-small text-ink-faint">
          <span>{subject}</span>
          <StatusBadge tone={HEALTH_TONE[overview.healthState]}>
            {HEALTH_LABEL[overview.healthState]}
          </StatusBadge>
        </p>

        <p className="mt-gap text-small text-ink-muted">
          {hasReading
            ? `Measured ${formatAge(age)}. ${HEALTH_SENTENCE[overview.healthState]}`
            : "Waiting for the first successful probe."}
        </p>
      </div>
    </article>
  );
}
