import { describe, expect, it } from 'vitest';
import { makeItem, track, UNPROBED } from './directplay.fixture';
import type { PlaybackCapabilities } from './capabilities';
import { observedAudioDelivery } from './audio-delivery';
import { AVPLAY_HLS_CAPS, MSE_CAPS, SAFARI_CAPS, TIZEN_CAPS, WEBOS_CAPS } from './runtime-caps';
import { pictureNotice, streamNotice, streamNotices } from './stream-notice';

describe('streamNotice', () => {
  it('says nothing about a direct play, because silence is how a viewer knows', () => {
    const item = makeItem({ audio: [track({ index: 0 })] });

    expect(streamNotice(item, 'direct', MSE_CAPS)).toBeNull();
  });

  it('calls a remux of a copied track repackaged, without implying the sound changed', () => {
    const item = makeItem({ container: 'mkv', audio: [track({ index: 0, codec: 'ac3' })] });

    expect(streamNotice(item, 'remux', SAFARI_CAPS)).toEqual({ messageKey: 'player.repackaged' });
  });

  it('stays silent on a re-encode it cannot describe', () => {
    expect(streamNotice(UNPROBED, 'transcode', MSE_CAPS)).toBeNull();
  });
});

const hdrItem = (p: Omit<Parameters<typeof makeItem>[0], 'audio'>) =>
  makeItem({ videoCodec: 'hevc', bitDepth: 10, ...p, audio: [] });

describe('pictureNotice', () => {
  const none = (item: ReturnType<typeof makeItem>, caps: PlaybackCapabilities) =>
    pictureNotice(item, caps);

  it('says nothing about a picture that arrives as authored', () => {
    expect(none(hdrItem({ hdrFormat: 'hdr10' }), MSE_CAPS)).toBeNull();
    expect(
      none(hdrItem({ hdrFormat: 'dolbyVision', dolbyVisionProfile: 8 }), WEBOS_CAPS),
    ).toBeNull();
    expect(none(hdrItem({}), MSE_CAPS)).toBeNull();
  });

  it('names the layer Dolby Vision is drawn as on a device without a decoder', () => {
    const profile = (compat: number) =>
      hdrItem({
        hdrFormat: 'dolbyVision',
        dolbyVisionProfile: 8,
        dolbyVisionCompatibility: compat,
      });
    expect(none(profile(1), TIZEN_CAPS)).toEqual({ messageKey: 'player.dolbyVisionAsHdr10' });
    expect(none(profile(4), TIZEN_CAPS)).toEqual({ messageKey: 'player.dolbyVisionAsHlg' });
    expect(none(profile(2), TIZEN_CAPS)).toEqual({ messageKey: 'player.dolbyVisionAsSdr' });
    expect(none(hdrItem({ hdrFormat: 'dolbyVision', dolbyVisionProfile: 7 }), TIZEN_CAPS)).toEqual({
      messageKey: 'player.dolbyVisionAsHdr10',
    });
  });

  it('says HDR10+ plays as HDR10 where its metadata is not applied', () => {
    expect(none(hdrItem({ hdrFormat: 'hdr10Plus' }), WEBOS_CAPS)).toEqual({
      messageKey: 'player.hdr10PlusAsHdr10',
    });
    expect(none(hdrItem({ hdrFormat: 'hdr10Plus' }), TIZEN_CAPS)).toBeNull();
  });

  it('says HDR is tone-mapped on an SDR display', () => {
    const sdr = { ...MSE_CAPS, hdr: { ...MSE_CAPS.hdr, hdr10: false, hlg: false } };
    expect(none(hdrItem({ hdrFormat: 'hdr10' }), sdr)).toEqual({
      messageKey: 'player.hdrToneMapped',
    });
    expect(none(hdrItem({ hdrFormat: 'hlg' }), sdr)).toEqual({
      messageKey: 'player.hdrToneMapped',
    });
  });

  it('leaves a refused stream to the direct-play verdict', () => {
    expect(
      none(
        hdrItem({ hdrFormat: 'dolbyVision', dolbyVisionProfile: 5, dolbyVisionCompatibility: 0 }),
        MSE_CAPS,
      ),
    ).toBeNull();
  });
});

const TRUEHD_ATMOS = track({ index: 0, codec: 'truehd', channels: 8, spatial: 'atmos' });
const DTS_51 = track({ index: 0, codec: 'dts', channels: 6 });

describe('audio the server re-encodes in a remux', () => {
  it('owns up to the surround target a remux earns, not a repackage', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'truehd', channels: 8 })] });
    expect(streamNotice(item, 'remux', SAFARI_CAPS)).toEqual({
      messageKey: 'player.audioSurround',
      messageVars: { codec: 'Dolby TrueHD', target: 'Dolby Digital+' },
    });
  });

  it('owns up to losing Atmos when the bed is kept as 5.1', () => {
    expect(streamNotice(makeItem({ audio: [TRUEHD_ATMOS] }), 'remux', SAFARI_CAPS)).toEqual({
      messageKey: 'player.atmosSurround',
      messageVars: { codec: 'Dolby TrueHD Atmos', target: 'Dolby Digital+' },
    });
  });

  it('names Dolby Digital where that is the only Dolby codec declared', () => {
    const caps = { ...MSE_CAPS, audio: { ...MSE_CAPS.audio, ac3: true } };
    expect(streamNotice(makeItem({ audio: [DTS_51] }), 'remux', caps)?.messageVars).toEqual({
      codec: 'DTS',
      target: 'Dolby Digital',
    });
  });

  it('says a surround track is downmixed where no Dolby codec is declared', () => {
    expect(streamNotice(makeItem({ audio: [DTS_51] }), 'remux', MSE_CAPS)).toEqual({
      messageKey: 'player.audioDownmixed',
      messageVars: { codec: 'DTS' },
    });
    expect(streamNotice(makeItem({ audio: [TRUEHD_ATMOS] }), 'remux', MSE_CAPS)).toEqual({
      messageKey: 'player.atmosAdjusted',
      messageVars: { codec: 'Dolby TrueHD Atmos' },
    });
  });

  it('says stereo AAC for an explicitly AAC master, never 5.1', () => {
    expect(streamNotice(makeItem({ audio: [DTS_51] }), 'transcode', SAFARI_CAPS)).toEqual({
      messageKey: 'player.audioDownmixed',
      messageVars: { codec: 'DTS' },
    });
  });

  it('reports what the server answered with over what it predicted', () => {
    const item = makeItem({ audio: [DTS_51] });
    const aacFallback = observedAudioDelivery('http://s/api/items/i/hls/aac/0/0/index.m3u8');
    expect(streamNotice(item, 'remux', SAFARI_CAPS, { observed: aacFallback })).toEqual({
      messageKey: 'player.audioDownmixed',
      messageVars: { codec: 'DTS' },
    });
  });

  it('says nothing is lost when a track the device decodes is copied', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'eac3', channels: 6 })] });
    expect(streamNotice(item, 'remux', SAFARI_CAPS)).toEqual({ messageKey: 'player.repackaged' });
  });

  it('judges the track being played, not the others', () => {
    const item = makeItem({
      audio: [
        track({ index: 0, codec: 'dts', channels: 6, default: true }),
        track({ index: 1, codec: 'aac', channels: 2 }),
      ],
    });
    expect(streamNotice(item, 'remux', SAFARI_CAPS, { audioIndex: 1 })).toEqual({
      messageKey: 'player.repackaged',
    });
    expect(streamNotice(item, 'remux', SAFARI_CAPS, { audioIndex: 0 })?.messageKey).toBe(
      'player.audioSurround',
    );
  });

  it('owns up to the loudness filter with its own line', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'eac3', channels: 6 })] });
    expect(streamNotice(item, 'direct', AVPLAY_HLS_CAPS, { filter: 'night' })).toEqual({
      messageKey: 'player.audioLeveled',
      messageVars: { codec: 'Dolby Digital+' },
    });
  });
});

describe('a Dolby Vision profile 7 stream', () => {
  const p7 = hdrItem({
    hdrFormat: 'dolbyVision',
    dolbyVisionProfile: 7,
    dolbyVisionCompatibility: 6,
  });

  it('arrives as HDR10 over HLS even on a device that decodes Dolby Vision', () => {
    expect(pictureNotice(p7, WEBOS_CAPS, 'remux')).toEqual({
      messageKey: 'player.dolbyVisionAsHdr10',
    });
  });

  it('stays Dolby Vision when the original file is played', () => {
    expect(pictureNotice(p7, WEBOS_CAPS, 'direct')).toBeNull();
  });

  it('says it is tone-mapped on an SDR display', () => {
    const sdr = { ...WEBOS_CAPS, hdr: { ...WEBOS_CAPS.hdr, hdr10: false, hlg: false } };
    expect(pictureNotice(p7, sdr, 'remux')).toEqual({ messageKey: 'player.hdrToneMapped' });
  });
});

describe('streamNotices', () => {
  it('puts the picture before the audio and omits what is untouched', () => {
    const item = makeItem({
      videoCodec: 'hevc',
      bitDepth: 10,
      hdrFormat: 'dolbyVision',
      dolbyVisionProfile: 8,
      dolbyVisionCompatibility: 1,
      audio: [track({ index: 0, codec: 'truehd', channels: 8 })],
    });
    expect(streamNotices(item, 'remux', TIZEN_CAPS).map((n) => n.messageKey)).toEqual([
      'player.dolbyVisionAsHdr10',
      'player.audioSurround',
    ]);
    expect(streamNotices(item, 'direct', WEBOS_CAPS)).toEqual([]);
  });
});
