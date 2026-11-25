import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, type SshPrompt } from "../api";

/** Answers questions the SSH connection asks: unknown host key, key passphrase, password. */
export function SshPromptDialog() {
  const [queue, setQueue] = useState<SshPrompt[]>([]);
  const [secret, setSecret] = useState("");
  const [remember, setRemember] = useState(false);

  useEffect(() => {
    const un = listen<SshPrompt>("ssh-prompt", (e) => setQueue((q) => [...q, e.payload]));
    return () => {
      un.then((f) => f());
    };
  }, []);

  const p = queue[0];
  if (!p) return null;

  const answer = (accept: boolean) => {
    const needsSecret = p.kind !== "hostKey";
    api.sshPromptReply(p.id, accept, accept && needsSecret ? secret : null, accept && needsSecret && remember);
    setQueue((q) => q.slice(1));
    setSecret("");
    setRemember(false);
  };

  return (
    <div className="modal-back">
      <form
        className="modal"
        onSubmit={(e) => {
          e.preventDefault();
          answer(true);
        }}
      >
        <div className="modal-head">
          <b>{p.kind === "hostKey" ? `${p.host} 첫 접속` : `${p.host} 로그인`}</b>
        </div>
        <div className="modal-body">
          {p.kind === "hostKey" ? (
            <>
              <p className="hint">
                처음 접속하는 서버입니다. 아래 지문이 서버의 실제 호스트 키와 같은지 확인한 뒤 접속하세요. 접속하면
                ~/.ssh/known_hosts에 저장되어 다음부터는 묻지 않습니다.
              </p>
              <code className="fingerprint">{p.fingerprint}</code>
            </>
          ) : (
            <>
              <label className="field">
                <span>{p.kind === "password" ? "비밀번호" : `키 파일 암호 (${p.keyPath})`}</span>
                <input
                  type="password"
                  autoFocus
                  aria-label={p.kind === "password" ? "비밀번호" : "키 파일 암호"}
                  value={secret}
                  onChange={(e) => setSecret(e.target.value)}
                />
              </label>
              <label className="check">
                <input type="checkbox" checked={remember} onChange={(e) => setRemember(e.target.checked)} />
                키체인에 저장
              </label>
            </>
          )}
          <div className="modal-actions">
            <button type="button" className="btn" onClick={() => answer(false)}>
              취소
            </button>
            <button type="submit" className="btn primary">
              {p.kind === "hostKey" ? "접속" : "확인"}
            </button>
          </div>
        </div>
      </form>
    </div>
  );
}
