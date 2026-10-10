/** JPEG APP1 Exif IFD1 thumbnail. Reads only the header bytes it is given. */
export function extractJpegThumbnail(buffer: ArrayBuffer): Uint8Array | null {
  const bytes = new Uint8Array(buffer);
  if (bytes.length < 4 || bytes[0] !== 0xff || bytes[1] !== 0xd8) return null;

  let p = 2;
  while (p + 4 < bytes.length) {
    if (bytes[p] !== 0xff) break;
    let marker = bytes[p + 1];
    while (marker === 0xff && p + 2 < bytes.length) {
      p += 1;
      marker = bytes[p + 1];
    }
    if (marker === 0xd9 || marker === 0xda) break;
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) {
      p += 2;
      continue;
    }
    const len = (bytes[p + 2] << 8) | bytes[p + 3];
    if (len < 2 || p + 2 + len > bytes.length) break;
    if (marker === 0xe1) {
      const thumb = thumbFromExif(bytes.subarray(p + 4, p + 2 + len));
      if (thumb) return thumb;
    }
    p += 2 + len;
  }
  return null;
}

function thumbFromExif(app1: Uint8Array): Uint8Array | null {
  if (app1.length < 16) return null;
  if (app1[0] !== 0x45 || app1[1] !== 0x78 || app1[2] !== 0x69 || app1[3] !== 0x66 || app1[4] !== 0 || app1[5] !== 0) {
    return null;
  }
  const tiff = 6;
  const le = app1[tiff] === 0x49 && app1[tiff + 1] === 0x49;
  const be = app1[tiff] === 0x4d && app1[tiff + 1] === 0x4d;
  if (!le && !be) return null;

  const u16 = (o: number) => {
    const a = app1[tiff + o] ?? 0;
    const b = app1[tiff + o + 1] ?? 0;
    return le ? a | (b << 8) : (a << 8) | b;
  };
  const u32 = (o: number) => {
    const a = u16(o);
    const b = u16(o + 2);
    return le ? (a | (b << 16)) >>> 0 : ((a << 16) | b) >>> 0;
  };
  if (tiff + 8 > app1.length || u16(2) !== 42) return null;

  let ifd = u32(4);
  for (let hop = 0; hop < 4 && ifd; hop += 1) {
    const found = readIfdThumb(app1, tiff, ifd, u16, u32);
    if (found) return found;
    if (tiff + ifd + 2 > app1.length) break;
    const count = u16(ifd);
    const nextPos = ifd + 2 + count * 12;
    if (tiff + nextPos + 4 > app1.length) break;
    const next = u32(nextPos);
    if (next === ifd) break;
    ifd = next;
  }
  return null;
}

function readIfdThumb(
  app1: Uint8Array,
  tiff: number,
  ifd: number,
  u16: (o: number) => number,
  u32: (o: number) => number,
): Uint8Array | null {
  if (tiff + ifd + 2 > app1.length) return null;
  const count = u16(ifd);
  let jpegOff: number | null = null;
  let jpegLen: number | null = null;
  for (let i = 0; i < count; i += 1) {
    const e = ifd + 2 + i * 12;
    if (tiff + e + 12 > app1.length) return null;
    const tag = u16(e);
    const value = u32(e + 8);
    if (tag === 0x0201) jpegOff = value;
    if (tag === 0x0202) jpegLen = value;
  }
  if (jpegOff == null || jpegLen == null || jpegLen < 4) return null;
  const start = tiff + jpegOff;
  const end = start + jpegLen;
  if (start < 0 || end > app1.length) return null;
  const slice = app1.subarray(start, end);
  if (slice[0] !== 0xff || slice[1] !== 0xd8) return null;
  return new Uint8Array(slice);
}
