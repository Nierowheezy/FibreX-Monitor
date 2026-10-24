import { loadClockFormat } from "./clock";
import { applyTheme, loadTheme } from "./theme";

/**
 * Applies the stored preferences before the first render.
 *
 * Both windows call this. If the panel skipped it, the same stored preference
 * could produce two windows in different themes, or in different clock formats,
 * which is exactly the kind of bug nobody notices until it is showing on their
 * screen.
 */
export function bootstrapTheme(): void {
  applyTheme(loadTheme());
  loadClockFormat();
}
