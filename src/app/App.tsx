import { useEffect, useState } from "react";

import { Wordmark } from "../components/ui/icons";
import IncidentsScreen from "../features/incidents/IncidentsScreen";
import OverviewScreen from "../features/overview/OverviewScreen";
import RouterScreen from "../features/router/RouterScreen";
import SettingsScreen from "../features/settings/SettingsScreen";
import SpeedTestScreen from "../features/speedtest/SpeedTestScreen";
import { leaveFullscreen, toggleFullscreen } from "../lib/tauri/window";
import AppearanceMenu from "./AppearanceMenu";
import Rail from "./Rail";
import type { ScreenId } from "./screens";

/**
 * What the topbar says under the title, per screen.
 *
 * A sub-line rather than a heading, because the card titles already name their
 * own sections and a second heading row would be one layer of chrome too many
 * for a utility window.
 */
const TAGLINE: Record<ScreenId, string> = {
  overview: "Live connection health",
  router: "The fibre line and the router on it",
  speedtest: "Measure the line to the internet",
  incidents: "Outages and their recovery times",
  settings: "Monitoring and appearance",
};

function Screen({ id }: { id: ScreenId }) {
  switch (id) {
    case "overview":
      return <OverviewScreen />;
    case "router":
      return <RouterScreen />;
    case "speedtest":
      return <SpeedTestScreen />;
    case "incidents":
      return <IncidentsScreen />;
    case "settings":
      return <SettingsScreen />;
  }
}

export default function App() {
  const [screen, setScreen] = useState<ScreenId>("overview");

  // The window title follows the screen, so the menu bar and Mission Control
  // name what is actually open rather than always saying "FibreX Monitor".
  useEffect(() => {
    document.title = `FibreX Monitor: ${TAGLINE[screen]}`;
  }, [screen]);

  // Escape leaves fullscreen, and the usual macOS shortcut enters it.
  //
  // Escape has to be handled here because the app's fullscreen is not macOS's
  // own, so macOS does not leave it. The shortcut uses the app's own path,
  // which falls back to the pre-Lion fullscreen if the native one does not
  // engage, so it works wherever the window does.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        void leaveFullscreen().catch(() => {
          // A window that cannot leave fullscreen is not worth a message here.
        });
        return;
      }

      if (event.ctrlKey && event.metaKey && event.key.toLowerCase() === "f") {
        event.preventDefault();
        void toggleFullscreen().catch(() => {
          // A window that cannot change its fullscreen state is not worth a
          // message here either.
        });
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="min-h-screen text-ink">
      <Rail current={screen} onSelect={setScreen} />

      {/*
        The rail is fixed and floats over the canvas, so the content reserves its
        width plus both gutters rather than sitting under it. Written as an
        arbitrary calc because the rail width and the spacing steps are tokens,
        not Tailwind scale values.
      */}
      <div className="mx-auto w-full max-w-[var(--shell-max)] px-gutter py-gutter pl-[calc(var(--s5)+var(--rail-w)+var(--s6))]">
        <header className="mb-stack flex items-center justify-between gap-stack">
          <div className="flex items-center gap-row">
            <Wordmark className="size-7 shrink-0" />
            <div className="flex flex-col">
              <h1 className="font-display text-title font-medium tracking-title">
                FibreX Monitor
              </h1>
              <p className="text-label text-ink-faint">{TAGLINE[screen]}</p>
            </div>
          </div>

          <div className="flex items-center gap-gap">
            <AppearanceMenu />
          </div>
        </header>

        <main key={screen} className="screen-in">
          <Screen id={screen} />
        </main>
      </div>
    </div>
  );
}
