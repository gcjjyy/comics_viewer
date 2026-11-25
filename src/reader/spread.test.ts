import { describe, expect, test } from "vitest";
import { buildSpreads, screenOrder, spreadIndexOf } from "./spread";

const portrait = { w: 2, h: 3 };
const wide = { w: 6, h: 4 };

describe("buildSpreads", () => {
  test("cover alone, then pairs, last single alone", () => {
    expect(buildSpreads(6, [], false, "spread")).toEqual([[0], [1, 2], [3, 4], [5]]);
  });
  test("shift makes page 1 alone too", () => {
    expect(buildSpreads(6, [], true, "spread")).toEqual([[0], [1], [2, 3], [4, 5]]);
  });
  test("wide page alone, pairing restarts after it", () => {
    const d = Array(7).fill(portrait);
    d[3] = wide;
    expect(buildSpreads(7, d, false, "spread")).toEqual([[0], [1, 2], [3], [4, 5], [6]]);
  });
  test("page before a wide page stays alone", () => {
    const e = Array(5).fill(portrait);
    e[2] = wide;
    expect(buildSpreads(5, e, false, "spread")).toEqual([[0], [1], [2], [3, 4]]);
  });
  test("single mode", () => {
    expect(buildSpreads(3, [], false, "single")).toEqual([[0], [1], [2]]);
  });
  test("empty and one page", () => {
    expect(buildSpreads(0, [], false, "spread")).toEqual([]);
    expect(buildSpreads(1, [], false, "spread")).toEqual([[0]]);
  });
});

describe("spreadIndexOf / screenOrder", () => {
  test("finds spread containing page", () => {
    expect(spreadIndexOf([[0], [1, 2], [3]], 2)).toBe(1);
    expect(spreadIndexOf([[0], [1, 2], [3]], 9)).toBe(2);
  });
  test("rtl places first-read page on the right", () => {
    expect(screenOrder([1, 2], "rtl")).toEqual([2, 1]);
    expect(screenOrder([1, 2], "ltr")).toEqual([1, 2]);
  });
});
