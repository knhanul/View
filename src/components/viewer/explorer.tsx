import { useEffect, useRef, useState } from "react";
import { kindLabel, type Kind } from "@/lib/viewer/format";
import { requestThumb } from "@/lib/viewer/thumbs";

export type LibraryItem = {
  id: string;
  name: string;
  file: File;
  kind: Kind;
  size: number;
};

const ROW = 48;

type Props = {
  items: LibraryItem[];
  index: number;
  onPick: (index: number) => void;
};

export function Explorer({ items, index, onPick }: Props) {
  const scroller = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ top: 0, height: 480 });
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const thumbsRef = useRef(thumbs);
  thumbsRef.current = thumbs;
  const started = useRef(new Set<string>());

  useEffect(() => {
    started.current = new Set();
    setThumbs({});
    return () => {
      Object.values(thumbsRef.current).forEach((url) => URL.revokeObjectURL(url));
    };
  }, [items]);

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const sync = () => setBox({ top: el.scrollTop, height: el.clientHeight });
    sync();
    el.addEventListener("scroll", sync, { passive: true });
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", sync);
      ro.disconnect();
    };
  }, []);

  const start = Math.max(0, Math.floor(box.top / ROW) - 4);
  const end = Math.min(items.length, Math.ceil((box.top + box.height) / ROW) + 6);

  useEffect(() => {
    for (let i = start; i < end; i += 1) {
      const item = items[i];
      if (!item || started.current.has(item.id)) continue;
      started.current.add(item.id);
      const id = item.id;
      requestThumb(item.file, item.kind)
        .then((url) => {
          if (!started.current.has(id)) {
            URL.revokeObjectURL(url);
            return;
          }
          setThumbs((prev) => ({ ...prev, [id]: url }));
        })
        .catch(() => {
          /* thumbnail is optional */
        });
    }
  }, [start, end, items]);

  return (
    <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto">
      <div className="relative" style={{ height: items.length * ROW }}>
        {items.slice(start, end).map((item, offset) => {
          const i = start + offset;
          const active = i === index;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onPick(i)}
              className={
                "absolute right-0 left-0 flex h-12 items-center gap-2 px-2 text-left " +
                (active ? "bg-surface-2 text-fg" : "text-fg hover:bg-surface-2")
              }
              style={{ top: i * ROW }}
            >
              <span className={"h-full w-0.5 shrink-0 " + (active ? "bg-accent" : "bg-transparent")} />
              <span className="flex size-9 shrink-0 items-center justify-center overflow-hidden bg-bg">
                {thumbs[item.id] ? (
                  <img src={thumbs[item.id]} alt="" className="size-full object-cover" />
                ) : (
                  <span className="font-mono text-xs text-muted">{kindLabel(item.kind)}</span>
                )}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm">{item.name}</span>
                <span className="font-mono text-xs text-muted">{kindLabel(item.kind)}</span>
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
