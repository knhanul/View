import { sniffKind, type Kind } from "@/lib/viewer/format";

export type RasterView = {
  type: "raster";
  kind: Kind;
  bitmap: ImageBitmap;
  width: number;
  height: number;
  decodeMs: number;
};

export type SvgView = {
  type: "svg";
  kind: "svg";
  image: HTMLImageElement;
  url: string;
  width: number;
  height: number;
  decodeMs: number;
};

export type GifView = {
  type: "gif";
  kind: "gif";
  bitmap: ImageBitmap;
  width: number;
  height: number;
  decodeMs: number;
  frames: ImageBitmap[] | null;
  delays: number[] | null;
  frame: number;
  playing: boolean;
};

export type LoadedView = RasterView | SvgView | GifView;

export function closeView(view: LoadedView | null) {
  if (!view) return;
  if (view.type === "svg") {
    URL.revokeObjectURL(view.url);
    return;
  }
  if (view.type === "raster") {
    view.bitmap.close();
    return;
  }
  view.bitmap.close();
  view.frames?.forEach((frame) => {
    if (frame !== view.bitmap) frame.close();
  });
}

function viewBoxSize(text: string): { w: number; h: number } | null {
  const m = text.match(/viewBox\s*=\s*["']\s*[-\d.]+\s+[-\d.]+\s+([\d.]+)\s+([\d.]+)/i);
  if (!m) return null;
  const w = Math.ceil(Number(m[1]));
  const h = Math.ceil(Number(m[2]));
  if (!w || !h) return null;
  return { w, h };
}

export async function loadImage(file: File): Promise<LoadedView> {
  const kind = await sniffKind(file);
  if (kind === "other") throw new Error("이 형식은 열 수 없습니다");
  const t0 = performance.now();

  if (kind === "svg") {
    const url = URL.createObjectURL(file);
    try {
      const image = new Image();
      image.src = url;
      await image.decode();
      const text = await file.text();
      const box = viewBoxSize(text);
      const explicit = /<svg\b[^>]*\swidth\s*=/i.test(text);
      let w = image.naturalWidth;
      let h = image.naturalHeight;
      if ((!explicit || !w || !h) && box) {
        w = box.w;
        h = box.h;
      }
      if (!w || !h) {
        w = 512;
        h = 512;
      }
      return { type: "svg", kind: "svg", image, url, width: w, height: h, decodeMs: performance.now() - t0 };
    } catch (error) {
      URL.revokeObjectURL(url);
      throw error instanceof Error ? error : new Error("SVG를 열 수 없습니다");
    }
  }

  const bitmap = await createImageBitmap(file, { imageOrientation: "from-image" });
  const decodeMs = performance.now() - t0;
  if (kind === "gif") {
    return {
      type: "gif",
      kind: "gif",
      bitmap,
      width: bitmap.width,
      height: bitmap.height,
      decodeMs,
      frames: null,
      delays: null,
      frame: 0,
      playing: false,
    };
  }
  return { type: "raster", kind, bitmap, width: bitmap.width, height: bitmap.height, decodeMs };
}
