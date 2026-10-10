export type OutMime = "image/png" | "image/jpeg" | "image/webp";

export type ConvertOptions = {
  mime: OutMime;
  quality: number;
  maxLong: number | null;
};

export async function convertImage(
  source: CanvasImageSource,
  srcW: number,
  srcH: number,
  opts: ConvertOptions,
): Promise<Blob> {
  const long = Math.max(srcW, srcH);
  let w = srcW;
  let h = srcH;
  if (opts.maxLong && opts.maxLong > 0 && opts.maxLong < long) {
    const s = opts.maxLong / long;
    w = Math.max(1, Math.round(srcW * s));
    h = Math.max(1, Math.round(srcH * s));
  }
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("캔버스를 만들 수 없습니다");
  if (opts.mime === "image/jpeg") {
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, w, h);
  }
  ctx.drawImage(source, 0, 0, w, h);
  const quality = opts.mime === "image/png" ? undefined : opts.quality / 100;
  const blob = await new Promise<Blob | null>((resolve) => {
    canvas.toBlob((b) => resolve(b), opts.mime, quality);
  });
  if (!blob) throw new Error("변환에 실패했습니다");
  return blob;
}

export function extensionFor(mime: OutMime): string {
  if (mime === "image/jpeg") return "jpg";
  if (mime === "image/webp") return "webp";
  return "png";
}

export function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1500);
}
