import type { ReactNode } from "react";

/**
 * The one card treatment, and the bento grid it sits in.
 *
 * Depth is a hairline plus a one-step surface lift, never a shadow: only things
 * that actually float (the rail, popovers, the menu-bar panel) get a shadow, so
 * a shadow on a card would say "floating" about something that is not.
 */

/**
 * How many of the twelve columns a card claims at each breakpoint.
 *
 * Written out rather than interpolated because Tailwind only emits classes it
 * can see in the source, so a template string like `col-span-${span}` would
 * produce nothing at all.
 */
const SPAN_CLASS: Record<number, string> = {
  2: "col-span-2 sm:col-span-3 lg:col-span-2",
  3: "col-span-2 sm:col-span-3",
  4: "col-span-2 sm:col-span-6 lg:col-span-4",
  6: "col-span-2 sm:col-span-6",
  8: "col-span-2 sm:col-span-6 lg:col-span-8",
  12: "col-span-2 sm:col-span-6 lg:col-span-12",
};

export function Bento({ children }: { children: ReactNode }) {
  return (
    <div className="grid grid-cols-2 gap-stack sm:grid-cols-6 lg:grid-cols-12">
      {children}
    </div>
  );
}

export function Card({
  span = 4,
  rowSpan,
  className = "",
  children,
}: {
  span?: 2 | 3 | 4 | 6 | 8 | 12;
  /** Set for the focal card so a short card beside it cannot leave a gap. */
  tall?: boolean;
  rowSpan?: number;
  className?: string;
  children: ReactNode;
}) {
  const rowClass = rowSpan === 2 ? "lg:row-span-2" : "";
  return (
    <article
      className={`card flex min-w-0 flex-col p-card-pad ${SPAN_CLASS[span]} ${rowClass} ${className}`}
    >
      {children}
    </article>
  );
}

/** A card's top row: an icon tile, a title with an optional sub-line, and an end slot. */
export function CardHead({
  icon,
  title,
  sub,
  end,
}: {
  icon?: ReactNode;
  title: string;
  sub?: ReactNode;
  end?: ReactNode;
}) {
  return (
    <div className="mb-stack flex items-start gap-row">
      {icon !== undefined && (
        <div className="grid size-[38px] flex-none place-items-center rounded-inner bg-surface-3 text-ink-muted">
          {icon}
        </div>
      )}
      <div className="min-w-0">
        <h2 className="text-title font-medium tracking-title">{title}</h2>
        {sub !== undefined && (
          <p className="mt-0.5 text-small leading-[1.3] text-ink-faint">{sub}</p>
        )}
      </div>
      {end !== undefined && (
        <div className="ml-auto flex flex-none items-center gap-gap">{end}</div>
      )}
    </div>
  );
}

/** A card's body, pushed to the bottom so figures line up across a row. */
export function CardBody({ children }: { children: ReactNode }) {
  // `min-w-0` so a child with a large intrinsic width, such as a chart with a
  // view box, can shrink to the card instead of pushing the card's content
  // past its edge.
  return (
    <div className="flex min-w-0 flex-1 flex-col justify-end">{children}</div>
  );
}

/**
 * A measured figure with its unit, or an honest empty.
 *
 * `value` null is the only signal for "nothing measured", so a caller cannot
 * pass a stand-in zero through this by accident.
 */
export function Figure({
  value,
  unit,
  caption,
  size = "lg",
}: {
  value: string | null;
  unit?: string;
  caption?: ReactNode;
  size?: "sm" | "lg";
}) {
  const numberClass =
    size === "lg"
      ? "text-figure"
      : "text-[2rem] leading-none";

  return (
    <div>
      {value === null ? (
        <p className="font-display text-[1.375rem] leading-tight tracking-display text-ink-faint">
          No reading
        </p>
      ) : (
        <p className="flex items-baseline gap-row">
          <span
            className={`font-display font-light tracking-figure text-ink ${numberClass}`}
          >
            {value}
          </span>
          {unit !== undefined && (
            <span className="font-display text-body text-ink-faint">{unit}</span>
          )}
        </p>
      )}
      {caption !== undefined && (
        <p className="mt-gap text-small text-ink-faint">{caption}</p>
      )}
    </div>
  );
}

/**
 * The empty note: one quiet line where a list or a section would be.
 *
 * Used instead of an empty table or a zeroed card, because a row of zeros looks
 * like a measurement and this says plainly that there is nothing to show.
 */
export function EmptyNote({ children }: { children: ReactNode }) {
  return (
    <p className="hairline rounded-inner px-card-pad py-gutter text-center text-small text-ink-faint">
      {children}
    </p>
  );
}
