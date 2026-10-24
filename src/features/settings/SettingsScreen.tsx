import { useState } from "react";

import StatusBadge from "../../components/status/StatusBadge";
import { Bento, Card, CardBody, CardHead, EmptyNote } from "../../components/ui/Card";
import SegmentedControl from "../../components/ui/SegmentedControl";
import {
  ClockIcon,
  GlobeIcon,
  PauseIcon,
  PlayIcon,
  SettingsIcon,
  ShieldIcon,
} from "../../components/ui/icons";
import { useConnectionOverview } from "../../hooks/useConnectionOverview";
import { useHistory } from "../../hooks/useHistory";
import AppearanceMenu from "../../app/AppearanceMenu";
import { clockFormatLabel } from "../../app/clock";
import { useClockFormat } from "../../hooks/useClockFormat";
import { useTheme } from "../../hooks/useTheme";
import RouterConnectionCard from "./RouterConnectionCard";
import { setMonitoringPaused } from "../../lib/tauri/overview";
import { isTauri } from "../../lib/tauri/environment";
import type { ProbeTargetSummary } from "../../types/overview";

const APPEARANCE_LABEL: Record<string, string> = {
  system: "System",
  light: "Light",
  dark: "Dark",
};

/** One read-only fact, in a label above its value. */
function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="label-caps">{label}</dt>
      <dd className="text-small text-ink">{value}</dd>
    </div>
  );
}

/**
 * Every setting the app actually has.
 *
 * Appearance, clock format, pause, and the router connection are live here,
 * each applying as soon as it is changed. The probe target, interval and
 * timeout remain compiled in and read-only: the app measures with one fixed
 * target, so offering an editable field would be a control that changes
 * nothing.
 */
export default function SettingsScreen() {
  const [theme] = useTheme();
  const [clockFormat, setClockFormat] = useClockFormat();
  const [refreshKey, setRefreshKey] = useState(0);
  const state = useConnectionOverview({ refreshKey });
  const history = useHistory();
  const target: ProbeTargetSummary | null =
    state.status === "ready" ? state.overview.primaryTarget : null;
  const paused = state.status === "ready" ? state.overview.paused : false;

  return (
    <Bento>
      <Card span={6}>
        <CardHead
          icon={<SettingsIcon className="size-5" />}
          title="Appearance"
          sub="Applies to the window and the menu-bar panel"
          end={<AppearanceMenu />}
        />
        <CardBody>
          <p className="flex items-baseline gap-row">
            <span className="font-display text-display tracking-display text-ink">
              {APPEARANCE_LABEL[theme.appearance]}
            </span>
          </p>
          <p className="mt-gap text-small text-ink-faint">
            System follows your Mac's light and dark setting.
          </p>
        </CardBody>
      </Card>

      <Card span={6}>
        <CardHead
          icon={<ShieldIcon className="size-5" />}
          title="Privacy"
          sub="Local only"
        />
        <CardBody>
          <p className="flex items-baseline gap-row">
            <span className="font-display text-display tracking-display text-ink">
              No account
            </span>
          </p>
          <p className="mt-gap text-small text-ink-faint">
            Everything stays on this Mac. There is no cloud service, no sign-in,
            and no telemetry.
          </p>
        </CardBody>
      </Card>

      <Card span={6}>
        <CardHead
          icon={<GlobeIcon className="size-5" />}
          title="Probe target"
          sub="Compiled in"
        />
        <CardBody>
          <dl className="flex flex-col gap-stack">
            <Field
              label="Target"
              value={target === null ? "Not configured" : `${target.host}:${target.port}`}
            />
            <Field label="Method" value={target?.method ?? "TCP"} />
            <Field
              label="Interval"
              value={
                target === null ? "Not configured" : `${target.intervalMs / 1000} seconds`
              }
            />
            <Field
              label="Timeout"
              value={target === null ? "Not configured" : `${target.timeoutMs} ms`}
            />
          </dl>
        </CardBody>
      </Card>

      <RouterConnectionCard />

      <Card span={6}>
        <CardHead
          icon={<ClockIcon className="size-5" />}
          title="History retention"
          sub="Local only, on this Mac"
          end={
            history.status === "ready" && history.info.enabled ? (
              <StatusBadge tone="ok">Recording</StatusBadge>
            ) : (
              <StatusBadge tone="neutral">Off</StatusBadge>
            )
          }
        />
        <CardBody>
          {history.status !== "ready" || !history.info.enabled ? (
            <EmptyNote>
              History is not being recorded. Readings are held in memory and are
              lost when the app quits.
            </EmptyNote>
          ) : (
            <dl className="flex flex-col gap-stack">
              <div className="flex flex-col gap-1">
                <dt className="label-caps">Probes stored</dt>
                <dd className="tnum text-small text-ink">{history.info.sampleCount}</dd>
              </div>
              <div className="flex flex-col gap-1">
                <dt className="label-caps">Kept for</dt>
                <dd className="text-small text-ink">
                  {Math.round(history.info.retentionMs / 86_400_000)} days, then the
                  oldest samples are dropped.
                </dd>
              </div>
            </dl>
          )}
        </CardBody>
      </Card>

      <Card span={6}>
        <CardHead
          icon={<ClockIcon className="size-5" />}
          title="Clock"
          sub="How times are written"
        />
        <CardBody>
          <p className="text-small text-ink-muted">
            System follows your Mac's setting. The choice applies everywhere a
            time appears, including the incident times and the range the
            latency history covers.
          </p>
          <div className="mt-stack">
            <SegmentedControl
              label="Clock format"
              value={clockFormat}
              onChange={setClockFormat}
              options={[
                { value: "system", label: clockFormatLabel("system") },
                { value: "24h", label: clockFormatLabel("24h") },
                { value: "12h", label: clockFormatLabel("12h") },
              ]}
            />
          </div>
        </CardBody>
      </Card>

      <Card span={6}>
        <CardHead
          icon={<SettingsIcon className="size-5" />}
          title="Monitoring"
          sub="Runs in the background after the window closes"
          end={
            state.status === "ready" ? (
              paused ? (
                <StatusBadge tone="neutral">Paused</StatusBadge>
              ) : (
                <StatusBadge tone="ok">Running</StatusBadge>
              )
            ) : (
              <StatusBadge tone="neutral">Unknown</StatusBadge>
            )
          }
        />
        <CardBody>
          <p className="text-small text-ink-muted">
            A pause keeps the loop alive and stops the probes. The readings taken
            before the pause stay in the history, and a resume picks up from
            there.
          </p>
          <button
            type="button"
            onClick={() => {
              if (!isTauri()) {
                return;
              }
              void setMonitoringPaused(!paused).then(() => {
                setRefreshKey((key) => key + 1);
              });
            }}
            className="mt-stack inline-flex cursor-pointer items-center gap-gap rounded-control bg-surface-2 px-4 py-2.5 text-small font-semibold text-ink transition-colors duration-[var(--motion)] hover:bg-surface-3"
          >
            {paused ? <PlayIcon className="size-4" /> : <PauseIcon className="size-4" />}
            {paused ? "Resume monitoring" : "Pause monitoring"}
          </button>
        </CardBody>
      </Card>
    </Bento>
  );
}
