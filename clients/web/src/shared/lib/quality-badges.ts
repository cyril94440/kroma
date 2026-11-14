import type { AudioTrack, VideoTrack } from '@kromatv/client/media';
import { audioBadgeForTracks, hdrLabel } from '@kromatv/core';

export type QualityTone = string;

export function qualityBadges(
  video: VideoTrack | null | undefined,
  audioTracks?: ReadonlyArray<AudioTrack> | null,
): QualityTone[] {
  return [
    video && (video.width ?? 0) >= 3840 ? '4K' : null,
    hdrLabel(video),
    video?.codec === 'hevc' ? 'H.265' : null,
    audioBadgeForTracks(audioTracks),
  ].filter((badge): badge is QualityTone => badge !== null);
}
