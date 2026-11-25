import type { Direction } from "../api";

export type Dim = { w: number; h: number } | undefined;

const isWide = (d: Dim) => d !== undefined && d.w > d.h;

/**
 * Groups pages into what is shown at once, in reading order.
 * Cover alone; `shift` also leaves page 1 alone; wide pages stand alone; unknown sizes count as portrait.
 */
export function buildSpreads(count: number, dims: Dim[], shift: boolean, mode: "spread" | "single"): number[][] {
  const out: number[][] = [];
  let i = 0;
  while (i < count) {
    const solo = mode === "single" || i === 0 || (shift && i === 1) || isWide(dims[i]) || i + 1 >= count || isWide(dims[i + 1]);
    if (solo) {
      out.push([i]);
      i += 1;
    } else {
      out.push([i, i + 1]);
      i += 2;
    }
  }
  return out;
}

/** Index of the spread holding `page`; pages past the end map to the last spread. */
export function spreadIndexOf(spreads: number[][], page: number): number {
  const i = spreads.findIndex((s) => s.includes(page));
  return i >= 0 ? i : Math.max(0, spreads.length - 1);
}

/** Left-to-right screen order. Japanese books put the first-read page on the right. */
export function screenOrder(spread: number[], dir: Direction): number[] {
  return dir === "rtl" ? [...spread].reverse() : spread;
}
