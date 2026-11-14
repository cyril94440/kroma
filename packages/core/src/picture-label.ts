import type { VideoTrack } from '@kromatv/client/media';
import { qualityBadgeForVideo } from './format';
import { type HdrTraits, hdrLabel } from './hdr-label';

const deepColour = (bitDepth: number | null): string | null =>
  bitDepth !== null && bitDepth > 8 ? `${bitDepth}-bit` : null;

export function pictureFormat(video: Pick<VideoTrack, 'codec' | 'bitDepth'> & HdrTraits): string {
  const parts = [video.codec.toUpperCase()];
  if (video.bitDepth) parts.push(`${video.bitDepth}-bit`);
  const hdr = hdrLabel(video);
  if (hdr) parts.push(hdr);
  return parts.join(' ');
}

/** The player's quality badge: the HDR system, else what the catalogue's badge
 * names, and a bit depth above 8, e.g. `Dolby Vision · 10-bit`. Null for an
 * 8-bit SDR picture that nothing else sets apart. */
export function pictureBadge(video: VideoTrack | null | undefined): string | null {
  if (!video) return null;
  const parts = [hdrLabel(video) ?? qualityBadgeForVideo(video), deepColour(video.bitDepth)];
  const named = parts.filter((part): part is string => part !== null);
  return named.length > 0 ? named.join(' · ') : null;
}
