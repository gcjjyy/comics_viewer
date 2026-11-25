import { useEffect, useState } from "react";
import { api, type RemoteDir } from "../api";

interface Props {
  host: string;
  onPick: (path: string) => void;
}

const parentOf = (path: string) => path.replace(/\/[^/]+\/?$/, "") || "/";
const join = (dir: string, name: string) => (dir.endsWith("/") ? dir + name : `${dir}/${name}`);

/** Folder picker on an SSH host. Starts at the login folder. */
export function RemoteBrowser({ host, onPick }: Props) {
  const [dir, setDir] = useState<RemoteDir | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const go = async (path: string | null) => {
    setLoading(true);
    setError(null);
    try {
      setDir(await api.sftpListDir(host, path));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    setDir(null);
    go(null);
  }, [host]);

  if (!dir) {
    return error ? <p className="error">{error}</p> : <p className="hint">{host}에 접속하는 중…</p>;
  }

  return (
    <div className="remote">
      <div className="remote-path">
        <button className="btn" disabled={dir.path === "/" || loading} onClick={() => go(parentOf(dir.path))}>
          상위 폴더
        </button>
        <code>{dir.path}</code>
      </div>
      <ul className="remote-list">
        {dir.folders.map((f) => (
          <li key={f}>
            <button className="folder" disabled={loading} onClick={() => go(join(dir.path, f))}>
              {f}
            </button>
          </li>
        ))}
        {dir.folders.length === 0 && <li className="hint">하위 폴더 없음</li>}
      </ul>
      {error && <p className="error">{error}</p>}
      <div className="modal-actions">
        <span className="hint">{dir.zipCount > 0 ? `이 폴더에 ZIP ${dir.zipCount}개` : "하위 폴더까지 찾아서 묶습니다"}</span>
        <button className="btn primary" disabled={loading} onClick={() => onPick(dir.path)}>
          이 폴더를 서재로 추가
        </button>
      </div>
    </div>
  );
}
