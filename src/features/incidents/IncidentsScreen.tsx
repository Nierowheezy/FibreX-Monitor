import { Bento, Card, CardBody, CardHead, EmptyNote, Figure } from "../../components/ui/Card";
import { ClockIcon, IncidentsIcon, ShieldIcon } from "../../components/ui/icons";
import { useHistory } from "../../hooks/useHistory";
import { formatAge, formatClock } from "../overview/format";
import IncidentRow from "./IncidentRow";
import { formatDuration, summariseIncidents } from "./format";

const REFRESH_MS = 15_000;

/**
 * Outages and their recovery times, read from local storage.
 *
 * Every figure on this screen comes from probes the app actually made and wrote
 * to disk. Nothing is reconstructed, and nothing is inferred: an incident is a
 * run of failed connects with a start and an end, and that is exactly what is
 * shown.
 *
 * The three empty states are deliberately different, because they mean
 * different things and merging them would be the screen's one real chance to
 * mislead. "History is off" is not "nothing has gone wrong yet".
 */
export default function IncidentsScreen() {
  const state = useHistory(REFRESH_MS);
  const now = Date.now();

  if (state.status === "loading") {
    return (
      <Bento>
        <Card span={12}>
          <CardHead
            icon={<IncidentsIcon className="size-5" />}
            title="Incident timeline"
            sub="Reading local history"
          />
        </Card>
      </Bento>
    );
  }

  if (state.status === "error") {
    return (
      <Bento>
        <Card span={12}>
          <CardHead
            icon={<IncidentsIcon className="size-5" />}
            title="Incident history unavailable"
            sub="The monitoring backend did not answer."
          />
          <CardBody>
            <p className="text-small text-ink-muted">{state.message}</p>
          </CardBody>
        </Card>
      </Bento>
    );
  }

  const { incidents, info } = state;
  const summary = summariseIncidents(incidents, now);

  return (
    <Bento>
      <Card span={8}>
        <CardHead
          icon={<IncidentsIcon className="size-5" />}
          title="Incident timeline"
          sub={
            incidents.length === 0
              ? "No outages recorded yet"
              : `Newest first. ${incidents.length} recorded.`
          }
        />
        <CardBody>
          {incidents.length === 0 ? (
            <EmptyNote>
              {info.enabled
                ? info.sampleCount === 0
                  ? "Nothing recorded yet. Incidents appear once probing has run long enough to record one, which takes two consecutive failed probes."
                  : "No outages recorded in the stored history. That means every run of probes recovered before it became an outage."
                : "History is switched off, so nothing is being recorded. Monitoring is still running."}
            </EmptyNote>
          ) : (
            // The timeline is the screen's list, and the stored history only
            // grows. A fixed-height scroll region contains it, so the screen
            // keeps its shape and the oldest rows become reachable where they
            // already sit rather than by the page growing under the rail.
            <div className="scroll-region max-h-[320px] flex flex-col overflow-y-auto pr-gap">
              <ul className="flex flex-col">
                {incidents.map((incident) => (
                  <IncidentRow key={incident.id} incident={incident} now={now} />
                ))}
              </ul>
            </div>
          )}
        </CardBody>
      </Card>

      <Card span={4} className="self-start">
        <CardHead
          icon={<ClockIcon className="size-5" />}
          title="Recorded outages"
          sub="History"
        />
        <CardBody>
          <Figure
            value={summary === null ? null : String(summary.count)}
            caption={summary === null ? undefined : "outages"}
          />
          <p className="mt-stack text-small text-ink-faint">
            {summary === null
              ? "Nothing recorded yet."
              : `Longest ${formatDuration(summary.longestMs)}. Total ${formatDuration(
                  summary.totalMs,
                )} across all of them.`}
          </p>
          {summary !== null && summary.unobserved > 0 && (
            <p className="mt-gap text-small text-ink-faint">
              {summary.unobserved === 1
                ? "One ended when monitoring stopped, so its duration is the part that was watched."
                : `${summary.unobserved} ended when monitoring stopped, so their durations are the parts that were watched.`}
            </p>
          )}
          {summary !== null && summary.open > 0 && (
            <p className="mt-gap text-small text-bad">
              {summary.open === 1 ? "One is still running." : `${summary.open} are still running.`}
            </p>
          )}
        </CardBody>
      </Card>

      <Card span={6}>
        <CardHead
          icon={<ClockIcon className="size-5" />}
          title="History"
          sub="What is stored on this Mac"
        />
        <CardBody>
          {!info.enabled ? (
            <EmptyNote>
              History is switched off. The database could not be opened, so
              readings are held in memory and lost when the app quits.
            </EmptyNote>
          ) : (
            <dl className="flex flex-col gap-stack">
              <div className="flex flex-col gap-1">
                <dt className="label-caps">Probes stored</dt>
                <dd className="tnum text-small text-ink">{info.sampleCount}</dd>
              </div>
              <div className="flex flex-col gap-1">
                <dt className="label-caps">Oldest sample</dt>
                <dd className="text-small text-ink">
                  {info.earliestSampleAt === null
                    ? "None yet"
                    : `${formatClock(info.earliestSampleAt)}, ${formatAge(
                        now - info.earliestSampleAt,
                      )}`}
                </dd>
              </div>
              <div className="flex flex-col gap-1">
                <dt className="label-caps">Kept for</dt>
                <dd className="text-small text-ink">
                  {Math.round(info.retentionMs / 86_400_000)} days, then the oldest are dropped
                </dd>
              </div>
            </dl>
          )}
        </CardBody>
      </Card>

      <Card span={6}>
        <CardHead
          icon={<ShieldIcon className="size-5" />}
          title="How an incident is described"
          sub="Evidence only"
        />
        <CardBody>
          <p className="text-small text-ink-muted">
            An incident records when probes started failing, when they recovered,
            how long that lasted, and whether the router stayed reachable through
            it. If probing stops — paused, quit, or the Mac slept — the incident
            ends at the last probe that was actually seen and is marked
            unobserved, so an unwatched gap is never counted as downtime. It does
            not name a cause. A failed probe says a connection did not open; it
            does not say why, and it is not evidence against any provider.
          </p>
        </CardBody>
      </Card>
    </Bento>
  );
}
