import type { AudioTrack } from '@kromatv/client/media';
import { SURROUND_MIN_CHANNELS } from './audio-support';

export type AudioRequest = 'copy' | 'aac' | 'aac-standard' | 'aac-night';

export type AudioDelivery =
  | { kind: 'copy' }
  | { kind: 'surround'; target: 'eac3' | 'ac3' }
  | { kind: 'stereo'; leveled: boolean };

const COPY: AudioDelivery = { kind: 'copy' };

function isStereoAac(track: AudioTrack): boolean {
  return track.codec === 'aac' && (track.channels ?? 0) <= 2;
}

/**
 * What the server writes for the selected track, by the rule it applies itself:
 * in mode `copy` a track whose codec the device declared is copied, and any other
 * track with five channels or more is re-encoded to E-AC-3 5.1 when `eac3` is
 * declared, AC-3 5.1 when `ac3` is, and stereo AAC otherwise. Every `aac*` mode is
 * stereo AAC, the loudness ones with their compressor.
 */
export function predictAudioDelivery(
  track: AudioTrack | undefined,
  request: AudioRequest,
  copyCodecs: readonly string[],
): AudioDelivery {
  if (!track) return COPY;
  if (request !== 'copy') {
    const leveled = request !== 'aac';
    return !leveled && isStereoAac(track) ? COPY : { kind: 'stereo', leveled };
  }
  if (!track.codec || copyCodecs.includes(track.codec)) return COPY;
  if ((track.channels ?? 0) >= SURROUND_MIN_CHANNELS) {
    if (copyCodecs.includes('eac3')) return { kind: 'surround', target: 'eac3' };
    if (copyCodecs.includes('ac3')) return { kind: 'surround', target: 'ac3' };
  }
  return { kind: 'stereo', leveled: false };
}

const MASTER_MODE = /\/hls\/(?:h264(?:-\d+)?-)?(copy|eac3|ac3|aac(?:-standard|-night)?)\//;

/**
 * What the server actually answered with, read from the URL a master request
 * ended on: it redirects to the mode it settled on, so a server that could not
 * encode E-AC-3 is seen to have sent stereo AAC. Null for a URL that names none.
 */
export function observedAudioDelivery(finalUrl: string | null | undefined): AudioDelivery | null {
  const mode = MASTER_MODE.exec(finalUrl ?? '')?.[1];
  switch (mode) {
    case 'copy':
      return COPY;
    case 'eac3':
    case 'ac3':
      return { kind: 'surround', target: mode };
    case 'aac':
      return { kind: 'stereo', leveled: false };
    case 'aac-standard':
    case 'aac-night':
      return { kind: 'stereo', leveled: true };
    default:
      return null;
  }
}
