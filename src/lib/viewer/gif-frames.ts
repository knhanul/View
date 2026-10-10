export type DecodedGif = {
  frames: ImageBitmap[];
  delays: number[];
};

type Decoder = {
  close: () => void;
  decode: (opts: { frameIndex: number }) => Promise<{ image: VideoFrame }>;
  tracks: {
    ready: Promise<void>;
    selectedTrack: { frameCount: number } | null;
  };
};

function openDecoder(data: ArrayBuffer): Decoder | null {
  const Ctor = (
    globalThis as unknown as {
      ImageDecoder?: new (init: { data: ArrayBuffer; type: string }) => Decoder;
    }
  ).ImageDecoder;
  if (!Ctor) return null;
  return new Ctor({ data, type: "image/gif" });
}

/** Full GIF decode. Call only after the first frame is already on screen. */
export async function decodeGifFrames(file: File, signal: AbortSignal): Promise<DecodedGif | null> {
  const data = await file.arrayBuffer();
  if (signal.aborted) return null;
  const decoder = openDecoder(data);
  if (!decoder) return null;
  try {
    await decoder.tracks.ready;
    const track = decoder.tracks.selectedTrack;
    if (!track || track.frameCount < 1) return null;
    const frames: ImageBitmap[] = [];
    const delays: number[] = [];
    for (let i = 0; i < track.frameCount; i += 1) {
      if (signal.aborted) {
        frames.forEach((frame) => frame.close());
        return null;
      }
      const { image } = await decoder.decode({ frameIndex: i });
      const delayUs = image.duration;
      const canvas = document.createElement("canvas");
      canvas.width = image.displayWidth;
      canvas.height = image.displayHeight;
      const ctx = canvas.getContext("2d");
      if (!ctx) {
        image.close();
        continue;
      }
      ctx.drawImage(image, 0, 0);
      const delayMs = delayUs == null || delayUs <= 0 ? 100 : Math.max(20, delayUs / 1000);
      delays.push(delayMs);
      image.close();
      frames.push(await createImageBitmap(canvas));
    }
    if (frames.length === 0) return null;
    return { frames, delays };
  } finally {
    decoder.close();
  }
}
