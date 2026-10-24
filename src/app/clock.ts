/**
 * The clock format setting.
 *
 * Kept out of the theme module because it is a different thing: the theme is
 * about how the app looks and follows the system, while the clock format is
 * about how a number is written and is the reader's choice.
 *
 * The format lives in a module-level value rather than in React state, because
 * the formatters that use it are plain functions called from all over the app,
 * including from the menu-bar panel, which is a separate window with its own
 * React root. A value a formatter can read is a value that cannot be forgotten
 * by a component that forgot to pass it down.
 */

export type ClockFormat = "system" | "24h" | "12h";

const STORAGE_KEY = "fibrex.clockFormat";

/**
 * The format in force.
 *
 * Starts as the system format, so a first run is right without anything being
 * stored, and is replaced by the stored value on load.
 */
let current: ClockFormat = "system";

/** The system clock format, which is the user's macOS setting. */
function systemUsesTwelveHours(): boolean {
  return new Intl.DateTimeFormat().resolvedOptions().hour12 === true;
}

/**
 * Whether a time is written on a twelve-hour clock.
 *
 * The system answer is read at the moment the time is written rather than
 * cached, so a user who changes the setting in macOS while the app is running
 * sees the change without restarting it.
 */
export function usesTwelveHours(format: ClockFormat = current): boolean {
  if (format === "12h") return true;
  if (format === "24h") return false;
  return systemUsesTwelveHours();
}

/** Reads the stored format, and applies it. */
export function loadClockFormat(): ClockFormat {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === "system" || stored === "12h" || stored === "24h") {
      current = stored;
    }
  } catch {
    // A browser with storage disabled is not a failure worth reporting. The
    // setting simply does not persist, and the app still works.
  }
  return current;
}

/** Stores the format and applies it. */
export function saveClockFormat(format: ClockFormat): void {
  current = format;
  try {
    window.localStorage.setItem(STORAGE_KEY, format);
  } catch {
    // As above: the setting does not persist, and nothing else is affected.
  }
}

/** The label a control shows for a format. */
export function clockFormatLabel(format: ClockFormat): string {
  switch (format) {
    case "12h":
      return "12-hour";
    case "24h":
      return "24-hour";
    case "system":
      return "System";
  }
}
