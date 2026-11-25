import { useEffect, useRef } from "react";

/** Requests the given page URLs ahead of time so turning pages is instant. */
export function usePrefetch(urls: string[]) {
  const held = useRef<HTMLImageElement[]>([]);
  const key = urls.join("\n");
  useEffect(() => {
    held.current = urls.map((u) => {
      const img = new Image();
      img.decoding = "async";
      img.src = u;
      return img;
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
}
