import type { AudioTrack, MediaItem } from '@kromatv/client/media';
import type { PlaybackMode } from '@kromatv/client/playback';
import type { MessageKey, TVars } from '@kromatv/i18n';
import { audioFormatName, spatialOf } from '../audio-label';
import { type AudioDelivery, type AudioRequest, predictAudioDelivery } from './audio-delivery';
import { selectedAudioTrack } from './audio-support';
import { capabilities, copyableAudioCodecs, type PlaybackCapabilities } from './capabilities';
import { pictureOutcome } from './dolby-vision';

export interface StreamNotice {
  messageKey: MessageKey;
  messageVars?: TVars;
}

const DOLBY_NAMES = { eac3: 'Dolby Digital+', ac3: 'Dolby Digital' } as const;

export interface DeliveryInput {
  audioIndex?: number;
  observed?: AudioDelivery | null;
  filter?: 'standard' | 'night';
}

/**
 * What the player says about a stream that is not the original file, and null
 * for direct play: the absence of a notice is the signal that the bytes are
 * untouched. The audio line describes what the server delivers for the selected
 * track, as observed where the engine saw the server's answer and as predicted by
 * the server's own rule otherwise.
 */
export function streamNotice(
  item: MediaItem,
  mode: PlaybackMode,
  caps: PlaybackCapabilities = capabilities(),
  input: DeliveryInput = {},
): StreamNotice | null {
  const track = selectedAudioTrack(item, input.audioIndex);
  if (mode === 'direct' && !input.filter) return null;
  if (!track && mode === 'transcode') return null;
  const delivery =
    input.observed ??
    predictAudioDelivery(track, requestedAudio(mode, input.filter), copyableAudioCodecs(caps));
  if (delivery.kind === 'copy') return { messageKey: 'player.repackaged' };
  if (!track?.codec) return null;
  return audioNotice(track, delivery);
}

function requestedAudio(mode: PlaybackMode, filter: DeliveryInput['filter']): AudioRequest {
  if (filter) return `aac-${filter}`;
  return mode === 'transcode' ? 'aac' : 'copy';
}

function audioNotice(
  track: AudioTrack,
  delivery: Exclude<AudioDelivery, { kind: 'copy' }>,
): StreamNotice | null {
  const codec = audioFormatName(track);
  const atmos = spatialOf(track) === 'atmos';
  if (delivery.kind === 'surround') {
    return {
      messageKey: atmos ? 'player.atmosSurround' : 'player.audioSurround',
      messageVars: { codec, target: DOLBY_NAMES[delivery.target] },
    };
  }
  if (delivery.leveled) return { messageKey: 'player.audioLeveled', messageVars: { codec } };
  if (track.codec === 'aac' && (track.channels ?? 0) <= 2)
    return { messageKey: 'player.repackaged' };
  const surround = (track.channels ?? 0) > 2;
  if (atmos) return { messageKey: 'player.atmosAdjusted', messageVars: { codec } };
  return {
    messageKey: surround ? 'player.audioDownmixed' : 'player.audioAdjusted',
    messageVars: { codec },
  };
}

/** What the picture loses on this device, whatever the mode: a Dolby Vision
 * stream drawn as its HDR10 base layer, HDR10+ without its dynamic metadata, HDR
 * tone-mapped to an SDR display. Null when the picture arrives as authored, and
 * for the one case that is refused outright, which the direct-play verdict says. */
export function pictureNotice(
  item: MediaItem,
  caps: PlaybackCapabilities = capabilities(),
  mode: PlaybackMode = 'direct',
): StreamNotice | null {
  const outcome = pictureOutcome(item.video, caps, mode === 'direct' ? 'file' : 'stream');
  if (outcome.kind === 'toneMapped') return { messageKey: 'player.hdrToneMapped' };
  if (outcome.kind !== 'base') return null;
  if (outcome.source === 'hdr10Plus') return { messageKey: 'player.hdr10PlusAsHdr10' };
  if (outcome.as === 'hlg') return { messageKey: 'player.dolbyVisionAsHlg' };
  return {
    messageKey: outcome.as === 'sdr' ? 'player.dolbyVisionAsSdr' : 'player.dolbyVisionAsHdr10',
  };
}

/** Every compromise the viewer is owed a word about, picture first. */
export function streamNotices(
  item: MediaItem,
  mode: PlaybackMode,
  caps: PlaybackCapabilities = capabilities(),
  input: DeliveryInput = {},
): StreamNotice[] {
  return [
    pictureNotice(item, caps, input.filter ? 'remux' : mode),
    streamNotice(item, mode, caps, input),
  ].filter((notice): notice is StreamNotice => notice !== null);
}
