import type { VideoTrack } from '@kromatv/client/media';
import type { PlaybackCapabilities } from './capabilities';
import { type PictureDelivery, pictureOutcome } from './dolby-vision';

/**
 * The video as this device receives it: the file's own traits where the picture
 * arrives as authored, the base layer it degrades to, or SDR where it is
 * tone-mapped. A badge or a stats row about the player describes this, never the
 * file, which the catalogue describes.
 */
export function deliveredVideo<T extends VideoTrack | null | undefined>(
  video: T,
  caps: PlaybackCapabilities,
  delivery: PictureDelivery,
): T {
  if (!video) return video;
  const outcome = pictureOutcome(video, caps, delivery);
  switch (outcome.kind) {
    case 'base':
      return outcome.as === 'sdr'
        ? { ...video, hdr: false, hdrFormat: null }
        : { ...video, hdr: true, hdrFormat: outcome.as };
    case 'toneMapped':
      return { ...video, hdr: false, hdrFormat: null };
    default:
      return video;
  }
}
