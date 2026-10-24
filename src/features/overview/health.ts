import type { BadgeTone } from "../../components/status/types";
import type { HealthState } from "../../types/overview";

/**
 * The health vocabulary, in one place.
 *
 * Three surfaces name the same five states, so the words and their tones live
 * here rather than being restated per component. The plain-language sentences
 * describe without blaming anyone: an offline connection is described, never
 * attributed to the provider.
 */
export const HEALTH_LABEL: Record<HealthState, string> = {
  connected: "Connected",
  degraded: "Degraded",
  offline: "Offline",
  unknown: "Unknown",
  paused: "Paused",
};

export const HEALTH_TONE: Record<HealthState, BadgeTone> = {
  connected: "ok",
  degraded: "warn",
  offline: "error",
  unknown: "neutral",
  paused: "neutral",
};

export const HEALTH_SENTENCE: Record<HealthState, string> = {
  connected: "Everything is fine",
  degraded: "The connection is struggling",
  offline: "No internet connection",
  unknown: "Nothing measured yet",
  paused: "Monitoring is paused",
};