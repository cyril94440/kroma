import type { Clip } from './schemas';

export function clipUrl(
  fileUrl: (path: string, moduleId?: string) => string,
  file: Clip['file'],
): string {
  return fileUrl(file.path, file.module);
}

const FALLBACK_DELAY_MS = 180_000;

/** A clip shows once this viewer's own picture has passed its end; without a frame time, three minutes after it was cut. */
export function clipVisible(clip: Clip, pdt: number | null, now: number): boolean {
  return pdt === null ? now >= clip.ts + FALLBACK_DELAY_MS : pdt >= clip.fin_ms;
}
