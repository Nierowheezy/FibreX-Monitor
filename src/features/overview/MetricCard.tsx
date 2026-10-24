import type { ReactNode } from "react";

import StatusBadge from "../../components/status/StatusBadge";
import type { BadgeTone } from "../../components/status/types";
import { Card, CardBody, CardHead, Figure } from "../../components/ui/Card";
import type { MetricValue, ProbeFailureBasis, ValueState } from "../../types/overview";
import { formatAge } from "./format";
import { STATE_LABEL, STATE_NOTE, STATE_TONE } from "./state";

type MetricCardProps = {
  icon: ReactNode;
  title: string;
  sub?: ReactNode;
  /** Set on the head, right-aligned: a window, a host, a control. */
  end?: ReactNode;
  metric: MetricValue<number>;
  unit?: string;
  /** A small word beneath the figure, for a figure that would read bare. */
  caption?: ReactNode;
  /** Foot text, overriding the state note. Only shows when there is a figure. */
  foot?: ReactNode;
  /** A replacement for the provenance badge, for a card whose state is a verdict. */
  badge?: ReactNode;
  /** The counts behind the failure figure, when there are any. */
  basis?: ProbeFailureBasis | null;
  /**
   * `null` means the formatter could not vouch for a figure, which the card
   * renders exactly like having no reading at all.
   */
  format: (value: number, basis: ProbeFailureBasis | null) => string | null;
  span?: 2 | 3 | 4 | 6 | 8 | 12;
  rowSpan?: number;
};

/**
 * One measured metric as a bento card.
 *
 * The rule that matters lives here: a metric with no reading renders the words
 * "No reading" and never a zero, because a zero would read as a real measurement
 * of zero rather than the absence of one.
 *
 * A stale metric keeps its number and states its age instead of blinking out, so
 * a brief blip and a dead connection do not look the same on screen.
 */
export default function MetricCard({
  icon,
  title,
  sub,
  end,
  metric,
  unit,
  caption,
  foot,
  basis,
  badge,
  format,
  span = 4,
  rowSpan,
}: MetricCardProps) {
  // A figure the formatter refuses to stand behind is no figure, so the card
  // falls back to its empty rendering in full: no value, no caption, and no foot
  // naming a window the value does not actually sit in.
  const figure = metric.value === null ? null : format(metric.value, basis ?? null);
  const hasReading = figure !== null;
  const state: ValueState = hasReading ? metric.state : "unavailable";

  const stateNote =
    state === "stale" && metric.measuredAt !== null
      ? `Measured ${formatAge(Date.now() - metric.measuredAt)}`
      : STATE_NOTE[state];

  return (
    <Card span={span} rowSpan={rowSpan}>
      <CardHead icon={icon} title={title} sub={sub} end={end} />
      <CardBody>
        <Figure
          value={figure}
          unit={unit}
          caption={hasReading ? caption : undefined}
        />
        <div className="mt-stack flex items-center justify-between gap-row text-micro text-ink-faint">
          <span>{hasReading && foot !== undefined ? foot : stateNote}</span>
          {badge ?? (
            <StatusBadge tone={STATE_TONE[state]}>{STATE_LABEL[state]}</StatusBadge>
          )}
        </div>
      </CardBody>
    </Card>
  );
}

/**
 * A card whose figure is a word rather than a measurement: a verdict, a state,
 * a count of things rather than a quantity.
 *
 * Kept apart from [`MetricCard`] on purpose, because a `MetricValue` is the
 * shape for "we measured a number", and forcing a sentence through it would put
 * a `ValueState` on something that was never measured.
 */
export function WordCard({
  icon,
  title,
  sub,
  end,
  figure,
  badge,
  children,
  span = 4,
  rowSpan,
}: {
  icon: ReactNode;
  title: string;
  sub?: ReactNode;
  end?: ReactNode;
  figure: string;
  badge?: { tone: BadgeTone; label: string };
  children?: ReactNode;
  span?: 2 | 3 | 4 | 6 | 8 | 12;
  rowSpan?: number;
}) {
  return (
    <Card span={span} rowSpan={rowSpan}>
      <CardHead icon={icon} title={title} sub={sub} end={end} />
      <CardBody>
        <p className="font-display text-display tracking-display text-ink">{figure}</p>
        {children !== undefined && (
          <p className="mt-gap text-small text-ink-faint">{children}</p>
        )}
        {badge !== undefined && (
          <div className="mt-stack flex items-center gap-row">
            <StatusBadge tone={badge.tone}>{badge.label}</StatusBadge>
          </div>
        )}
      </CardBody>
    </Card>
  );
}
