import type { Direction } from "../api";

export type Action = "next" | "prev";

export function actionForKey(e: { key: string; shiftKey: boolean }, dir: Direction): Action | null {
  if (e.key === " ") return e.shiftKey ? "prev" : "next";
  if (e.key === "ArrowLeft") return dir === "rtl" ? "next" : "prev";
  if (e.key === "ArrowRight") return dir === "rtl" ? "prev" : "next";
  return null;
}

/** Click on the left half moves toward the left: next for rtl books, previous for ltr. */
export function actionForClick(xRatio: number, dir: Direction): Action {
  const left = xRatio < 0.5;
  return left === (dir === "rtl") ? "next" : "prev";
}

export function actionForWheel(deltaY: number): Action | null {
  if (deltaY > 0) return "next";
  if (deltaY < 0) return "prev";
  return null;
}
