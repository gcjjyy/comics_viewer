import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, type Direction, type OpenedVolume } from "../api";
import { Progress } from "../shelf/Progress";
import { actionForClick, actionForKey, actionForWheel, type Action } from "./keys";
import { buildSpreads, screenOrder, spreadIndexOf, type Dim } from "./spread";
import { usePrefetch } from "./usePrefetch";

type Mode = "spread" | "single";
const MODE_KEY = "reader.mode";
const PREFETCH_AHEAD = 4;
const PREFETCH_BEHIND = 2;

function loadMode(): Mode {
  try {
    return localStorage.getItem(MODE_KEY) === "single" ? "single" : "spread";
  } catch {
    return "spread";
  }
}

interface Props {
  libraryId: number;
  initialKey: string;
  onClose: () => void;
}

export function Reader({ libraryId, initialKey, onClose }: Props) {
  const [vol, setVol] = useState<OpenedVolume | null>(null);
  const [page, setPage] = useState(0);
  const [dims, setDims] = useState<Dim[]>([]);
  const [mode, setMode] = useState<Mode>(loadMode);
  const [shift, setShift] = useState(false);
  const [dir, setDir] = useState<Direction>("rtl");
  const [barsVisible, setBarsVisible] = useState(true);
  const [failed, setFailed] = useState<Set<number>>(new Set());
  const [loadError, setLoadError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [reconnecting, setReconnecting] = useState(false);
  const hideTimer = useRef<number | undefined>(undefined);
  const wheelLock = useRef(0);

  const open = useCallback(
    async (key: string, at: "saved" | "first" | "last") => {
      try {
        const v = await api.openVolume(libraryId, key);
        setVol(v);
        setDims([]);
        setFailed(new Set());
        setShift(v.shift);
        setDir(v.direction);
        setPage(at === "saved" ? v.page : at === "first" ? 0 : Math.max(0, v.pages.length - 1));
        setLoadError(null);
      } catch (e) {
        setLoadError(String(e));
      }
    },
    [libraryId],
  );

  useEffect(() => {
    open(initialKey, "saved");
  }, [initialKey, open]);

  const count = vol?.pages.length ?? 0;
  const spreads = useMemo(() => buildSpreads(count, dims, shift, mode), [count, dims, shift, mode]);
  const idx = spreadIndexOf(spreads, page);
  const current = spreads[idx] ?? [];
  const first = current[0] ?? 0;

  // Save progress (debounced) as the first page of the visible spread.
  useEffect(() => {
    if (!vol) return;
    const t = window.setTimeout(() => api.saveProgress(libraryId, vol.key, first, count), 300);
    return () => window.clearTimeout(t);
  }, [vol, first, count, libraryId]);

  const prefetch = useMemo(() => {
    if (!vol) return [];
    const last = current[current.length - 1] ?? 0;
    const ahead = Array.from({ length: PREFETCH_AHEAD }, (_, i) => last + 1 + i).filter((p) => p < count);
    const behind = Array.from({ length: PREFETCH_BEHIND }, (_, i) => first - 1 - i).filter((p) => p >= 0);
    return [...ahead, ...behind].map((p) => vol.pages[p]);
  }, [vol, current, first, count]);
  usePrefetch(prefetch);

  const go = useCallback(
    (a: Action) => {
      if (!vol) return;
      if (a === "next") {
        if (idx + 1 < spreads.length) setPage(spreads[idx + 1][0]);
        else if (vol.nextKey) open(vol.nextKey, "first");
      } else {
        if (idx > 0) setPage(spreads[idx - 1][0]);
        else if (vol.prevKey) open(vol.prevKey, "last");
      }
    },
    [vol, idx, spreads, open],
  );

  const toggleDir = useCallback(async () => {
    if (!vol) return;
    const next: Direction = dir === "rtl" ? "ltr" : "rtl";
    setDir(next);
    // Back to the inherited direction clears the book override so folder/library changes apply again.
    await api.setDirection("book", libraryId, vol.key, next === vol.inheritedDirection ? null : next);
  }, [vol, dir, libraryId]);

  const toggleMode = useCallback(() => {
    setMode((m) => {
      const n = m === "spread" ? "single" : "spread";
      try {
        localStorage.setItem(MODE_KEY, n);
      } catch {
        /* per-viewer convenience only */
      }
      return n;
    });
  }, []);

  const toggleShift = useCallback(async () => {
    if (!vol) return;
    const n = !shift;
    setShift(n);
    await api.setShift(libraryId, vol.key, n);
  }, [vol, shift, libraryId]);

  const toggleFullscreen = useCallback(async () => {
    const w = getCurrentWindow();
    await w.setFullscreen(!(await w.isFullscreen()));
  }, []);

  useEffect(() => {
    const onKey = async (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const a = actionForKey(e, dir);
      if (a) {
        e.preventDefault();
        go(a);
        return;
      }
      switch (e.key.toLowerCase()) {
        case "d": return toggleDir();
        case "s": return toggleMode();
        case "a": return toggleShift();
        case "f": return toggleFullscreen();
        case "escape": {
          const w = getCurrentWindow();
          if (await w.isFullscreen()) await w.setFullscreen(false);
          else onClose();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dir, go, toggleDir, toggleMode, toggleShift, toggleFullscreen, onClose]);

  const showBars = () => {
    setBarsVisible(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => setBarsVisible(false), 2000);
  };
  useEffect(() => {
    showBars();
    return () => window.clearTimeout(hideTimer.current);
  }, []);

  const onWheel = (e: React.WheelEvent) => {
    const now = Date.now();
    if (now - wheelLock.current < 250) return;
    const a = actionForWheel(e.deltaY);
    if (!a) return;
    wheelLock.current = now;
    go(a);
  };

  const onStageClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    go(actionForClick((e.clientX - r.left) / r.width, dir));
  };

  const seek = (e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    let ratio = (e.clientX - r.left) / r.width;
    if (dir === "rtl") ratio = 1 - ratio;
    setPage(Math.min(count - 1, Math.max(0, Math.floor(ratio * count))));
  };

  const onImgLoad = (p: number, img: HTMLImageElement) => {
    setDims((d) => {
      if (d[p]) return d;
      const n = [...d];
      n[p] = { w: img.naturalWidth, h: img.naturalHeight };
      return n;
    });
  };

  if (loadError) {
    return (
      <div className="reader">
        <div className="reader-msg">
          <p>책을 열지 못했습니다: {loadError}</p>
          <button className="btn" onClick={onClose}>책장으로</button>
        </div>
      </div>
    );
  }
  async function reconnect() {
    setReconnecting(true);
    try {
      await api.reconnectLibrary(libraryId);
      setFailed(new Set());
      setRetry((r) => r + 1);
    } catch (e) {
      setLoadError(String(e));
    } finally {
      setReconnecting(false);
    }
  }

  if (!vol) return <div className="reader" />;

  const shown = screenOrder(current, dir);
  const lastShown = current[current.length - 1] ?? 0;

  return (
    <div className={`reader ${barsVisible ? "" : "hide-cursor"}`} onMouseMove={showBars} onWheel={onWheel}>
      <div className="stage" onClick={onStageClick}>
        {shown.map((p) => (
          <div key={p} className={`page ${shown.length === 2 ? "half" : "full"} ${shown.length === 2 ? (p === shown[0] ? "left" : "right") : ""}`}>
            {failed.has(p) ? (
              <div className="page-error" onClick={(e) => e.stopPropagation()}>
                페이지를 불러오지 못했습니다
                {vol.remote && (
                  <button className="btn" disabled={reconnecting} onClick={reconnect}>
                    {reconnecting ? "연결 중…" : "다시 연결"}
                  </button>
                )}
              </div>
            ) : (
              <img
                src={retry ? `${vol.pages[p]}?r=${retry}` : vol.pages[p]}
                alt=""
                draggable={false}
                onLoad={(e) => onImgLoad(p, e.currentTarget)}
                onError={() => setFailed((f) => new Set(f).add(p))}
              />
            )}
          </div>
        ))}
      </div>

      <div className={`bar top ${barsVisible ? "" : "hidden"}`} onMouseEnter={showBars}>
        <button className="btn" onClick={onClose} title="Esc">← 책장</button>
        <span className="bar-title">{vol.workTitle} · {vol.title}</span>
        <span className="grow" />
        <button className="chip" onClick={toggleDir} title="이 책의 읽는 방향 바꾸기 (D)">
          {dir === "rtl" ? "일본 만화 · 오른쪽→왼쪽" : "한국 만화 · 왼쪽→오른쪽"}
        </button>
        <button className="chip" onClick={toggleMode} title="한 쪽/두 쪽 보기 (S)">{mode === "spread" ? "두 쪽" : "한 쪽"}</button>
        {mode === "spread" && (
          <button className={`chip ${shift ? "on" : ""}`} onClick={toggleShift} title="펼침 짝이 어긋났을 때 한 쪽 밀기 (A)">짝 밀기</button>
        )}
        <button className="chip" onClick={toggleFullscreen} title="전체 화면 (F)">전체 화면</button>
      </div>

      <div className={`bar bottom ${barsVisible ? "" : "hidden"}`} onMouseEnter={showBars}>
        <span className="page-no">{dir === "rtl" ? count : 1}</span>
        <div className="seek" onClick={seek}>
          <Progress value={count ? (lastShown + 1) / count : 0} direction={dir} />
        </div>
        <span className="page-no">{dir === "rtl" ? 1 : count}</span>
        <span className="page-now">
          {current.map((p) => p + 1).join("–")} / {count}쪽
        </span>
      </div>
    </div>
  );
}
