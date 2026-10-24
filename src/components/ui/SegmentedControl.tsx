import type { ReactNode } from "react";

/**
 * A row of mutually exclusive choices.
 *
 * The design's segmented control, and the right control for a small set of
 * options that the user picks one of: every option is visible at once, so
 * there is no menu to open and nothing to remember.
 *
 * Only one is selected, and the selection is carried by `aria-pressed` as well
 * as by the colour, so it does not depend on being able to tell the colours
 * apart.
 */
export default function SegmentedControl<T extends string>({
  label,
  options,
  value,
  onChange,
}: {
  /** Names the group, for a screen reader. */
  label: string;
  options: Array<{ value: T; label: ReactNode }>;
  value: T;
  onChange: (next: T) => void;
}) {
  return (
    <div
      role="group"
      aria-label={label}
      className="inline-flex items-center gap-0.5 rounded-control border border-hairline bg-surface-2 p-0.5"
    >
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            onClick={() => onChange(option.value)}
            aria-pressed={selected}
            className={`rounded-[9px] px-3 py-1.5 text-label font-medium transition-colors duration-[var(--motion)] ${
              selected
                ? "bg-surface-4 text-ink shadow-none"
                : "text-ink-muted hover:text-ink"
            }`}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
