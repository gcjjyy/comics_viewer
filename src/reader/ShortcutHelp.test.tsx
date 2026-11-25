import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ShortcutHelp } from "./ShortcutHelp";

const rowOf = (key: string) => screen.getByText(key).closest("tr")!.textContent;

describe("ShortcutHelp", () => {
  it("maps arrows for Japanese (rtl) books", () => {
    render(<ShortcutHelp direction="rtl" onClose={() => {}} />);
    expect(rowOf("←")).toContain("다음 쪽");
    expect(rowOf("→")).toContain("이전 쪽");
  });

  it("maps arrows for Korean (ltr) books", () => {
    render(<ShortcutHelp direction="ltr" onClose={() => {}} />);
    expect(rowOf("→")).toContain("다음 쪽");
    expect(rowOf("←")).toContain("이전 쪽");
  });

  it("closes from the button", () => {
    const onClose = vi.fn();
    render(<ShortcutHelp direction="rtl" onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "닫기" }));
    expect(onClose).toHaveBeenCalled();
  });
});
