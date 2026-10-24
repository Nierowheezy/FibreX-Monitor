import { ChevronLeftIcon, ChevronRightIcon } from "../../components/ui/icons";
import { formatClockMinute } from "./format";
import { HISTORY_RANGES } from "./ranges";

/**
 * The range stepper: two arrows and the span of time in force.
 *
 * The design's control, and a better one than a slider here. A slider can land
 * on a range nobody can read, and it hides which range is in force. Arrows and
 * a label say both.
 *
 * The label is the clock times the data covers, not a duration. "5 min" says
 * how long the window is; it does not say *when* the readings were taken, and
 * a latency shown in milliseconds invites the reader to treat it as a number
 * with no time attached. So the label names the span, and the range's own
 * length stays in the card's subtitle where it belongs.
 *
 * The arrows are disabled at the ends rather than wrapping, so the control runs
 * out instead of silently jumping to the other end of the list.
 */
export default function RangeStepper({
  index,
  onChange,
  from,
  to,
}: {
  index: number;
  onChange: (next: number) => void;
  /** When the first reading in the range was taken, epoch milliseconds. */
  from: number | null;
  /** When the last reading in the range was taken, epoch milliseconds. */
  to: number | null;
}) {
  const range = HISTORY_RANGES[index] ?? HISTORY_RANGES[0];
  const atStart = index <= 0;
  const atEnd = index >= HISTORY_RANGES.length - 1;

  const label =
    from !== null && to !== null
      ? `${formatClockMinute(from)} to ${formatClockMinute(to)}`
      : range.label;

  return (
    <div className="flex items-center gap-1 rounded-pill border border-hairline bg-surface p-0.5">
      <button
        type="button"
        onClick={() => onChange(index - 1)}
        disabled={atStart}
        aria-label="Shorter range"
        title="Show a shorter range"
        className="grid size-6 place-items-center rounded-pill text-ink-muted transition-colors duration-[var(--motion)] hover:bg-surface-3 hover:text-ink disabled:pointer-events-none disabled:opacity-30"
      >
        <ChevronLeftIcon className="size-3.5" />
      </button>

      <span
        className="min-w-[7.5rem] text-center font-mono text-micro tracking-label text-ink-muted"
        // The visible text is the clock times. The name a screen reader gets is
        // the range's length, which is the part that says what the arrows do.
        aria-label={`Last ${range.label}`}
        aria-live="polite"
        title={`Last ${range.label}: ${label}`}
      >
        {label}
      </span>

      <button
        type="button"
        onClick={() => onChange(index + 1)}
        disabled={atEnd}
        aria-label="Longer range"
        title="Show a longer range"
        className="grid size-6 place-items-center rounded-pill text-ink-muted transition-colors duration-[var(--motion)] hover:bg-surface-3 hover:text-ink disabled:pointer-events-none disabled:opacity-30"
      >
        <ChevronRightIcon className="size-3.5" />
      </button>
    </div>
  );
}
