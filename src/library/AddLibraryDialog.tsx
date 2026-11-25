import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { api } from "../api";

interface Props {
  onClose: () => void;
  onAdded: (id: number) => void;
}

export function AddLibraryDialog({ onClose, onAdded }: Props) {
  const [error, setError] = useState<string | null>(null);

  const pickLocal = async () => {
    const dir = await open({ directory: true, title: "만화 폴더 선택" });
    if (typeof dir !== "string") return;
    try {
      onAdded(await api.addLocalLibrary(dir));
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="modal-back" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <b>서재 추가</b>
          <button className="icon-btn" onClick={onClose} aria-label="닫기">✕</button>
        </div>
        <div className="modal-body">
          <p className="hint">ZIP 만화가 들어 있는 폴더를 고르세요. 하위 폴더까지 찾아서 작품별로 묶습니다.</p>
          <button className="btn primary" onClick={pickLocal}>폴더 선택…</button>
          {error && <p className="error">{error}</p>}
        </div>
      </div>
    </div>
  );
}
