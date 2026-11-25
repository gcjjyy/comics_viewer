import type { WorkView } from "../api";
import { BookCard } from "./BookCard";

interface Props {
  works: WorkView[];
  onOpen: (w: WorkView) => void;
  onMenu: (w: WorkView, e: React.MouseEvent) => void;
}

export function WorkGrid({ works, onOpen, onMenu }: Props) {
  return (
    <div className="grid">
      {works.map((w) => (
        <BookCard
          key={w.id}
          title={w.title}
          coverUrl={w.coverUrl}
          badge={`${w.volumeCount}권`}
          progress={w.volumeCount ? w.readVolumes / w.volumeCount : 0}
          direction={w.direction}
          stacked={w.volumeCount > 1}
          onOpen={() => onOpen(w)}
          onMenu={(e) => {
            e.preventDefault();
            onMenu(w, e);
          }}
        />
      ))}
    </div>
  );
}
