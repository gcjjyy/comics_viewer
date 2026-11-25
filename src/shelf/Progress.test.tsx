import { render } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { Progress } from "./Progress";

describe("Progress", () => {
  test("rtl fills from the right", () => {
    const { container } = render(<Progress value={0.5} direction="rtl" />);
    const fill = container.querySelector("[data-fill]") as HTMLElement;
    expect(fill.style.right).toBe("0px");
    expect(fill.style.left).toBe("");
    expect(fill.style.width).toBe("50%");
  });

  test("ltr fills from the left", () => {
    const { container } = render(<Progress value={0.25} direction="ltr" />);
    const fill = container.querySelector("[data-fill]") as HTMLElement;
    expect(fill.style.left).toBe("0px");
    expect(fill.style.width).toBe("25%");
  });

  test("clamps value", () => {
    const { container } = render(<Progress value={2} direction="ltr" />);
    expect((container.querySelector("[data-fill]") as HTMLElement).style.width).toBe("100%");
  });
});
