import type { Direction } from "../api";
import { Progress } from "./Progress";

interface Props {
  title: string;
  coverUrl: string;
  badge?: string;
  sub?: string;
  progress?: number;
  direction: Direction;
  stacked?: boolean;
  onOpen: () => void;
  onMenu?: (e: React.MouseEvent) => void;
}

export function BookCard({ title, coverUrl, badge, sub, progress, direction, stacked, onOpen, onMenu }: Props) {
  return (
    <button className="card" onClick={onOpen} onContextMenu={onMenu} title={title}>
      {/* Wide wrap-around cover scans: the front cover is on the left for rtl books, right for ltr. */}
      <div className={`cover ${stacked ? "stacked" : ""}`}>
        <img
          src={coverUrl}
          loading="lazy"
          alt=""
          draggable={false}
          style={{ objectPosition: direction === "rtl" ? "left center" : "right center" }} onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
        {badge && <span className="badge">{badge}</span>}
      </div>
      <div className="card-title">{title}</div>
      {sub && <div className="card-sub">{sub}</div>}
      {progress !== undefined && <Progress value={progress} direction={direction} />}
    </button>
  );
}
