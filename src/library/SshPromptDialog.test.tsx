import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SshPrompt } from "../api";

let handler: ((e: { payload: SshPrompt }) => void) | null = null;
vi.mock("@tauri-apps/api/event", () => ({
  listen: (_: string, h: (e: { payload: SshPrompt }) => void) => {
    handler = h;
    return Promise.resolve(() => {});
  },
}));
const reply = vi.fn().mockResolvedValue(undefined);
vi.mock("../api", () => ({ api: { sshPromptReply: (...a: unknown[]) => reply(...a) } }));

import { SshPromptDialog } from "./SshPromptDialog";

const send = async (p: Partial<SshPrompt>) => {
  await act(async () => handler!({ payload: { id: 1, kind: "password", host: "nas", fingerprint: null, keyPath: null, ...p } }));
};

describe("SshPromptDialog", () => {
  beforeEach(() => reply.mockClear());

  it("shows the host key fingerprint and accepts", async () => {
    render(<SshPromptDialog />);
    await send({ id: 7, kind: "hostKey", fingerprint: "SHA256:abc" });
    expect(screen.getByText("SHA256:abc")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "접속" }));
    expect(reply).toHaveBeenCalledWith(7, true, null, false);
    expect(screen.queryByText("SHA256:abc")).toBeNull();
  });

  it("sends the password with remember", async () => {
    render(<SshPromptDialog />);
    await send({ id: 3, kind: "password" });
    fireEvent.change(screen.getByLabelText("비밀번호"), { target: { value: "pw" } });
    fireEvent.click(screen.getByLabelText("키체인에 저장"));
    fireEvent.click(screen.getByRole("button", { name: "확인" }));
    expect(reply).toHaveBeenCalledWith(3, true, "pw", true);
  });

  it("names the key file when asking a passphrase, and cancels", async () => {
    render(<SshPromptDialog />);
    await send({ id: 4, kind: "passphrase", keyPath: "/h/.ssh/id_ed25519" });
    expect(screen.getByText(/id_ed25519/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "취소" }));
    expect(reply).toHaveBeenCalledWith(4, false, null, false);
  });
});
