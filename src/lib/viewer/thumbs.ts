import { extractJpegThumbnail } from "@/lib/viewer/exif-thumb";
import { toArrayBuffer } from "@/lib/viewer/bytes";
import type { Kind } from "@/lib/viewer/format";

const queue: Array<() => void> = [];
let active = 0;
const LIMIT = 2;

function pump() {
  while (active < LIMIT && queue.length > 0) {
    const job = queue.shift();
    if (!job) return;
    active += 1;
    job();
  }
}

function schedule<T>(fn: () => Promise<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    queue.push(() => {
      fn().then(resolve, reject).finally(() => {
        active -= 1;
        pump();
      });
    });
    pump();
  });
}

async function bitmapUrl(bitmap: ImageBitmap): Promise<string> {
  const max = 96;
  const scale = Math.min(1, max / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("thumb");
  ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob((b) => resolve(b), "image/png"));
  if (!blob) throw new Error("thumb");
  return URL.createObjectURL(blob);
}

export function requestThumb(file: File, kind: Kind): Promise<string> {
  return schedule(async () => {
    if (kind === "jpeg") {
      const head = await file.slice(0, 131072).arrayBuffer();
      const thumb = extractJpegThumbnail(head);
      if (thumb) return URL.createObjectURL(new Blob([toArrayBuffer(thumb)], { type: "image/jpeg" }));
    }
    if (kind === "svg") return URL.createObjectURL(file);
    const bitmap = await createImageBitmap(file, {
      resizeWidth: 96,
      resizeQuality: "low",
      imageOrientation: "from-image",
    });
    try {
      return await bitmapUrl(bitmap);
    } finally {
      bitmap.close();
    }
  });
}
