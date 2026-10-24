import { invoke } from "@tauri-apps/api/core";

/**
 * The only place the window commands are called from, so the command names and
 * their contracts stay in one file.
 *
 * `toggleFullscreen` and `isFullscreen` both return the real state rather than
 * nothing, so the interface can label its button from what the window actually
 * is and never from an assumption about what the last click did.
 */

/**
 * How the window is filling the screen.
 *
 * `native` is the real macOS fullscreen, in its own space. `simple` is the
 * pre-Lion behaviour: the window covers the screen and the menu bar and the
 * Dock are hidden. A menu-bar app gets `simple`, because macOS does not give
 * an accessory app a fullscreen space.
 */
export type FullscreenMode = "windowed" | "native" | "simple";

/** What the window actually ended up doing. */
export interface FullscreenState {
  fullscreen: boolean;
  mode: FullscreenMode;
}

/**
 * Enters or leaves fullscreen.
 *
 * The backend tries the native path first and falls back to the simple one,
 * because the native path does not engage for a menu-bar app. Either way the
 * menu bar and the Dock are out of the way, which is the point of the control.
 */
export function toggleFullscreen(): Promise<FullscreenState> {
  return invoke<FullscreenState>("toggle_fullscreen");
}

/**
 * Leaves fullscreen without toggling.
 *
 * A separate call because Escape has to leave fullscreen even when the window
 * is already fullscreen, and a toggle would take it the other way.
 */
export function leaveFullscreen(): Promise<FullscreenState> {
  return invoke<FullscreenState>("leave_fullscreen_mode");
}
