import { Card, CardBody, CardHead } from "../../components/ui/Card";
import { IncidentsIcon, RouterIcon } from "../../components/ui/icons";
import { formatClock } from "../overview/format";
import type { IncidentSummary } from "../../types/history";
import { describeRouter, durationOf, formatDuration, isOpen } from "./format";

/**
 * The most recent recorded outage, on the overview.
 *
 * The design gives this card the second-most prominent position on the screen,
 * and it is the right place for it: an outage is the thing a person checking
 * their connection most wants to know about, and the overview is where they
 * land.
 *
 * It reads as evidence rather than as a notification. What happened, when, how
 * long, and what the router was doing at the time. It never says why, because
 * the app did not measure why.
 */
export default function LastIncidentCard({
  incident,
  now,
}: {
  /** The newest incident, or null when nothing has been recorded. */
  incident: IncidentSummary | null;
  now: number;
}) {
  if (incident === null) {
    return (
      <Card span={4}>
        <CardHead
          icon={<IncidentsIcon className="size-5" />}
          title="Last incident"
          sub="Nothing recorded"
          end={<span className="inline-flex w-fit items-center rounded-pill bg-idle-wash px-3 py-0.5 text-micro font-semibold tracking-label text-idle">No incidents</span>}
        />
        <CardBody>
          <div className="flex items-center gap-stack">
            <span className="grid size-[40px] flex-none place-items-center rounded-chip bg-surface-3 text-ink-faint">
              <IncidentsIcon className="size-4" />
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-title font-medium tracking-title text-ink">
                No incidents recorded
              </p>
              <p className="mt-0.5 text-small text-ink-faint">
                Nothing has been measured yet
              </p>
              <p className="mt-0.5 text-small text-ink-faint">
                History starts when monitoring starts
              </p>
            </div>
          </div>

          <p className="hairline mt-stack rounded-inner px-card-pad py-gutter text-center text-small text-ink-faint">
            No outages or degradations yet
          </p>
        </CardBody>
      </Card>
    );
  }

  const open = isOpen(incident);
  const duration = formatDuration(durationOf(incident, now));

  return (
    <Card span={4}>
      <CardHead
        icon={<IncidentsIcon className="size-5" />}
        title="Last incident"
        sub={formatClock(incident.startedAt)}
        end={
          <span
            className={`inline-flex w-fit items-center rounded-pill px-3 py-0.5 text-micro font-semibold tracking-label ${
              open ? "bg-bad-wash text-bad" : "bg-idle-wash text-idle"
            }`}
          >
            {open ? "Ongoing" : "Recovered"}
          </span>
        }
      />
      <CardBody>
        <div className="flex items-center gap-stack">
          <span className="grid size-[40px] flex-none place-items-center rounded-chip bg-surface-3 text-ink-faint">
            <IncidentsIcon className="size-4" />
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-title font-medium tracking-title text-ink">
              {open ? "Failing now" : "Probes did not connect"}
            </p>
            <p className="mt-0.5 text-small text-ink-faint">
              {duration} &middot; confirmed by probes
            </p>
            <p className="mt-0.5 text-small text-ink-faint">
              {describeRouter(incident)}
            </p>
          </div>
        </div>

        <div className="mt-gap flex items-center justify-between gap-row font-mono text-micro text-ink-faint">
          <span>Started {formatClock(incident.startedAt)}</span>
          <span>
            {open ? "Not recovered" : `Recovered ${formatClock(incident.recoveredAt)}`}
          </span>
        </div>

        <div className="mt-stack flex flex-wrap gap-gap">
          <span className="inline-flex items-center gap-gap rounded-chip bg-surface-2 px-3 py-[7px] text-label text-ink-muted">
            <RouterIcon className="size-3.5" />
            {describeRouter(incident)}
          </span>
          <span className="inline-flex items-center gap-gap rounded-chip bg-surface-2 px-3 py-[7px] text-label text-ink-muted">
            {duration}
          </span>
        </div>
      </CardBody>
    </Card>
  );
}
