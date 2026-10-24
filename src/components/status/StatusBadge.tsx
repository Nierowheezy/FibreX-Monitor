import type { BadgeTone } from "./types";

/*
 * A tinted pill with no border and no dot. The label is always rendered as text
 * beside the colour, so meaning never depends on colour alone.
 *
 * The washes are separate tokens from the status colours rather than the colour
 * at low alpha, because the light theme needs a different relationship between
 * the two to stay legible.
 */
const TONE_CLASS: Record<BadgeTone, string> = {
  ok: "bg-good-wash text-good",
  warn: "bg-warn-wash text-warn",
  error: "bg-bad-wash text-bad",
  neutral: "bg-idle-wash text-idle",
};

export default function StatusBadge({
  tone,
  children,
}: {
  tone: BadgeTone;
  children: React.ReactNode;
}) {
  return (
    <span
      className={`inline-flex w-fit items-center rounded-pill px-3 py-0.5 text-micro font-semibold tracking-label ${TONE_CLASS[tone]}`}
    >
      {children}
    </span>
  );
}
