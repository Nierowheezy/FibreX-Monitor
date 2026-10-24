import StatusBadge from "../../components/status/StatusBadge";
import type { IncidentSummary } from "../../types/history";
import {
  describeIncident,
  describeRouter,
  isOpen,
} from "./format";
import { formatClock } from "../overview/format";

/**
 * One recorded outage, as a row.
 *
 * The row reads as a piece of evidence rather than a notification: what
 * happened, when, how long, and what the router was doing at the time. It never
 * says why, because the app did not measure why.
 */
export default function IncidentRow({
  incident,
  now,
}: {
  incident: IncidentSummary;
  now: number;
}) {
  const open = isOpen(incident);

  return (
    <li className="hairline-b flex flex-col gap-gap py-row last:border-b-0">
      <div className="flex flex-wrap items-baseline gap-x-gap gap-y-1">
        <span className="tnum font-mono text-small text-ink">
          {formatClock(incident.startedAt)}
        </span>

        <span className="text-small text-ink-muted">{describeIncident(incident, now)}</span>

        <span className="ml-auto">
          {open ? (
            <StatusBadge tone="error">Ongoing</StatusBadge>
          ) : (
            <StatusBadge tone="ok">Recovered</StatusBadge>
          )}
        </span>
      </div>

      <div className="flex flex-wrap items-baseline gap-x-gap gap-y-1 text-micro text-ink-faint">
        <span>
          {incident.probeCount} failed probe{incident.probeCount === 1 ? "" : "s"}
        </span>
        <span aria-hidden="true">·</span>
        <span>{describeRouter(incident)}</span>
        {!open && incident.recoveredAt !== null && (
          <>
            <span aria-hidden="true">·</span>
            <span>Recovered {formatClock(incident.recoveredAt)}</span>
          </>
        )}
      </div>
    </li>
  );
}
