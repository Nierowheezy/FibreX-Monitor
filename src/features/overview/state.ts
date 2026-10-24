import type { BadgeTone } from "../../components/status/types";
import type { ValueState } from "../../types/overview";

/**
 * How a value's provenance is named, in one place.
 *
 * The four states are the plan's data-integrity rule made visible: a number is
 * only ever shown as measured, calculated, stale or unavailable, so a reader can
 * tell a real reading from a held one from nothing at all.
 */
export const STATE_LABEL: Record<ValueState, string> = {
  measured: "Measured",
  calculated: "Calculated",
  stale: "Stale",
  unavailable: "Unavailable",
};

export const STATE_TONE: Record<ValueState, BadgeTone> = {
  measured: "ok",
  calculated: "ok",
  stale: "warn",
  unavailable: "neutral",
};

/**
 * The plain note beside the badge. `stale` is replaced at render with the real
 * age of the number being held, because a fixed phrase cannot know that age.
 */
export const STATE_NOTE: Record<ValueState, string> = {
  measured: "Measured now",
  calculated: "Calculated",
  stale: "Held reading",
  unavailable: "Nothing measured",
};
