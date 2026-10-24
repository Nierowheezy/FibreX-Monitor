import { useCallback, useEffect, useState } from "react";

import {
  applyTheme,
  loadTheme,
  onSystemAppearanceChange,
  saveTheme,
  type ThemeSettings,
} from "../app/theme";

/**
 * React binding for the theme settings. The settings themselves live in
 * `src/app/theme.ts` so that module stays free of React and the picker can move
 * to Preferences in feature 18 without touching this logic.
 *
 * The first paint is handled in `src/main.tsx`, which applies the stored theme
 * before React mounts. That is deliberate: doing it in an effect would show the
 * default theme for a frame and then repaint.
 */
export function useTheme(): [
  ThemeSettings,
  (next: ThemeSettings) => void,
] {
  const [settings, setSettings] = useState<ThemeSettings>(loadTheme);

  useEffect(() => {
    applyTheme(settings);
    saveTheme(settings);
  }, [settings]);

  useEffect(() => {
    if (settings.appearance !== "system") {
      return;
    }
    return onSystemAppearanceChange(() => applyTheme(settings));
  }, [settings]);

  const update = useCallback((next: ThemeSettings) => {
    setSettings(next);
  }, []);

  return [settings, update];
}