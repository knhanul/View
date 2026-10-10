import { toArrayBuffer } from "@/lib/viewer/bytes";

const BLINK_GIF =
  "R0lGODlhMAAgAJEAABIRD+KhWvTw6DozLCH/C05FVFNDQVBFMi4wAwEAAAAh+QQEHgAAACwAAAAAMAAgAAACa5yPqcuNAKOctNoLB968A+2F4jeWHGimFKq2pOuyQEDX9h3A2YT3tf6K+HxA2bBX5B1vScmSqTM+f1HlNFd1XrEw6bUp3IIhW27MOh3PxNnwt01md4EqGT1kv3fy+g2/f/EHWCE4OFFoGFEAACH5BAQeAAAALAAAAAAwACAAAAJrnI+py40Ao5y02gsH3rwD7YXiN5YcaKYUqrak67JqQNf2HawwgPe1DvP5gC5hj9gy4pAzpY2Zcj4nsqiUBjVdsdTdNtcNbrOlL3lkDhfH6iRbUtW+I/FdqG7v4PObPf/i91cRKNhWeHIYUgAAOw==";

function decodeBase64(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function canvasBlob(
  draw: (ctx: CanvasRenderingContext2D, w: number, h: number) => void,
  type: string,
  w: number,
  h: number,
  quality?: number,
) {
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas");
  draw(ctx, w, h);
  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error("sample"))), type, quality);
  });
}

export async function makeSamples(): Promise<File[]> {
  const jpeg = await canvasBlob(
    (ctx, w, h) => {
      ctx.fillStyle = "#3a2a22";
      ctx.fillRect(0, 0, w, h);
      const g = ctx.createLinearGradient(0, 0, w, h);
      g.addColorStop(0, "#e2a15a");
      g.addColorStop(1, "#6a3a28");
      ctx.fillStyle = g;
      ctx.fillRect(36, 28, w - 72, h - 56);
      ctx.fillStyle = "#f4f0e8";
      ctx.fillRect(64, h / 2 - 8, w - 128, 16);
    },
    "image/jpeg",
    640,
    400,
    0.85,
  );

  const png = await canvasBlob((ctx, w, h) => {
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = "#e2a15a";
    ctx.beginPath();
    ctx.arc(180, 140, 90, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "rgba(244,240,232,0.9)";
    ctx.beginPath();
    ctx.arc(250, 180, 70, 0, Math.PI * 2);
    ctx.fill();
  }, "image/png", 420, 320);

  const svg = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 200">
  <rect width="320" height="200" fill="#1c1b18"/>
  <circle cx="160" cy="100" r="62" fill="none" stroke="#e2a15a" stroke-width="10"/>
  <path d="M118 100h84M160 58v84" stroke="#f4f0e8" stroke-width="8" stroke-linecap="square"/>
</svg>`;

  return [
    new File([jpeg], "warm.jpg", { type: "image/jpeg" }),
    new File([png], "alpha.png", { type: "image/png" }),
    new File([toArrayBuffer(decodeBase64(BLINK_GIF))], "blink.gif", { type: "image/gif" }),
    new File([svg], "mark.svg", { type: "image/svg+xml" }),
  ];
}
