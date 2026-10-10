import { useCallback, useEffect, useImperativeHandle, useRef, useState, type RefObject } from "react";

export type StageApi = {
  fit: () => void;
  actual: () => void;
  zoomBy: (factor: number) => void;
};

type Mode = "fit" | "actual" | "custom";

type Props = {
  source: CanvasImageSource | null;
  width: number;
  height: number;
  pixelated: boolean;
  apiRef: RefObject<StageApi | null>;
  onPercent: (percent: number) => void;
};

const MIN = 0.05;
const MAX = 32;

export function Stage({ source, width, height, pixelated, apiRef, onPercent }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [mode, setMode] = useState<Mode>("fit");
  const [scale, setScale] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number; px: number; py: number } | null>(null);
  const view = useRef({ mode, scale, pan, width, height, pixelated, source });
  view.current = { mode, scale, pan, width, height, pixelated, source };

  const layout = useCallback((nextMode: Mode, nextScale: number, anchor?: { x: number; y: number; ix: number; iy: number }) => {
    const wrap = wrapRef.current;
    if (!wrap || width < 1 || height < 1) return;
    const vw = wrap.clientWidth;
    const vh = wrap.clientHeight;
    let s = nextScale;
    if (nextMode === "fit") s = Math.min(vw / width, vh / height);
    if (nextMode === "actual") s = 1;
    s = Math.min(MAX, Math.max(MIN, s));
    let x: number;
    let y: number;
    if (anchor && nextMode === "custom") {
      x = anchor.x - anchor.ix * s;
      y = anchor.y - anchor.iy * s;
    } else if (nextMode === "custom") {
      const prev = view.current.scale || 1;
      const cx = vw / 2;
      const cy = vh / 2;
      const ix = (cx - view.current.pan.x) / prev;
      const iy = (cy - view.current.pan.y) / prev;
      x = cx - ix * s;
      y = cy - iy * s;
    } else {
      x = (vw - width * s) / 2;
      y = (vh - height * s) / 2;
    }
    setMode(nextMode);
    setScale(s);
    setPan({ x, y });
    onPercent(Math.round(s * 100));
  }, [height, onPercent, width]);

  useImperativeHandle(apiRef, () => ({
    fit: () => layout("fit", 1),
    actual: () => layout("actual", 1),
    zoomBy: (factor: number) => layout("custom", view.current.scale * factor),
  }), [layout]);

  useEffect(() => {
    layout("fit", 1);
  }, [width, height, layout]);

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const ro = new ResizeObserver(() => {
      if (view.current.mode === "fit") layout("fit", 1);
    });
    ro.observe(wrap);
    return () => ro.disconnect();
  }, [layout]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;
    const dpr = window.devicePixelRatio || 1;
    const vw = wrap.clientWidth;
    const vh = wrap.clientHeight;
    canvas.width = Math.max(1, Math.round(vw * dpr));
    canvas.height = Math.max(1, Math.round(vh * dpr));
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const styles = getComputedStyle(document.documentElement);
    const a = styles.getPropertyValue("--color-check-a").trim() || "#171613";
    const b = styles.getPropertyValue("--color-check-b").trim() || "#221f1b";
    const cell = 16;
    for (let y = 0; y < vh; y += cell) {
      for (let x = 0; x < vw; x += cell) {
        ctx.fillStyle = ((x / cell + y / cell) & 1) === 0 ? a : b;
        ctx.fillRect(x, y, cell, cell);
      }
    }
    if (!source || width < 1) return;
    ctx.imageSmoothingEnabled = !pixelated;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(source, pan.x, pan.y, width * scale, height * scale);
  }, [source, pan, scale, pixelated, width, height]);

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const onWheel = (event: WheelEvent) => {
      if (!view.current.source) return;
      event.preventDefault();
      const rect = wrap.getBoundingClientRect();
      const x = event.clientX - rect.left;
      const y = event.clientY - rect.top;
      const prev = view.current.scale;
      const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
      const ix = (x - view.current.pan.x) / prev;
      const iy = (y - view.current.pan.y) / prev;
      layout("custom", prev * factor, { x, y, ix, iy });
    };
    wrap.addEventListener("wheel", onWheel, { passive: false });
    return () => wrap.removeEventListener("wheel", onWheel);
  }, [layout]);

  const onPointerDown = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (!source) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { x: event.clientX, y: event.clientY, px: pan.x, py: pan.y };
  };
  const onPointerMove = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    if (!d) return;
    setMode("custom");
    setPan({ x: d.px + event.clientX - d.x, y: d.py + event.clientY - d.y });
  };
  const onPointerUp = () => {
    drag.current = null;
  };

  return (
    <div ref={wrapRef} className="checker relative min-h-0 min-w-0 flex-1 touch-none">
      {source && width > 0 ? (
        <canvas
          ref={canvasRef}
          className="absolute inset-0 h-full w-full"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
        />
      ) : null}
    </div>
  );
}
