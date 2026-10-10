import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, FolderOpen, Images, PanelLeft, Pause, Play } from "lucide-react";
import { ConvertSheet } from "@/components/viewer/convert-sheet";
import { Explorer, type LibraryItem } from "@/components/viewer/explorer";
import { Stage, type StageApi } from "@/components/viewer/stage";
import { convertImage, downloadBlob } from "@/lib/viewer/convert";
import { formatBytes, formatMs, isImageName, kindFromName, stemOf } from "@/lib/viewer/format";
import { closeView, loadImage, type LoadedView } from "@/lib/viewer/load-image";
import { makeSamples } from "@/lib/viewer/samples";

let seq = 0;

function toItems(files: File[]): LibraryItem[] {
  return files.filter((file) => isImageName(file.name)).map((file) => ({
    id: `${Date.now()}-${seq++}`,
    name: file.name,
    file,
    kind: kindFromName(file.name),
    size: file.size,
  }));
}

export function ViewerApp() {
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [index, setIndex] = useState(0);
  const [view, setView] = useState<LoadedView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [percent, setPercent] = useState(100);
  const [explorer, setExplorer] = useState(false);
  const [convertOpen, setConvertOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [frameNote, setFrameNote] = useState<string | null>(null);
  const stageRef = useRef<StageApi | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const folderRef = useRef<HTMLInputElement>(null);
  const viewRef = useRef<LoadedView | null>(null);
  viewRef.current = view;

  const replaceItems = useCallback((next: LibraryItem[]) => {
    setItems(next);
    setIndex(0);
    setConvertOpen(false);
    if (next.length > 1 && window.matchMedia("(min-width: 768px)").matches) setExplorer(true);
  }, []);

  const addFiles = useCallback((files: File[]) => {
    const next = toItems(files).sort((a, b) => a.name.localeCompare(b.name, "ko", { numeric: true }));
    if (next.length === 0) {
      setError("JPG, PNG, GIF, SVG, WebP만 열 수 있습니다");
      return;
    }
    replaceItems(next);
  }, [replaceItems]);

  useEffect(() => {
    const item = items[index];
    if (!item) {
      closeView(viewRef.current);
      setView(null);
      setError(null);
      setFrameNote(null);
      return;
    }
    const gen = item.id;
    const ac = new AbortController();
    let cancelled = false;
    setLoading(true);
    setError(null);
    setFrameNote(null);
    loadImage(item.file)
      .then(async (loaded) => {
        if (cancelled) {
          closeView(loaded);
          return;
        }
        closeView(viewRef.current);
        setView(loaded);
        setLoading(false);
        if (loaded.type !== "gif") return;
        setFrameNote("프레임 읽는 중");
        const { decodeGifFrames } = await import("@/lib/viewer/gif-frames");
        if (cancelled) return;
        const decoded = await decodeGifFrames(item.file, ac.signal);
        if (cancelled || !decoded) {
          decoded?.frames.forEach((frame) => frame.close());
          if (!cancelled) setFrameNote(null);
          return;
        }
        setFrameNote(null);
        setView((current) => {
          if (!current || current.type !== "gif") {
            decoded.frames.forEach((frame) => frame.close());
            return current;
          }
          const nextBitmap = decoded.frames[0] ?? current.bitmap;
          const orphan = current.bitmap === nextBitmap ? null : current.bitmap;
          if (orphan) requestAnimationFrame(() => orphan.close());
          return {
            ...current,
            frames: decoded.frames,
            delays: decoded.delays,
            frame: 0,
            bitmap: nextBitmap,
          };
        });
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        closeView(viewRef.current);
        setView(null);
        setLoading(false);
        setError(err instanceof Error ? err.message : "열 수 없습니다");
      });
    return () => {
      cancelled = true;
      ac.abort();
    };
  }, [items, index]);

  const playing = view?.type === "gif" && view.playing;
  const frameCount = view?.type === "gif" ? (view.frames?.length ?? 0) : 0;

  useEffect(() => {
    if (!playing || frameCount < 1) return;
    const current = viewRef.current;
    if (!current || current.type !== "gif" || !current.frames || !current.delays) return;
    const frames = current.frames;
    const delays = current.delays;
    let frame = current.frame;
    let raf = 0;
    let last = performance.now();
    let acc = 0;
    const tick = (now: number) => {
      acc += now - last;
      last = now;
      let changed = false;
      while (acc >= (delays[frame] ?? 100)) {
        acc -= delays[frame] ?? 100;
        frame = (frame + 1) % frames.length;
        changed = true;
      }
      if (changed) {
        const bitmap = frames[frame];
        if (bitmap) {
          setView((prev) => (prev && prev.type === "gif" ? { ...prev, frame, bitmap } : prev));
        }
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [playing, frameCount]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
      const key = event.key.toLowerCase();
      if (event.key === "ArrowLeft") {
        event.preventDefault();
        setIndex((i) => Math.max(0, i - 1));
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        setIndex((i) => Math.min(items.length - 1, i + 1));
      } else if (key === "f") stageRef.current?.fit();
      else if (key === "1") stageRef.current?.actual();
      else if (key === "+" || key === "=") stageRef.current?.zoomBy(1.1);
      else if (key === "-" || key === "_") stageRef.current?.zoomBy(1 / 1.1);
      else if (key === "c") setConvertOpen((open) => !open);
      else if (key === "o") fileRef.current?.click();
      else if (event.key === " " && viewRef.current?.type === "gif") {
        event.preventDefault();
        setView((current) => (current && current.type === "gif" && current.frames ? { ...current, playing: !current.playing } : current));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [items.length]);

  const item = items[index] ?? null;
  const source = view?.type === "svg" ? view.image : view?.bitmap ?? null;
  const pixelated = !!view && view.type !== "svg" && percent >= 200;

  const onSave = async (opts: { mime: "image/png" | "image/jpeg" | "image/webp"; quality: number; maxLong: number | null; ext: string }) => {
    if (!view || !item) return;
    setSaving(true);
    try {
      const sourceImage =
        view.type === "gif" ? (view.frames?.[0] ?? view.bitmap) : view.type === "svg" ? view.image : view.bitmap;
      const blob = await convertImage(sourceImage, view.width, view.height, opts);
      downloadBlob(blob, `${stemOf(item.name)}.${opts.ext}`);
      setConvertOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "변환에 실패했습니다");
    } finally {
      setSaving(false);
    }
  };

  return (
    <main
      className="flex h-dvh flex-col bg-bg text-fg"
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault();
        addFiles([...event.dataTransfer.files]);
      }}
    >
      <header className="flex h-14 shrink-0 items-center gap-2 overflow-x-auto border-b border-border px-2 md:px-3">
        <h1 className="flex shrink-0 items-end gap-1.5">
          <img src="/nuni-logo.png" alt="누니뷰" className="h-10 w-auto" />
          <span aria-hidden="true" className="pb-1 text-sm font-medium tracking-tight text-accent">
            view
          </span>
        </h1>
        <button type="button" className="h-11 shrink-0 bg-surface-2 px-3 text-sm" onClick={() => fileRef.current?.click()}>
          열기
        </button>
        <button type="button" className="flex h-11 shrink-0 items-center gap-1 bg-surface-2 px-3 text-sm" onClick={() => folderRef.current?.click()}>
          <FolderOpen className="size-4" aria-hidden="true" />
          폴더
        </button>
        <button
          type="button"
          className="flex h-11 shrink-0 items-center gap-1 px-3 text-sm text-muted"
          onClick={() => {
            void makeSamples().then(addFiles);
          }}
        >
          <Images className="size-4" aria-hidden="true" />
          샘플
        </button>
        <p className="min-w-0 flex-1 truncate text-sm">{item?.name ?? "파일 없음"}</p>
        <p className="hidden shrink-0 font-mono text-xs text-muted md:block">
          {view ? `${view.width}×${view.height}` : "—"}
          {view ? ` · ${percent}%` : ""}
          {item ? ` · ${formatBytes(item.size)}` : ""}
          {loading ? " · 여는 중" : view ? ` · ${formatMs(view.decodeMs)}` : ""}
        </p>
      </header>

      <div className="relative flex min-h-0 flex-1">
        {explorer && items.length > 0 ? (
          <aside className="absolute inset-y-0 left-0 z-20 flex w-64 flex-col border-r border-border bg-surface md:static">
            <div className="flex h-11 items-center justify-between px-3 text-xs text-muted">
              <span className="font-mono">{items.length}개</span>
              <button type="button" className="h-11 px-2 md:hidden" onClick={() => setExplorer(false)}>
                닫기
              </button>
            </div>
            <Explorer items={items} index={index} onPick={setIndex} />
          </aside>
        ) : null}
        <Stage
          key={item?.id ?? "empty"}
          apiRef={stageRef}
          source={source}
          width={view?.width ?? 0}
          height={view?.height ?? 0}
          pixelated={pixelated}
          onPercent={setPercent}
        />
        {!item ? (
          <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-2 px-6 text-center">
            <img src="/favicon.svg" alt="" className="mb-1 h-16 w-16" />
            <p className="text-base">이미지를 놓거나 열기를 누르세요</p>
            <p className="text-sm text-muted">← → 이동 · F 맞춤 · 1 원본 · C 변환 · O 열기</p>
          </div>
        ) : null}
        {error ? (
          <p className="pointer-events-none absolute bottom-3 left-3 bg-surface px-3 py-2 text-sm text-danger">{error}</p>
        ) : null}
        {convertOpen && view && item ? (
          <ConvertSheet
            gif={view.type === "gif"}
            svg={view.type === "svg"}
            busy={saving}
            onClose={() => setConvertOpen(false)}
            onSave={(opts) => {
              void onSave(opts);
            }}
          />
        ) : null}
      </div>

      <footer className="flex h-14 shrink-0 items-center gap-1 overflow-x-auto border-t border-border px-2">
        <button type="button" className="flex h-11 w-11 items-center justify-center bg-surface-2 disabled:opacity-40" disabled={index <= 0} onClick={() => setIndex((i) => i - 1)} aria-label="이전">
          <ChevronLeft className="size-5" />
        </button>
        <button type="button" className="flex h-11 w-11 items-center justify-center bg-surface-2 disabled:opacity-40" disabled={index >= items.length - 1} onClick={() => setIndex((i) => i + 1)} aria-label="다음">
          <ChevronRight className="size-5" />
        </button>
        <button type="button" className="h-11 bg-surface-2 px-3 text-sm" onClick={() => stageRef.current?.fit()}>
          맞춤
        </button>
        <button type="button" className="h-11 bg-surface-2 px-3 text-sm" onClick={() => stageRef.current?.actual()}>
          1:1
        </button>
        <button
          type="button"
          className="flex h-11 w-11 items-center justify-center bg-surface-2 disabled:opacity-40"
          disabled={items.length === 0}
          onClick={() => setExplorer((open) => !open)}
          aria-label="목록"
        >
          <PanelLeft className="size-4" />
        </button>
        {view?.type === "gif" ? (
          <div className="flex min-w-0 flex-1 items-center gap-2 px-1">
            <button
              type="button"
              className="flex h-11 w-11 items-center justify-center bg-surface-2 disabled:opacity-40"
              disabled={!view.frames}
              aria-label={view.playing ? "일시정지" : "재생"}
              onClick={() => setView((current) => (current && current.type === "gif" ? { ...current, playing: !current.playing } : current))}
            >
              {view.playing ? <Pause className="size-4" /> : <Play className="size-4" />}
            </button>
            <span className="shrink-0 font-mono text-xs text-muted">
              {view.frames ? `${view.frame + 1}/${view.frames.length}` : frameNote ?? "1"}
            </span>
          </div>
        ) : (
          <span className="flex-1" />
        )}
        <p className="shrink-0 font-mono text-xs text-muted md:hidden">
          {view ? `${view.width}×${view.height} ${formatMs(view.decodeMs)}` : ""}
        </p>
        <button
          type="button"
          className={"h-11 px-4 text-sm " + (convertOpen ? "bg-accent text-accent-fg" : "bg-surface-2 text-fg")}
          disabled={!view}
          onClick={() => setConvertOpen((open) => !open)}
        >
          변환
        </button>
      </footer>

      <input
        ref={fileRef}
        type="file"
        accept="image/jpeg,image/png,image/gif,image/webp,image/svg+xml,.jpg,.jpeg,.png,.gif,.svg,.webp"
        multiple
        className="hidden"
        onChange={(event) => {
          addFiles([...(event.target.files ?? [])]);
          event.target.value = "";
        }}
      />
      <input
        ref={folderRef}
        type="file"
        multiple
        className="hidden"
        onChange={(event) => {
          addFiles([...(event.target.files ?? [])]);
          event.target.value = "";
        }}
        {...{ webkitdirectory: "", directory: "" }}
      />
    </main>
  );
}
