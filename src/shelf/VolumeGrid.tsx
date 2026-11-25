import type { VolumeView } from "../api";
import { BookCard } from "./BookCard";

interface Props {
  volumes: VolumeView[];
  onOpen: (v: VolumeView) => void;
}

export function VolumeGrid({ volumes, onOpen }: Props) {
  return (
    <div className="grid">
      {volumes.map((v) => (
        <BookCard
          key={v.key}
          title={v.title}
          coverUrl={v.coverUrl}
          sub={v.page !== null ? `${v.page + 1} / ${v.pageCount}쪽` : `${v.pageCount}쪽`}
          progress={v.page !== null && v.pageCount ? (v.page + 1) / v.pageCount : 0}
          direction={v.direction}
          onOpen={() => onOpen(v)}
        />
      ))}
    </div>
  );
}
