import { describe, expect, test } from "vitest";
import { actionForClick, actionForKey, actionForWheel } from "./keys";

const k = (key: string, shiftKey = false) => ({ key, shiftKey });

describe("actionForKey", () => {
  test("arrows follow direction", () => {
    expect(actionForKey(k("ArrowLeft"), "rtl")).toBe("next");
    expect(actionForKey(k("ArrowRight"), "rtl")).toBe("prev");
    expect(actionForKey(k("ArrowLeft"), "ltr")).toBe("prev");
    expect(actionForKey(k("ArrowRight"), "ltr")).toBe("next");
  });
  test("space is always next, shift+space prev", () => {
    expect(actionForKey(k(" "), "ltr")).toBe("next");
    expect(actionForKey(k(" ", true), "rtl")).toBe("prev");
  });
  test("other keys ignored", () => {
    expect(actionForKey(k("x"), "rtl")).toBeNull();
  });
});

describe("actionForClick / wheel", () => {
  test("left half click", () => {
    expect(actionForClick(0.2, "rtl")).toBe("next");
    expect(actionForClick(0.2, "ltr")).toBe("prev");
    expect(actionForClick(0.8, "rtl")).toBe("prev");
    expect(actionForClick(0.8, "ltr")).toBe("next");
  });
  test("wheel down is next", () => {
    expect(actionForWheel(40)).toBe("next");
    expect(actionForWheel(-40)).toBe("prev");
    expect(actionForWheel(0)).toBeNull();
  });
});
