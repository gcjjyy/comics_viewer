import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const dirs: Record<string, { path: string; folders: string[]; zipCount: number }> = {
  home: { path: "/home/me", folders: ["comics", "docs"], zipCount: 0 },
  "/home/me/comics": { path: "/home/me/comics", folders: ["슬램덩크"], zipCount: 2 },
  "/home": { path: "/home", folders: ["me"], zipCount: 0 },
  "/home/me": { path: "/home/me", folders: ["comics", "docs"], zipCount: 0 },
};
const list = vi.fn((_h: string, p: string | null) => Promise.resolve(dirs[p ?? "home"]));
vi.mock("../api", () => ({ api: { sftpListDir: (h: string, p: string | null) => list(h, p) } }));

import { RemoteBrowser } from "./RemoteBrowser";

describe("RemoteBrowser", () => {
  it("starts at the login folder, enters folders, goes up, and picks", async () => {
    const onPick = vi.fn();
    await act(async () => render(<RemoteBrowser host="nas" onPick={onPick} />));
    expect((screen.getByLabelText("경로") as HTMLInputElement).value).toBe("/home/me");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "comics" })));
    expect(list).toHaveBeenLastCalledWith("nas", "/home/me/comics");
    expect(screen.getByText(/ZIP 2개/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "이 폴더를 서재로 추가" }));
    expect(onPick).toHaveBeenCalledWith("/home/me/comics");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "상위 폴더" })));
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "상위 폴더" })));
    expect(list).toHaveBeenLastCalledWith("nas", "/home");
  });

  it("goes to a typed path on Enter", async () => {
    await act(async () => render(<RemoteBrowser host="nas" onPick={() => {}} />));
    const input = screen.getByLabelText("경로");
    fireEvent.change(input, { target: { value: "/home/me/comics" } });
    await act(async () => fireEvent.keyDown(input, { key: "Enter" }));
    expect(list).toHaveBeenLastCalledWith("nas", "/home/me/comics");
    expect((screen.getByLabelText("경로") as HTMLInputElement).value).toBe("/home/me/comics");
  });

  it("shows the connection error", async () => {
    list.mockRejectedValueOnce("'nas' 서버에 접속하지 못했습니다: timeout");
    await act(async () => render(<RemoteBrowser host="nas" onPick={() => {}} />));
    expect(screen.getByText(/접속하지 못했습니다/)).toBeTruthy();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "다시 접속" })));
    expect((screen.getByLabelText("경로") as HTMLInputElement).value).toBe("/home/me");
  });
});
