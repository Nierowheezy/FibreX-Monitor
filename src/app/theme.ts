/**
 * Appearance settings: system, light, or dark.
 *
 * One choice, not two. An earlier version also carried a "palette" concept that
 * the CSS never implemented, so every screen rendered the same tokens while the
 * stored preference implied otherwise. That is gone: appearance is the only
 * setting here, and it maps straight onto the `data-theme` attribute the token
 * layer is scoped by.
 *
 * The value is stored in local storage. Nothing leaves the machine.
 */

export const APPEARANCES = ["system", "light", "dark"] as const;
export type Appearance = (typeof APPEARANCES)[number];

export type ThemeSettings = {
  appearance: Appearance;
};

/**
 * System is the default. The dark bento tokens are the base layer, so a Mac in
 * dark mode gets the reference design with no stored preference at all, and a
 * Mac in light mode gets the light inversion.
 */
export const DEFAULT_THEME: ThemeSettings = {
  appearance: "system",
};

const APPEARANCE_KEY = "fibrex.theme.appearance";

/**
 * A stored value from the old two-field shape is still readable, because the
 * appearance key never changed. The dead palette key is left where it is rather
 * than migrated; unknown keys are ignored on read.
 */
const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

export function resolveAppearance(appearance: Appearance): "light" | "dark" {
  if (appearance !== "system") {
    return appearance;
  }
  return darkQuery.matches ? "dark" : "light";
}

/**
 * Applies the theme by setting the attribute the token layer is scoped by.
 * Called once before the first render so the screen never flashes the wrong
 * theme, and again on every change.
 */
export function applyTheme(settings: ThemeSettings): void {
  document.documentElement.setAttribute(
    "data-theme",
    resolveAppearance(settings.appearance),
  );
}

function readStored<T extends string>(
  key: string,
  allowed: readonly T[],
  fallback: T,
): T {
  try {
    const stored = window.localStorage.getItem(key);
    const isAllowed =
      stored !== null && (allowed as readonly string[]).includes(stored);
    return isAllowed ? (stored as T) : fallback;
  } catch {
    // Storage can be unavailable. A missing preference must never stop startup.
    return fallback;
  }
}

function writeStored(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // Same reasoning as above: the choice still applies for this session.
  }
}

export function loadTheme(): ThemeSettings {
  return {
    appearance: readStored(APPEARANCE_KEY, APPEARANCES, DEFAULT_THEME.appearance),
  };
}

export function saveTheme(settings: ThemeSettings): void {
  writeStored(APPEARANCE_KEY, settings.appearance);
}

/**
 * Subscribes to OS appearance changes so "system" follows it live, and returns
 * the unsubscribe function for the effect cleanup.
 */
export function onSystemAppearanceChange(handler: () => void): () => void {
  darkQuery.addEventListener("change", handler);
  return () => darkQuery.removeEventListener("change", handler);
}
