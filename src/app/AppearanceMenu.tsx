import { useCallback, useEffect, useRef, useState } from "react";

import { MoonIcon, SunIcon, SystemIcon } from "../components/ui/icons";
import { useTheme } from "../hooks/useTheme";
import type { Appearance } from "./theme";

const APPEARANCE_LABEL: Record<Appearance, string> = {
  system: "System",
  light: "Light",
  dark: "Dark",
};

const APPEARANCE_ICON: Record<Appearance, (props: { className?: string }) => React.ReactNode> =
  {
    system: SystemIcon,
    light: SunIcon,
    dark: MoonIcon,
  };

/**
 * One icon that opens the three appearance modes, rather than a row of segments
 * spending permanent header width on a control most people set once.
 *
 * The trigger shows the mode that was chosen, not the mode currently showing, so
 * System does not silently read as Dark on a Mac in dark mode.
 */
export default function AppearanceMenu() {
  const [settings, update] = useTheme();
  const [open, setOpen] = useState(false);
  const container = useRef<HTMLDivElement>(null);

  const close = useCallback(() => setOpen(false), []);

  useEffect(() => {
    if (!open) {
      return;
    }

    // pointerdown rather than click, so the menu closes before a click lands on
    // whatever is underneath it.
    const onPointerDown = (event: PointerEvent) => {
      if (container.current && !container.current.contains(event.target as Node)) {
        close();
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        close();
      }
    };

    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);

    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open, close]);

  const TriggerIcon = APPEARANCE_ICON[settings.appearance];

  return (
    <div className="relative" ref={container}>
      <button
        type="button"
        className="flex size-[var(--control-size)] cursor-pointer items-center justify-center rounded-pill border border-hairline bg-surface text-ink-muted transition-colors duration-[var(--motion)] hover:text-ink"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`Appearance: ${APPEARANCE_LABEL[settings.appearance]}`}
        title="Appearance"
        onClick={() => setOpen((value) => !value)}
      >
        <TriggerIcon className="size-5" />
      </button>

      {open && (
        <div
          role="menu"
          aria-label="Appearance"
          className="absolute top-[calc(100%+var(--s2))] right-0 z-10 min-w-[148px] rounded-inner border border-hairline bg-surface-2 p-1 shadow-pop"
        >
          {(Object.keys(APPEARANCE_LABEL) as Appearance[]).map((appearance) => {
            const Icon = APPEARANCE_ICON[appearance];
            const selected = settings.appearance === appearance;

            return (
              <button
                key={appearance}
                type="button"
                role="menuitemradio"
                aria-checked={selected}
                className={`flex w-full cursor-pointer items-center gap-row rounded-chip px-3 py-2 text-left text-label font-medium ${
                  selected
                    ? "bg-surface-3 font-semibold text-ink"
                    : "text-ink-muted hover:bg-surface-3 hover:text-ink"
                }`}
                onClick={() => {
                  update({ ...settings, appearance });
                  close();
                }}
              >
                <Icon className="size-4 shrink-0" />
                {APPEARANCE_LABEL[appearance]}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
