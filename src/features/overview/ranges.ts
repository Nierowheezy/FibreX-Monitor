/**
 * The ranges the latency history can be shown over.
 *
 * The windows are the ones a person actually asks about: is it bad right now,
 * is it bad this afternoon, is it bad today. Each is a real span of stored
 * history, and the control that picks one is a stepper rather than a free
 * slider, because a slider that can land on a range nobody can read is a
 * control that produces an empty chart.
 */

export interface HistoryRange {
  /** The label the control shows. */
  label: string;
  /** How far back the range reaches, in milliseconds. */
  windowMs: number;
}

export const HISTORY_RANGES: readonly HistoryRange[] = [
  { label: "5 min", windowMs: 5 * 60_000 },
  { label: "15 min", windowMs: 15 * 60_000 },
  { label: "1 hour", windowMs: 60 * 60_000 },
  { label: "6 hours", windowMs: 6 * 60 * 60_000 },
  { label: "24 hours", windowMs: 24 * 60 * 60_000 },
] as const;

/** The range the app opens on. The shortest one, which is the live picture. */
export const DEFAULT_RANGE_INDEX = 0;

/**
 * The range at an index, clamped to the list.
 *
 * Clamped rather than wrapped: stepping past the end of a list of ranges
 * should stop at the end, not silently jump to the other end, because the
 * control is a pair of arrows and the reader expects them to run out.
 */
export function rangeAt(index: number): HistoryRange {
  const clamped = Math.min(Math.max(index, 0), HISTORY_RANGES.length - 1);
  return HISTORY_RANGES[clamped];
}
