import { useCallback, useEffect, useState } from "react";

import {
  loadClockFormat,
  saveClockFormat,
  type ClockFormat,
} from "../app/clock";

/**
 * React binding for the clock format.
 *
 * The format itself lives in `src/app/clock.ts` so the formatters can read it
 * without a component in the way, and this hook is the one place the choice is
 * made and stored.
 */
export function useClockFormat(): [
  ClockFormat,
  (next: ClockFormat) => void,
] {
  const [format, setFormat] = useState<ClockFormat>(loadClockFormat);

  useEffect(() => {
    saveClockFormat(format);
  }, [format]);

  const update = useCallback((next: ClockFormat) => {
    setFormat(next);
  }, []);

  return [format, update];
}
