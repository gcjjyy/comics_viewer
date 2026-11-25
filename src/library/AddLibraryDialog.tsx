import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { api } from "../api";
import { RemoteBrowser } from "./RemoteBrowser";

interface Props {
  onClose: () => void;
  onAdded: (id: number) => void;
}

type Tab = "local" | "ssh";

export function AddLibraryDialog({ onClose, onAdded }: Props) {
  const [tab, setTab] = useState<Tab>("local");
  const [hosts, setHosts] = useState<string[] | null>(null);
  const [host, setHost] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (tab === "ssh" && hosts === null) api.listSshHosts().then(setHosts, (e) => setError(String(e)));
  }, [tab, hosts]);

  const add = async (f: () => Promise<number>) => {
    setError(null);
    try {
      onAdded(await f());
    } catch (e) {
      setError(String(e));
    }
  };

  const pickLocal = async () => {
    const dir = await open({ directory: true, title: "만화 폴더 선택" });
    if (typeof dir === "string") add(() => api.addLocalLibrary(dir));
  };

  return (
    <div className="modal-back" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <b>서재 추가</b>
          <button className="icon-btn" onClick={onClose} aria-label="닫기">✕</button>
        </div>
        <div className="tabs" role="tablist">
          {(["local", "ssh"] as const).map((t) => (
            <button key={t} role="tab" aria-selected={tab === t} className={tab === t ? "on" : ""} onClick={() => setTab(t)}>
              {t === "local" ? "내 컴퓨터" : "SSH 서버"}
            </button>
          ))}
        </div>
        <div className="modal-body">
          {tab === "local" ? (
            <>
              <p className="hint">ZIP 만화가 들어 있는 폴더를 고르세요. 하위 폴더까지 찾아서 작품별로 묶습니다.</p>
              <button className="btn primary" onClick={pickLocal}>폴더 선택…</button>
            </>
          ) : hosts === null ? null : hosts.length === 0 ? (
            <p className="hint">
              ~/.ssh/config에 등록된 호스트가 없습니다. Host 항목(HostName, User 등)을 추가한 뒤 이 창을 다시 여세요.
            </p>
          ) : (
            <>
              <div className="host-row">
                {hosts.map((h) => (
                  <button key={h} className={`chip-btn ${host === h ? "on" : ""}`} onClick={() => setHost(h)}>
                    {h}
                  </button>
                ))}
              </div>
              {host ? (
                <RemoteBrowser key={host} host={host} onPick={(path) => add(() => api.addSftpLibrary(host, path))} />
              ) : (
                <p className="hint">~/.ssh/config의 호스트 중 하나를 고르세요.</p>
              )}
            </>
          )}
          {error && <p className="error">{error}</p>}
        </div>
      </div>
    </div>
  );
}
