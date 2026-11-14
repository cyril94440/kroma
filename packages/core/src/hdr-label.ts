import type { HdrFormat, VideoTrack } from '@kromatv/client/media';

const HDR_NAMES: Record<HdrFormat, string> = {
  dolbyVision: 'Dolby Vision',
  hdr10Plus: 'HDR10+',
  hdr10: 'HDR10',
  hlg: 'HLG',
};

export type HdrTraits = Pick<VideoTrack, 'hdr' | 'hdrFormat'>;

/** The HDR system a picture carries, by the name a viewer knows it by: `HDR`
 * where the file says HDR without saying which, null for SDR. */
export function hdrLabel(video: HdrTraits | null | undefined): string | null {
  if (video?.hdrFormat) return HDR_NAMES[video.hdrFormat];
  return video?.hdr ? 'HDR' : null;
}
