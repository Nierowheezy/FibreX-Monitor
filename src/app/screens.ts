/**
 * The screens, in rail order.
 *
 * One list, so the rail and any keyboard navigation read the same order and a
 * screen cannot exist in one place and not another. Settings sits apart from the
 * monitoring screens, which is why the rail draws a separator above it.
 *
 * There is no Devices screen. The router's device list is behind an endpoint
 * that answers `"NONE"` on this firmware, because the router builds the table
 * lazily from an ARP scan and it does not exist until that scan has run. A
 * screen that cannot be filled is a screen that would sit there saying so
 * forever, so it was taken out rather than left as a permanent apology.
 */
export type ScreenId =
  | "overview"
  | "router"
  | "speedtest"
  | "incidents"
  | "settings";

export const MONITORING_SCREENS: ScreenId[] = [
  "overview",
  "router",
  "speedtest",
  "incidents",
];
