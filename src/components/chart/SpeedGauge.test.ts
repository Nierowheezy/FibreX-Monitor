import { describe, expect, it } from "vitest";

import { angleFor } from "./SpeedGauge";

describe("the speed gauge's scale", () => {
  it("puts zero at the start of the dial and the top speed at the end", () => {
    expect(angleFor(0)).toBe(-120);
    expect(angleFor(1000)).toBe(120);
  });

  it("never points outside the dial", () => {
    for (const value of [-5, 0, 0.4, 1, 7, 42, 250, 999, 1000, 50_000]) {
      expect(angleFor(value)).toBeGreaterThanOrEqual(-120);
      expect(angleFor(value)).toBeLessThanOrEqual(120);
    }
  });

  /**
   * A logarithmic scale is the point of the dial, so a fast line does not
   * press every ordinary speed into the first few degrees.
   */
  it("does not crush the low end into a corner", () => {
    const zero = angleFor(0);
    const one = angleFor(1);
    const ten = angleFor(10);

    // A megabit and ten megabits each get a real slice of the dial, rather
    // than every slow line pointing at the same spot.
    expect(one - zero).toBeGreaterThan(15);
    expect(ten - one).toBeGreaterThan(15);
  });

  it("puts a slower speed lower on the dial than a faster one", () => {
    expect(angleFor(10)).toBeLessThan(angleFor(100));
    expect(angleFor(100)).toBeLessThan(angleFor(500));
  });

  it("puts a value between two marks between their angles", () => {
    const one = angleFor(1);
    const ten = angleFor(10);
    const five = angleFor(5);

    expect(five).toBeGreaterThan(one);
    expect(five).toBeLessThan(ten);
  });

  it("treats a reading that is not a number as no reading", () => {
    // A reading that is not finite is not a reading, so the needle rests at
    // the start rather than pointing at a speed nothing measured.
    expect(angleFor(Number.NaN)).toBe(-120);
    expect(angleFor(Number.POSITIVE_INFINITY)).toBe(-120);
  });
});
