import type { AudioTrack, MediaItem } from '@kromatv/client/media';
import type { PlaybackMode } from '@kromatv/client/playback';
import type { MessageKey, TVars } from '@kromatv/i18n';
import { audioFormatName, spatialOf } from '../audio-label';
import {
  audioTracksOf,
  canDecodeAudioCodec,
  FMP4_COPY_CODECS,
  selectedAudioTrack,
  surroundTargetFor,
} from './audio-support';
import { capabilities, type PlaybackCapabilities } from './capabilities';
import { pictureOutcome } from './dolby-vision';

export interface StreamNotice {
  messageKey: MessageKey;
  messageVars?: TVars;
}

const DOLBY_NAMES = { eac3: 'Dolby Digital+', ac3: 'Dolby Digital' } as const;

/**
 * What the player says about a stream that is not the original file, and null
 * for direct play: the absence of a notice is the signal that the bytes are
 * untouched. A compromise the player cannot name says nothing rather than
 * worrying a viewer with a blank.
 */
export function streamNotice(
  item: MediaItem,
  mode: PlaybackMode,
  caps: PlaybackCapabilities = capabilities(),
  audioIndex?: number,
): StreamNotice | null {
  if (mode === 'direct') return null;
  if (mode === 'remux') return { messageKey: 'player.repackaged' };
  const track = replacedAudioTrack(item, caps, audioIndex);
  return track ? audioNotice(track, caps) : null;
}

function audioNotice(track: AudioTrack, caps: PlaybackCapabilities): StreamNotice {
  const codec = audioFormatName(track);
  const target = surroundTargetFor(track, caps);
  const atmos = spatialOf(track) === 'atmos';
  if (target) {
    return {
      messageKey: atmos ? 'player.atmosSurround' : 'player.audioSurround',
      messageVars: { codec, target: DOLBY_NAMES[target] },
    };
  }
  return { messageKey: atmos ? 'player.atmosAdjusted' : 'player.audioAdjusted', messageVars: { codec } };
}

function replacedAudioTrack(
  item: MediaItem,
  caps: PlaybackCapabilities,
  audioIndex?: number,
): AudioTrack | null {
  const selected = selectedAudioTrack(item, audioIndex);
  const forced = [selected, ...audioTracksOf(item)].find(
    (track) =>
      track &&
      (!track.codec || !canDecodeAudioCodec(track.codec, caps) || !FMP4_COPY_CODECS.has(track.codec)),
  );
  return forced?.codec ? forced : null;
}

/** What the picture loses on this device, whatever the mode: a Dolby Vision
 * stream drawn as its HDR10 base layer, HDR10+ without its dynamic metadata, HDR
 * tone-mapped to an SDR display. Null when the picture arrives as authored, and
 * for the one case that is refused outright, which the direct-play verdict says. */
export function pictureNotice(
  item: MediaItem,
  caps: PlaybackCapabilities = capabilities(),
): StreamNotice | null {
  const outcome = pictureOutcome(item.video, caps);
  if (outcome.kind === 'toneMapped') return { messageKey: 'player.hdrToneMapped' };
  if (outcome.kind !== 'base') return null;
  if (outcome.source === 'hdr10Plus') return { messageKey: 'player.hdr10PlusAsHdr10' };
  if (outcome.as === 'hlg') return { messageKey: 'player.dolbyVisionAsHlg' };
  return { messageKey: outcome.as === 'sdr' ? 'player.dolbyVisionAsSdr' : 'player.dolbyVisionAsHdr10' };
}

/** Every compromise the viewer is owed a word about, picture first. */
export function streamNotices(
  item: MediaItem,
  mode: PlaybackMode,
  caps: PlaybackCapabilities = capabilities(),
  audioIndex?: number,
): StreamNotice[] {
  return [pictureNotice(item, caps), streamNotice(item, mode, caps, audioIndex)].filter(
    (notice): notice is StreamNotice => notice !== null,
  );
}
