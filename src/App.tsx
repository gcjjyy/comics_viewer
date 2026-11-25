import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type Direction, type Library, type LibraryUpdated, type RecentView, type VolumeView, type WorkView } from "./api";
import { AddLibraryDialog } from "./library/AddLibraryDialog";
import { Reader } from "./reader/Reader";
import { BookCard } from "./shelf/BookCard";
import { Sidebar, type Selection } from "./shelf/Sidebar";
import { VolumeGrid } from "./shelf/VolumeGrid";
import { WorkGrid } from "./shelf/WorkGrid";
import { ContextMenu, type MenuState } from "./ui/ContextMenu";

const norm = (s: string) => s.toLowerCase().replace(/\s+/g, "");
const dirLabel: Record<Direction, string> = { rtl: "일본 만화 (오른쪽→왼쪽)", ltr: "한국 만화 (왼쪽→오른쪽)" };

type ReaderTarget = { libraryId: number; key: string };

export default function App() {
  const [libraries, setLibraries] = useState<Library[] | null>(null);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [works, setWorks] = useState<WorkView[] | null>(null);
  const [work, setWork] = useState<WorkView | null>(null);
  const [volumes, setVolumes] = useState<VolumeView[] | null>(null);
  const [recent, setRecent] = useState<RecentView[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [adding, setAdding] = useState(false);
  const [reader, setReader] = useState<ReaderTarget | null>(null);

  const libraryId = selection?.kind === "library" ? selection.id : null;

  const loadLibraries = useCallback(async () => {
    const libs = await api.listLibraries();
    setLibraries(libs);
    return libs;
  }, []);

  useEffect(() => {
    loadLibraries().then((libs) => {
      if (libs.length) setSelection({ kind: "library", id: libs[0].id });
    });
  }, [loadLibraries]);

  const loadWorks = useCallback(async (id: number) => {
    try {
      setWorks(await api.listWorks(id));
      setError(null);
    } catch (e) {
      setWorks([]);
      setError(String(e));
    }
  }, []);

  const loadVolumes = useCallback(async (id: number, w: WorkView) => {
    setVolumes(await api.listVolumes(id, w.id));
  }, []);

  // Library opened: show cached list immediately, then rescan in the background.
  useEffect(() => {
    if (libraryId === null) return;
    setWorks(null);
    setWork(null);
    setVolumes(null);
    setQuery("");
    loadWorks(libraryId).then(() => api.refreshLibrary(libraryId));
  }, [libraryId, loadWorks]);

  useEffect(() => {
    if (selection?.kind === "recent") api.listRecent().then(setRecent);
  }, [selection]);

  useEffect(() => {
    const un = listen<LibraryUpdated>("library-updated", (e) => {
      if (e.payload.libraryId !== libraryId) return;
      if (e.payload.error) {
        setError(e.payload.error);
        return;
      }
      loadWorks(libraryId);
      if (work) loadVolumes(libraryId, work);
    });
    return () => {
      un.then((f) => f());
    };
  }, [libraryId, work, loadWorks, loadVolumes]);

  const openWork = (w: WorkView) => {
    if (libraryId === null) return;
    setWork(w);
    setVolumes(null);
    loadVolumes(libraryId, w);
  };

  const closeReader = () => {
    setReader(null);
    if (libraryId !== null) {
      loadWorks(libraryId);
      if (work) loadVolumes(libraryId, work);
    }
    if (selection?.kind === "recent") api.listRecent().then(setRecent);
  };

  const libraryMenu = (lib: Library, e: React.MouseEvent) => {
    const setDir = (d: Direction | null) => async () => {
      await api.setDirection("library", lib.id, null, d);
      await loadLibraries();
      if (libraryId === lib.id) loadWorks(lib.id);
    };
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: `읽는 방향: ${dirLabel.rtl}`, checked: lib.direction === "rtl", onClick: setDir("rtl") },
        { label: `읽는 방향: ${dirLabel.ltr}`, checked: lib.direction === "ltr", onClick: setDir("ltr") },
        { label: "읽는 방향: 기본 설정 따름", checked: lib.direction === null, onClick: setDir(null) },
        { label: "", separator: true },
        {
          label: "서재 삭제",
          danger: true,
          onClick: async () => {
            const ok = await ask(`'${lib.name}' 서재를 목록에서 뺍니다. 파일은 지우지 않습니다.`, {
              title: "서재 삭제",
              kind: "warning",
              okLabel: "삭제",
              cancelLabel: "취소",
            });
            if (!ok) return;
            await api.removeLibrary(lib.id);
            const libs = await loadLibraries();
            if (libraryId === lib.id) setSelection(libs.length ? { kind: "library", id: libs[0].id } : null);
          },
        },
      ],
    });
  };

  const workMenu = (w: WorkView, e: React.MouseEvent) => {
    if (libraryId === null) return;
    const setDir = (d: Direction | null) => async () => {
      await api.setDirection("folder", libraryId, w.id, d);
      loadWorks(libraryId);
    };
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: `읽는 방향: ${dirLabel.rtl}`, checked: w.folderDirection === "rtl", onClick: setDir("rtl") },
        { label: `읽는 방향: ${dirLabel.ltr}`, checked: w.folderDirection === "ltr", onClick: setDir("ltr") },
        { label: "읽는 방향: 서재 설정 따름", checked: w.folderDirection === null, onClick: setDir(null) },
      ],
    });
  };

  const shownWorks = useMemo(() => {
    if (!works) return null;
    const q = norm(query);
    return q ? works.filter((w) => norm(w.title).includes(q)) : works;
  }, [works, query]);

  if (reader) {
    return <Reader libraryId={reader.libraryId} initialKey={reader.key} onClose={closeReader} />;
  }

  const currentLib = libraries?.find((l) => l.id === libraryId);

  let body: React.ReactNode = null;
  if (libraries && libraries.length === 0) {
    body = (
      <div className="empty">
        <p>만화 폴더를 서재로 추가하세요.</p>
        <button className="btn primary" onClick={() => setAdding(true)}>서재 추가</button>
      </div>
    );
  } else if (selection?.kind === "recent") {
    body = recent && (
      <>
        <div className="crumbs"><b>최근 읽은 책</b></div>
        {recent.length === 0 ? (
          <div className="empty"><p>아직 읽은 책이 없습니다.</p></div>
        ) : (
          <div className="grid">
            {recent.map((r) => (
              <BookCard
                key={`${r.libraryId}:${r.key}`}
                title={r.title}
                sub={r.workTitle}
                coverUrl={r.coverUrl}
                progress={r.pageCount ? (r.page + 1) / r.pageCount : 0}
                direction={r.direction}
                onOpen={() => setReader({ libraryId: r.libraryId, key: r.key })}
              />
            ))}
          </div>
        )}
      </>
    );
  } else if (work && libraryId !== null) {
    body = (
      <>
        <div className="crumbs">
          <button className="link" onClick={() => setWork(null)}>{currentLib?.name}</button>
          <span>›</span>
          <b>{work.title}</b>
        </div>
        {volumes && <VolumeGrid volumes={volumes} onOpen={(v) => setReader({ libraryId, key: v.key })} />}
      </>
    );
  } else if (shownWorks) {
    body = (
      <>
        {error && <div className="banner">서재를 읽지 못했습니다: {error}</div>}
        {works && works.length === 0 && !error && <div className="empty"><p>이 폴더에서 ZIP 만화를 찾지 못했습니다.</p></div>}
        {works && works.length > 0 && shownWorks.length === 0 && <div className="empty"><p>'{query}'에 맞는 작품이 없습니다.</p></div>}
        <WorkGrid works={shownWorks} onOpen={openWork} onMenu={workMenu} />
      </>
    );
  } else if (libraryId !== null) {
    body = <div className="empty"><p>서재를 읽는 중…</p></div>;
  }

  return (
    <div className="app">
      <header className="topbar">
        <b>만화 뷰어</b>
        <span className="grow" />
        {libraryId !== null && !work && (
          <input className="search" placeholder="작품 검색" value={query} onChange={(e) => setQuery(e.target.value)} />
        )}
      </header>
      <div className="main">
        {libraries && (
          <Sidebar
            libraries={libraries}
            selection={selection}
            onSelect={(s) => {
              setWork(null);
              setVolumes(null);
              setSelection(s);
            }}
            onAdd={() => setAdding(true)}
            onLibraryMenu={libraryMenu}
          />
        )}
        <section className="content">{body}</section>
      </div>
      {menu && <ContextMenu menu={menu} onClose={() => setMenu(null)} />}
      {adding && (
        <AddLibraryDialog
          onClose={() => setAdding(false)}
          onAdded={async (id) => {
            setAdding(false);
            await loadLibraries();
            setSelection({ kind: "library", id });
          }}
        />
      )}
    </div>
  );
}
