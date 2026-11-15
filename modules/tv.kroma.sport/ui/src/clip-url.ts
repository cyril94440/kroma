import type { Clip } from './schemas';

/** Clips live in the live module's published folder; the hook only builds URLs for this module's own, so the id segment is swapped (the media ticket belongs to the account). */
export function clipUrl(
  ownFileUrl: (path: string) => string,
  file: Clip['file'],
  ownId: string,
): string {
  const own = ownFileUrl(file.path);
  return own.replace(
    `/module-files/${encodeURIComponent(ownId)}/`,
    `/module-files/${encodeURIComponent(file.module)}/`,
  );
}

const FALLBACK_DELAY_MS = 180_000;

/** A clip shows once this viewer's own picture has passed its end; without a frame time, three minutes after it was cut. */
export function clipVisible(clip: Clip, pdt: number | null, now: number): boolean {
  return pdt === null ? now >= clip.ts + FALLBACK_DELAY_MS : pdt >= clip.fin_ms;
}
