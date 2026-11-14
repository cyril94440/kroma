import type { AudioTrack, MediaItem } from '@kromatv/client/media';
import type { MessageKey, TVars } from '@kromatv/i18n';
import { capabilities, type PlaybackCapabilities } from './capabilities';
import { canDirectPlay, UNNAMED_CODEC } from './directplay';

/** Browsers cannot decode AC3/EAC3/DTS/TrueHD, which plays as video with no
 * sound; `messageKey` is null when audio plays fine. */
export interface AudioSupport {
  canPlay: boolean;
  messageKey: MessageKey | null;
  messageVars?: TVars;
}

export function audioSupport(
  item: MediaItem,
  caps: PlaybackCapabilities = capabilities(),
): AudioSupport {
  const codec = item.audio?.codec;
  if (!codec) return { canPlay: true, messageKey: null };
  if (canDecodeAudioCodec(codec, caps)) return { canPlay: true, messageKey: null };
  return {
    canPlay: false,
    messageKey: 'player.audioUnsupported',
    messageVars: { codec: codec.toUpperCase() },
  };
}

/** Audio codecs ffmpeg can stream-copy into the fMP4 HLS variant, preserving
 * surround; others (DTS/TrueHD/FLAC/Opus) fall back to a stereo-AAC transcode. */
export const FMP4_COPY_CODECS = new Set<string>(['aac', 'ac3', 'eac3']);

/** The track a title plays first: the one asked for, else the file's own default. */
export function selectedAudioTrack(item: MediaItem, audioIndex?: number): AudioTrack | undefined {
  const tracks = audioTracksOf(item);
  return tracks.find((t) => t.index === audioIndex) ?? tracks.find((t) => t.default) ?? tracks[0];
}

/** Whether the track a title plays first is one this device decodes, so direct
 * play does not open a file the viewer would hear nothing from. */
export function decodesSelectedAudio(
  item: MediaItem,
  caps: PlaybackCapabilities = capabilities(),
  audioIndex?: number,
): boolean {
  return canDecodeAudioCodec(selectedAudioTrack(item, audioIndex)?.codec, caps);
}

/** Falls back to a single track for older payloads that only carry the
 * representative `audio`. */
export function audioTracksOf(item: MediaItem): AudioTrack[] {
  if (item.audioTracks?.length) return item.audioTracks;
  return item.audio ? [{ ...item.audio, index: item.audio.index ?? 0 }] : [];
}

/** A codec the table does not list is assumed decodable; one the probe could not
 *  name is not, because there is nothing to assume about. */
export function canDecodeAudioCodec(
  codec: string | undefined,
  caps: PlaybackCapabilities = capabilities(),
): boolean {
  if (!codec) return true;
  if (codec === UNNAMED_CODEC) return false;
  const known = Object.entries(caps.audio).find(([name]) => name === codec);
  return known === undefined || known[1];
}

/** Whether the single-stream HLS master can carry every audio track as an
 * alternate rendition, making language switches in-place. */
export function canSeamlessAudioSwitch(
  item: MediaItem,
  caps: PlaybackCapabilities = capabilities(),
): boolean {
  if (!canDirectPlay(item, caps).canDirectPlay) return false;
  return audioTracksOf(item).length > 1;
}

/** A codec the server's surround target can stand in for: it re-encodes a
 * multichannel track the device cannot take to Dolby Digital Plus or Dolby Digital
 * when the device declares one of them. */
export function surroundTargetFor(
  track: AudioTrack,
  caps: PlaybackCapabilities = capabilities(),
): 'eac3' | 'ac3' | null {
  if ((track.channels ?? 0) <= 2) return null;
  if (caps.audio.eac3) return 'eac3';
  return caps.audio.ac3 ? 'ac3' : null;
}

function copyable(track: AudioTrack, caps: PlaybackCapabilities): boolean {
  return (
    !!track.codec && canDecodeAudioCodec(track.codec, caps) && FMP4_COPY_CODECS.has(track.codec)
  );
}

/**
 * For a master stream, whether audio must be transcoded to stereo AAC (true) or
 * can be handed to the server's copy path (false), which stream-copies what the
 * device decodes and re-encodes the rest to the surround target the device
 * declares. Every track must be either copyable here or have a surround target.
 * Unprobed audio must be AAC: stream-copying a codec the browser cannot decode
 * stalls the load on `HAVE_NOTHING`.
 */
export function masterNeedsAac(
  item: MediaItem,
  caps: PlaybackCapabilities = capabilities(),
): boolean {
  const tracks = audioTracksOf(item);
  if (tracks.length === 0) return true;
  return !tracks.every(
    (t) =>
      !!t.codec &&
      t.codec !== UNNAMED_CODEC &&
      (copyable(t, caps) || surroundTargetFor(t, caps) !== null),
  );
}

/**
 * Track selection must key off this, never the array order: the server can serve
 * `item.audioTracks` in a different order than the player last saw.
 */
export interface AudioTrackId {
  index: number;
  language: string | null;
  title: string | null;
  channels: number | null;
}

export function audioTrackId(t: AudioTrack): AudioTrackId {
  return {
    index: t.index,
    language: t.language ?? null,
    title: t.title ?? null,
    channels: t.channels ?? null,
  };
}

function sameIdentity(t: AudioTrack, want: AudioTrackId): boolean {
  return (
    (t.language ?? null) === want.language &&
    (t.title ?? null) === want.title &&
    (t.channels ?? null) === want.channels
  );
}

function scoreMatch(t: AudioTrack, want: AudioTrackId): number {
  let s = 0;
  const lang = t.language ?? null;
  if (want.language != null && lang != null) {
    if (lang.toLowerCase() === want.language.toLowerCase()) s += 100;
  } else if (want.language == null && lang == null) {
    s += 20;
  }
  if (want.channels != null && t.channels != null && t.channels === want.channels) s += 10;
  const tt = (t.title ?? '').trim().toLowerCase();
  const wt = (want.title ?? '').trim().toLowerCase();
  if (wt && tt && tt === wt) s += 5;
  return s;
}

/**
 * Resolve a wanted audio identity to the audio-relative index to select (what
 * `hls.audioTrack` / a Safari `video.audioTracks` slot expects), tolerating a
 * reordered track list.
 */
export function resolveAudioRelativeIndex(tracks: AudioTrack[], want: AudioTrackId): number {
  if (tracks.length === 0) return 0;

  const exact = tracks.find((t) => t.index === want.index);
  if (exact && sameIdentity(exact, want)) return exact.index;

  let best: AudioTrack | null = null;
  let bestScore = 0;
  for (const t of tracks) {
    const s = scoreMatch(t, want);
    if (s > bestScore) {
      bestScore = s;
      best = t;
    }
  }
  if (best) return best.index;

  const def = tracks.find((t) => t.default);
  return def?.index ?? tracks[0]?.index ?? 0;
}
