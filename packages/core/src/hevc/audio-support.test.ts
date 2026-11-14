import type { AudioTrack, MediaItem } from '@kromatv/client/media';
import { describe, expect, it } from 'vitest';
import {
  audioSupport,
  audioTrackId,
  audioTracksOf,
  canDecodeAudioCodec,
  canSeamlessAudioSwitch,
  decodesSelectedAudio,
  masterNeedsAac,
  resolveAudioRelativeIndex,
  selectedAudioTrack,
  surroundTargetFor,
} from './audio-support';
import { EN_51, FR_51, FR_COMMENTARY, makeItem, track, UNPROBED } from './directplay.fixture';
import { MSE_CAPS, NATIVE_TV_CAPS, SAFARI_CAPS } from './runtime-caps';

describe('audioSupport', () => {
  it('says nothing about an item whose audio was never probed', () => {
    expect(audioSupport(makeItem({ audio: [] }), MSE_CAPS)).toEqual({
      canPlay: true,
      messageKey: null,
    });
  });

  it('does not block on a codec the table has no entry for', () => {
    const pcm = makeItem({ audio: [track({ index: 0, codec: 'pcm_s16le' })] });
    expect(audioSupport(pcm, MSE_CAPS)).toEqual({ canPlay: true, messageKey: null });
  });

  it('names the codec that would otherwise play as silence', () => {
    const eac3 = makeItem({ audio: [track({ index: 0, codec: 'eac3' })] });
    expect(audioSupport(eac3, MSE_CAPS)).toEqual({
      canPlay: false,
      messageKey: 'player.audioUnsupported',
      messageVars: { codec: 'EAC3' },
    });
    expect(audioSupport(eac3, SAFARI_CAPS)).toEqual({ canPlay: true, messageKey: null });
  });
});

describe('canDecodeAudioCodec', () => {
  it('assumes an absent or unlisted codec decodes', () => {
    expect(canDecodeAudioCodec(undefined, MSE_CAPS)).toBe(true);
    expect(canDecodeAudioCodec('pcm_s16le', MSE_CAPS)).toBe(true);
  });

  it('answers from the engine table for a listed codec', () => {
    expect(canDecodeAudioCodec('eac3', MSE_CAPS)).toBe(false);
    expect(canDecodeAudioCodec('eac3', SAFARI_CAPS)).toBe(true);
  });

  it('refuses a codec the probe itself could not name', () => {
    expect(canDecodeAudioCodec('unknown', MSE_CAPS)).toBe(false);
    expect(canDecodeAudioCodec('unknown', SAFARI_CAPS)).toBe(false);
  });
});

describe('audioTracksOf', () => {
  it('numbers an older payload that carries only the representative track', () => {
    const legacy = {
      container: 'mp4',
      video: { codec: 'h264' },
      audio: { codec: 'aac', channels: 2 },
      audioTracks: [],
    } as unknown as MediaItem;
    expect(audioTracksOf(legacy)).toEqual([{ codec: 'aac', channels: 2, index: 0 }]);
  });

  it('is empty for an item with no audio at all', () => {
    expect(audioTracksOf(UNPROBED)).toEqual([]);
  });
});

describe('canSeamlessAudioSwitch', () => {
  it('is off when the video itself cannot direct-play', () => {
    const av1 = makeItem({ videoCodec: 'av1', audio: [EN_51(0), FR_51(1)] });
    expect(canSeamlessAudioSwitch(av1, SAFARI_CAPS)).toBe(false);
  });

  it('needs a second track to be worth anything', () => {
    expect(canSeamlessAudioSwitch(makeItem({ audio: [EN_51(0)] }), MSE_CAPS)).toBe(false);
    expect(canSeamlessAudioSwitch(makeItem({ audio: [EN_51(0), FR_51(1)] }), MSE_CAPS)).toBe(true);
  });
});

describe('resolveAudioRelativeIndex', () => {
  it('resolves the commentary id even when the list is reordered', () => {
    const reordered: AudioTrack[] = [FR_51(1), EN_51(0), FR_COMMENTARY(2)];
    const want = audioTrackId(FR_COMMENTARY(2));
    expect(resolveAudioRelativeIndex(reordered, want)).toBe(2);
  });

  it('disambiguates same-language tracks by channel count', () => {
    const tracks: AudioTrack[] = [
      track({ index: 0, language: 'fr', channels: 6 }),
      track({ index: 1, language: 'fr', channels: 2 }),
    ];
    const want = { index: 5, language: 'fr', title: null, channels: 2 };
    expect(resolveAudioRelativeIndex(tracks, want)).toBe(1);
  });

  it('matches by language + channels when the wanted title is missing', () => {
    const tracks: AudioTrack[] = [
      track({ index: 3, language: 'en', channels: 6 }),
      track({ index: 4, language: 'fr', channels: 6 }),
    ];
    const want = { index: 9, language: 'en', title: null, channels: 6 };
    expect(resolveAudioRelativeIndex(tracks, want)).toBe(3);
  });

  it('returns the matching index when index and identity agree', () => {
    const tracks: AudioTrack[] = [EN_51(0), FR_51(1)];
    expect(resolveAudioRelativeIndex(tracks, audioTrackId(FR_51(1)))).toBe(1);
  });

  it('ignores a disagreeing index and resolves by identity', () => {
    const tracks: AudioTrack[] = [EN_51(0), FR_51(1)];
    const want = { index: 0, language: 'fr', title: null, channels: 6 };
    expect(resolveAudioRelativeIndex(tracks, want)).toBe(1);
  });

  it('falls back to the default track when nothing matches', () => {
    const tracks: AudioTrack[] = [
      track({ index: 0, language: 'en', channels: 6 }),
      track({ index: 1, language: 'de', channels: 6, default: true }),
    ];
    const want = { index: 9, language: 'ja', title: null, channels: 2 };
    expect(resolveAudioRelativeIndex(tracks, want)).toBe(1);
  });

  it('falls back to the first track when nothing matches and there is no default', () => {
    const tracks: AudioTrack[] = [
      track({ index: 0, language: 'en', channels: 6 }),
      track({ index: 1, language: 'de', channels: 6 }),
    ];
    const want = { index: 9, language: 'ja', title: null, channels: 2 };
    expect(resolveAudioRelativeIndex(tracks, want)).toBe(0);
  });

  it('returns 0 for an empty track list', () => {
    expect(
      resolveAudioRelativeIndex([], { index: 0, language: null, title: null, channels: null }),
    ).toBe(0);
  });

  it('reads an untagged track as an all-null identity', () => {
    const untagged = { index: 3, codec: 'aac', default: false } as unknown as AudioTrack;
    expect(audioTrackId(untagged)).toEqual({
      index: 3,
      language: null,
      title: null,
      channels: null,
    });
  });

  it('matches an untagged track on its index alone', () => {
    const tracks: AudioTrack[] = [track({ index: 0 }), track({ index: 1 })];
    const want = { index: 1, language: null, title: null, channels: null };
    expect(resolveAudioRelativeIndex(tracks, want)).toBe(1);
  });

  it('prefers the untagged track when the wanted identity is untagged too', () => {
    const tracks: AudioTrack[] = [
      track({ index: 0, language: 'en', channels: 6 }),
      track({ index: 1 }),
    ];
    const want = { index: 9, language: null, title: null, channels: null };
    expect(resolveAudioRelativeIndex(tracks, want)).toBe(1);
  });

  it('lets the title break a tie between two same-language, same-layout tracks', () => {
    const tracks: AudioTrack[] = [
      track({ index: 4, language: 'fr', title: 'Version française', channels: 2 }),
      track({ index: 5, language: 'fr', title: 'Commentaire', channels: 2 }),
    ];
    const want = { index: 9, language: 'fr', title: '  commentaire ', channels: 2 };
    expect(resolveAudioRelativeIndex(tracks, want)).toBe(5);
  });
});

describe('the track a title plays first', () => {
  const item = makeItem({
    audio: [
      track({ index: 0, codec: 'dts', channels: 6 }),
      track({ index: 1, codec: 'eac3', channels: 6, default: true }),
      track({ index: 2, codec: 'aac', channels: 2 }),
    ],
  });

  it('is the one asked for, else the default, else the first', () => {
    expect(selectedAudioTrack(item, 2)?.index).toBe(2);
    expect(selectedAudioTrack(item)?.index).toBe(1);
    expect(
      selectedAudioTrack(makeItem({ audio: [track({ index: 4, codec: 'aac' })] }))?.index,
    ).toBe(4);
    expect(selectedAudioTrack(makeItem({ audio: [] }))).toBeUndefined();
  });

  it('is judged on that track alone, so a title is not played silent', () => {
    expect(decodesSelectedAudio(item, MSE_CAPS, 0)).toBe(false);
    expect(decodesSelectedAudio(item, MSE_CAPS, 2)).toBe(true);
    expect(decodesSelectedAudio(item, MSE_CAPS)).toBe(false);
    expect(decodesSelectedAudio(item, SAFARI_CAPS)).toBe(true);
    expect(decodesSelectedAudio(makeItem({ audio: [] }), MSE_CAPS)).toBe(true);
  });
});

describe('surroundTargetFor', () => {
  it('prefers Dolby Digital Plus, falls back to Dolby Digital, and never targets stereo', () => {
    const surround = track({ index: 0, codec: 'truehd', channels: 8 });
    expect(surroundTargetFor(surround, SAFARI_CAPS)).toBe('eac3');
    expect(
      surroundTargetFor(surround, { ...MSE_CAPS, audio: { ...MSE_CAPS.audio, ac3: true } }),
    ).toBe('ac3');
    expect(surroundTargetFor(surround, MSE_CAPS)).toBeNull();
    expect(
      surroundTargetFor(track({ index: 0, codec: 'flac', channels: 2 }), SAFARI_CAPS),
    ).toBeNull();
  });
});

describe('masterNeedsAac', () => {
  it('keeps an all-aac master as stream-copy on every engine', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'aac', channels: 2 })] });
    expect(masterNeedsAac(item, MSE_CAPS)).toBe(false);
    expect(masterNeedsAac(item, SAFARI_CAPS)).toBe(false);
    expect(masterNeedsAac(item, NATIVE_TV_CAPS)).toBe(false);
  });

  it('forces AAC for EAC3 under MSE but stream-copies on Safari / native TV', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'eac3', channels: 6 })] });
    expect(masterNeedsAac(item, MSE_CAPS)).toBe(true);
    expect(masterNeedsAac(item, SAFARI_CAPS)).toBe(false);
    expect(masterNeedsAac(item, NATIVE_TV_CAPS)).toBe(false);
  });

  it('forces AAC for DTS where no Dolby decoder can take the surround transcode', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'dts', channels: 6 })] });
    expect(masterNeedsAac(item, MSE_CAPS)).toBe(true);
  });

  it('leaves surround DTS and TrueHD to the server where the device decodes E-AC-3 or AC-3', () => {
    for (const codec of ['dts', 'truehd']) {
      const item = makeItem({ audio: [track({ index: 0, codec, channels: 6 })] });
      expect(masterNeedsAac(item, SAFARI_CAPS)).toBe(false);
      expect(masterNeedsAac(item, NATIVE_TV_CAPS)).toBe(false);
      expect(masterNeedsAac(item, { ...MSE_CAPS, audio: { ...MSE_CAPS.audio, ac3: true } })).toBe(
        false,
      );
    }
  });

  it('keeps a stereo track the device cannot copy on AAC, since there is no stereo surround target', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'flac', channels: 2 })] });
    expect(masterNeedsAac(item, SAFARI_CAPS)).toBe(true);
  });

  it('forces AAC for a codec the probe could not name, whatever the device decodes', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'unknown', channels: 6 })] });
    expect(masterNeedsAac(item, SAFARI_CAPS)).toBe(true);
  });

  it('forces AAC when any track is undecodable under the engine', () => {
    const item = makeItem({
      audio: [
        track({ index: 0, codec: 'aac', channels: 2 }),
        track({ index: 1, codec: 'eac3', channels: 6 }),
      ],
    });
    expect(masterNeedsAac(item, MSE_CAPS)).toBe(true);
    expect(masterNeedsAac(item, SAFARI_CAPS)).toBe(false);
  });

  it('forces AAC when the audio is unknown (unprobed file, no track list)', () => {
    const item = makeItem({ audio: [] });
    expect(masterNeedsAac(item, MSE_CAPS)).toBe(true);
    expect(masterNeedsAac(item, SAFARI_CAPS)).toBe(true);
    expect(masterNeedsAac(item, NATIVE_TV_CAPS)).toBe(true);
  });
});
