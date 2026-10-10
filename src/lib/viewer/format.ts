export type Kind = "jpeg" | "png" | "gif" | "svg" | "webp" | "other";

const EXT: Record<string, Kind> = {
  jpg: "jpeg",
  jpeg: "jpeg",
  png: "png",
  gif: "gif",
  svg: "svg",
  webp: "webp",
};

export function extensionOf(name: string): string {
  const i = name.lastIndexOf(".");
  return i >= 0 ? name.slice(i + 1).toLowerCase() : "";
}

export function kindFromName(name: string): Kind {
  return EXT[extensionOf(name)] ?? "other";
}

export function isImageName(name: string): boolean {
  return kindFromName(name) !== "other";
}

export async function sniffKind(file: File): Promise<Kind> {
  const byName = kindFromName(file.name);
  const head = new Uint8Array(await file.slice(0, 256).arrayBuffer());
  if (head.length >= 3 && head[0] === 0xff && head[1] === 0xd8) return "jpeg";
  if (head.length >= 8 && head[0] === 0x89 && head[1] === 0x50 && head[2] === 0x4e && head[3] === 0x47) return "png";
  if (head.length >= 6 && head[0] === 0x47 && head[1] === 0x49 && head[2] === 0x46) return "gif";
  if (
    head.length >= 12 &&
    head[0] === 0x52 &&
    head[1] === 0x49 &&
    head[2] === 0x46 &&
    head[3] === 0x46 &&
    head[8] === 0x57 &&
    head[9] === 0x45 &&
    head[10] === 0x42 &&
    head[11] === 0x50
  ) {
    return "webp";
  }
  const text = new TextDecoder("utf-8", { fatal: false }).decode(head).trimStart().toLowerCase();
  if (text.startsWith("<svg") || text.includes("<svg") || byName === "svg") return "svg";
  return byName;
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n}B`;
  if (n < 1024 * 1024) {
    const kb = n / 1024;
    return `${kb < 10 ? kb.toFixed(1) : Math.round(kb)}KB`;
  }
  const mb = n / (1024 * 1024);
  return `${mb < 10 ? mb.toFixed(1) : Math.round(mb)}MB`;
}

export function formatMs(ms: number): string {
  if (ms < 10) return `${ms.toFixed(1)}ms`;
  return `${Math.round(ms)}ms`;
}

export function kindLabel(kind: Kind): string {
  if (kind === "jpeg") return "JPG";
  if (kind === "other") return "—";
  return kind.toUpperCase();
}

export function stemOf(name: string): string {
  const i = name.lastIndexOf(".");
  return i > 0 ? name.slice(0, i) : name;
}
