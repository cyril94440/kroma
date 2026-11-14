import type { VideoTrack } from '@kromatv/client/media';
import type { PlaybackCapabilities } from './capabilities';

export type BaseLayer = 'none' | 'hdr10' | 'hlg' | 'sdr';

type DolbyVisionTraits = Pick<VideoTrack, 'dolbyVisionProfile' | 'dolbyVisionCompatibility'>;

const BASE_BY_COMPATIBILITY: Readonly<Record<number, BaseLayer>> = {
  0: 'none',
  1: 'hdr10',
  2: 'sdr',
  4: 'hlg',
  6: 'hdr10',
};

const BASE_BY_PROFILE: Readonly<Record<number, BaseLayer>> = {
  5: 'none',
  7: 'hdr10',
};

/**
 * What a device with no Dolby Vision decoder draws from a Dolby Vision stream:
 * the signalled `bl_signal_compatibility_id` decides, and the profile number only
 * where the probe did not carry it. Profile 7 is a dual layer stream whose base
 * layer is the HDR10 picture; a player without Dolby Vision reads that layer and
 * ignores the enhancement layer. Null where neither names a base layer.
 */
export function dolbyVisionBaseLayer(video: DolbyVisionTraits): BaseLayer | null {
  const id = video.dolbyVisionCompatibility;
  if (id != null && BASE_BY_COMPATIBILITY[id]) return BASE_BY_COMPATIBILITY[id];
  const profile = video.dolbyVisionProfile;
  return profile != null ? (BASE_BY_PROFILE[profile] ?? null) : null;
}

export function decodesDolbyVision(caps: PlaybackCapabilities, profile?: number | null): boolean {
  if (!caps.hdr.dolbyVision) return false;
  const profiles = caps.hdr.dolbyVisionProfiles;
  return profile == null || profiles == null || profiles.includes(profile);
}

export type PictureOutcome =
  | { kind: 'sdr' }
  | { kind: 'native' }
  | { kind: 'base'; source: 'dolbyVision' | 'hdr10Plus'; as: 'hdr10' | 'hlg' | 'sdr' }
  | { kind: 'toneMapped' }
  | { kind: 'refused' };

type HdrVideo = Pick<VideoTrack, 'hdr' | 'hdrFormat'> & DolbyVisionTraits;

function drawsOrToneMaps(drawn: boolean, as: 'hdr10' | 'hlg'): PictureOutcome {
  return drawn ? { kind: 'base', source: 'dolbyVision', as } : { kind: 'toneMapped' };
}

function dolbyVisionOutcome(video: HdrVideo, caps: PlaybackCapabilities): PictureOutcome {
  if (decodesDolbyVision(caps, video.dolbyVisionProfile)) return { kind: 'native' };
  const base = dolbyVisionBaseLayer(video) ?? 'hdr10';
  switch (base) {
    case 'none':
      return { kind: 'refused' };
    case 'sdr':
      return { kind: 'base', source: 'dolbyVision', as: 'sdr' };
    case 'hlg':
      return drawsOrToneMaps(caps.hdr.hlg, 'hlg');
    default:
      return drawsOrToneMaps(caps.hdr.hdr10, 'hdr10');
  }
}

/**
 * What this device puts on screen for a stream's exact HDR variant: the variant
 * itself, the base layer it degrades to, an SDR picture tone-mapped from HDR, or
 * nothing it can draw. Only Dolby Vision with no compatible base layer is refused;
 * every other variant carries a picture a device without its dynamic metadata can
 * still show.
 */
export function pictureOutcome(
  video: HdrVideo | null | undefined,
  caps: PlaybackCapabilities,
): PictureOutcome {
  if (!video || (!video.hdr && !video.hdrFormat)) return { kind: 'sdr' };
  switch (video.hdrFormat) {
    case 'dolbyVision':
      return dolbyVisionOutcome(video, caps);
    case 'hdr10Plus':
      if (caps.hdr.hdr10Plus) return { kind: 'native' };
      return caps.hdr.hdr10
        ? { kind: 'base', source: 'hdr10Plus', as: 'hdr10' }
        : { kind: 'toneMapped' };
    case 'hdr10':
      return caps.hdr.hdr10 ? { kind: 'native' } : { kind: 'toneMapped' };
    case 'hlg':
      return caps.hdr.hlg ? { kind: 'native' } : { kind: 'toneMapped' };
    default:
      return { kind: 'native' };
  }
}
