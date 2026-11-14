import type { AudioTrack, VideoTrack } from '@kromatv/client/media';
import { audioBadgeForTracks, hdrLabel } from '@kromatv/core';

export function qualityBadges(
  video: VideoTrack | null | undefined,
  audioTracks?: ReadonlyArray<AudioTrack> | null,
): string[] {
  return [
    video && (video.width ?? 0) >= 3840 ? '4K' : null,
    hdrLabel(video),
    video?.codec === 'hevc' ? 'H.265' : null,
    audioBadgeForTracks(audioTracks),
  ].filter((badge): badge is string => badge !== null);
}
