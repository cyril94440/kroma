import type { AudioTrack } from '@kromatv/client/media';

type AudioTraits = Pick<AudioTrack, 'codec'> & Partial<Pick<AudioTrack, 'profile' | 'spatial'>>;

export type SpatialAudio = NonNullable<AudioTrack['spatial']>;

/** The object-based format a track carries, from the probe's own field and, for
 * a payload that predates it, from the codec profile string. */
export function spatialOf(track: Partial<AudioTrack> | null | undefined): SpatialAudio | null {
  if (!track) return null;
  if (track.spatial) return track.spatial;
  const profile = track.profile ?? '';
  if (/dts:x/i.test(profile)) return 'dtsx';
  if (/atmos/i.test(profile)) return 'atmos';
  return null;
}

function dtsName(profile: string): string {
  if (/dts-hd ma/i.test(profile)) return 'DTS-HD MA';
  if (/dts-hd hra/i.test(profile)) return 'DTS-HD HRA';
  if (/dts-es/i.test(profile)) return 'DTS-ES';
  if (/96\/24/.test(profile)) return 'DTS 96/24';
  return 'DTS';
}

const PLAIN_NAMES: Record<string, string> = {
  aac: 'AAC',
  ac3: 'Dolby Digital',
  flac: 'FLAC',
  opus: 'Opus',
  mp3: 'MP3',
  vorbis: 'Vorbis',
  alac: 'ALAC',
};

/** How a viewer knows the codec: `Dolby TrueHD Atmos`, `Dolby Digital+`,
 * `DTS-HD MA`, `DTS:X`. Proper names, never translated. */
export function audioFormatName(track: AudioTraits): string {
  const codec = track.codec.toLowerCase();
  const spatial = spatialOf(track);
  switch (codec) {
    case 'truehd':
      return spatial === 'atmos' ? 'Dolby TrueHD Atmos' : 'Dolby TrueHD';
    case 'eac3':
      return spatial === 'atmos' ? 'Dolby Digital+ Atmos' : 'Dolby Digital+';
    case 'dts':
      return spatial === 'dtsx' ? 'DTS:X' : dtsName(track.profile ?? '');
    default:
      if (codec.startsWith('pcm')) return 'PCM';
      return PLAIN_NAMES[codec] ?? track.codec.toUpperCase();
  }
}

/** The badge an object-based track earns, or null for a channel-based one. */
export function audioBadgeForTrack(track: Partial<AudioTrack> | null | undefined): string | null {
  const spatial = spatialOf(track);
  if (spatial === 'atmos') return 'Dolby Atmos';
  if (spatial === 'dtsx') return 'DTS:X';
  return null;
}

/** The badge for the best object-based track an item carries. */
export function audioBadgeForTracks(
  tracks: ReadonlyArray<Partial<AudioTrack>> | null | undefined,
): string | null {
  const badges = (tracks ?? []).map(audioBadgeForTrack);
  return badges.find((b) => b === 'Dolby Atmos') ?? badges.find((b) => b !== null) ?? null;
}
