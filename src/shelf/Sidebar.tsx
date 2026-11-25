import type { Library } from "../api";

export type Selection = { kind: "library"; id: number } | { kind: "recent" };

interface Props {
  libraries: Library[];
  selection: Selection | null;
  onSelect: (s: Selection) => void;
  onAdd: () => void;
  onLibraryMenu: (lib: Library, e: React.MouseEvent) => void;
}

export function Sidebar({ libraries, selection, onSelect, onAdd, onLibraryMenu }: Props) {
  const isLib = (id: number) => selection?.kind === "library" && selection.id === id;
  return (
    <nav className="sidebar">
      <div className="side-head">서재</div>
      {libraries.map((l) => (
        <button
          key={l.id}
          className={`side-item ${isLib(l.id) ? "on" : ""}`}
          onClick={() => onSelect({ kind: "library", id: l.id })}
          onContextMenu={(e) => {
            e.preventDefault();
            onLibraryMenu(l, e);
          }}
          title={l.kind === "sftp" ? `${l.host}:${l.root}` : l.root}
        >
          <span className="side-icon">{l.kind === "sftp" ? "🖧" : "📁"}</span>
          <span className="side-label">{l.name}</span>
        </button>
      ))}
      <button className="side-item dim" onClick={onAdd}>
        <span className="side-icon">＋</span>서재 추가
      </button>
      <div className="side-head">보기</div>
      <button className={`side-item ${selection?.kind === "recent" ? "on" : ""}`} onClick={() => onSelect({ kind: "recent" })}>
        <span className="side-icon">🕘</span>최근 읽은 책
      </button>
    </nav>
  );
}
