import type { Direction } from "../api";

/** Thin progress bar. Japanese (rtl) books fill from the right. */
export function Progress({ value, direction, className }: { value: number; direction: Direction; className?: string }) {
  const pct = Math.max(0, Math.min(1, value)) * 100;
  const side = direction === "rtl" ? { right: 0 } : { left: 0 };
  return (
    <div className={`progress ${className ?? ""}`}>
      <div data-fill className="progress-fill" style={{ ...side, width: `${pct}%` }} />
    </div>
  );
}
