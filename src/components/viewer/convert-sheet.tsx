import { useState } from "react";
import { extensionFor, type OutMime } from "@/lib/viewer/convert";

const FORMATS: { mime: OutMime; label: string }[] = [
  { mime: "image/png", label: "PNG" },
  { mime: "image/jpeg", label: "JPEG" },
  { mime: "image/webp", label: "WebP" },
];

const LONGS: { label: string; value: number | null }[] = [
  { label: "원본", value: null },
  { label: "1920", value: 1920 },
  { label: "1280", value: 1280 },
  { label: "800", value: 800 },
];

type Props = {
  gif: boolean;
  svg: boolean;
  busy: boolean;
  onSave: (opts: { mime: OutMime; quality: number; maxLong: number | null; ext: string }) => void;
  onClose: () => void;
};

export function ConvertSheet({ gif, svg, busy, onSave, onClose }: Props) {
  const [mime, setMime] = useState<OutMime>("image/png");
  const [quality, setQuality] = useState(90);
  const [maxLong, setMaxLong] = useState<number | null>(null);

  return (
    <div className="absolute right-2 bottom-2 left-2 z-30 border border-border bg-surface p-3 shadow-none md:left-auto md:w-80">
      <div className="mb-3 flex items-center justify-between">
        <p className="text-sm font-medium">변환</p>
        <button type="button" className="h-11 px-2 text-sm text-muted" onClick={onClose}>
          닫기
        </button>
      </div>
      <div className="mb-3 flex gap-1">
        {FORMATS.map((fmt) => (
          <button
            key={fmt.mime}
            type="button"
            onClick={() => setMime(fmt.mime)}
            className={
              "h-11 flex-1 text-sm " +
              (mime === fmt.mime ? "bg-accent text-accent-fg" : "bg-surface-2 text-fg")
            }
          >
            {fmt.label}
          </button>
        ))}
      </div>
      <label className="mb-3 block text-sm text-muted">
        품질 {mime === "image/png" ? "—" : quality}
        <input
          type="range"
          min={40}
          max={100}
          value={quality}
          disabled={mime === "image/png"}
          onChange={(event) => setQuality(Number(event.target.value))}
          className="mt-1 block w-full disabled:opacity-40"
        />
      </label>
      <p className="mb-1 text-sm text-muted">긴 변</p>
      <div className="mb-3 flex gap-1">
        {LONGS.map((opt) => (
          <button
            key={opt.label}
            type="button"
            onClick={() => setMaxLong(opt.value)}
            className={
              "h-11 flex-1 text-sm " +
              (maxLong === opt.value ? "bg-accent text-accent-fg" : "bg-surface-2 text-fg")
            }
          >
            {opt.label}
          </button>
        ))}
      </div>
      {gif ? <p className="mb-3 text-sm text-muted">GIF는 첫 프레임만 저장합니다.</p> : null}
      {svg ? <p className="mb-3 text-sm text-muted">SVG는 픽셀로 굽습니다.</p> : null}
      <button
        type="button"
        disabled={busy}
        onClick={() => onSave({ mime, quality, maxLong, ext: extensionFor(mime) })}
        className="h-11 w-full bg-accent text-sm font-medium text-accent-fg disabled:opacity-40"
      >
        {busy ? "저장 중" : "저장"}
      </button>
    </div>
  );
}
