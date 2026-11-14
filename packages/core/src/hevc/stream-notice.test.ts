import { describe, expect, it } from 'vitest';
import { makeItem, track, UNPROBED } from './directplay.fixture';
import type { PlaybackCapabilities } from './capabilities';
import { MSE_CAPS, SAFARI_CAPS, TIZEN_CAPS, WEBOS_CAPS } from './runtime-caps';
import { pictureNotice, streamNotice, streamNotices } from './stream-notice';

describe('streamNotice', () => {
  it('says nothing about a direct play, because silence is how a viewer knows', () => {
    const item = makeItem({ audio: [track({ index: 0 })] });

    expect(streamNotice(item, 'direct', MSE_CAPS)).toBeNull();
  });

  it('calls a remux repackaged, without implying the picture changed', () => {
    const item = makeItem({ container: 'mkv', audio: [track({ index: 0, codec: 'ac3' })] });

    expect(streamNotice(item, 'remux', MSE_CAPS)).toEqual({
      messageKey: 'player.repackaged',
    });
  });

  it('names the codec the browser could not keep', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'truehd', channels: 8 })] });

    expect(streamNotice(item, 'transcode', MSE_CAPS)).toEqual({
      messageKey: 'player.audioAdjusted',
      messageVars: { codec: 'Dolby TrueHD' },
    });
  });

  it('names the first track that forced the re-encode, not the first track', () => {
    const item = makeItem({
      audio: [track({ index: 0, codec: 'aac' }), track({ index: 1, codec: 'dts' })],
    });

    expect(streamNotice(item, 'transcode', MSE_CAPS)?.messageVars).toEqual({ codec: 'DTS' });
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

describe('audio replaced on a transcode', () => {
  it('names the surround target where the device declares Dolby Digital Plus', () => {
    const item = makeItem({ audio: [track({ index: 0, codec: 'truehd', channels: 8 })] });
    expect(streamNotice(item, 'transcode', SAFARI_CAPS)).toEqual({
      messageKey: 'player.audioSurround',
      messageVars: { codec: 'Dolby TrueHD', target: 'Dolby Digital+' },
    });
  });

  it('names Dolby Digital where that is the only Dolby decoder', () => {
    const caps = { ...MSE_CAPS, audio: { ...MSE_CAPS.audio, ac3: true } };
    const item = makeItem({ audio: [track({ index: 0, codec: 'dts', channels: 6 })] });
    expect(streamNotice(item, 'transcode', caps)?.messageVars).toEqual({
      codec: 'DTS',
      target: 'Dolby Digital',
    });
  });

  it('owns up to losing Atmos, to surround or to stereo', () => {
    const atmos = makeItem({
      audio: [track({ index: 0, codec: 'truehd', channels: 8, spatial: 'atmos' })],
    });
    expect(streamNotice(atmos, 'transcode', SAFARI_CAPS)).toEqual({
      messageKey: 'player.atmosSurround',
      messageVars: { codec: 'Dolby TrueHD Atmos', target: 'Dolby Digital+' },
    });
    expect(streamNotice(atmos, 'transcode', MSE_CAPS)).toEqual({
      messageKey: 'player.atmosAdjusted',
      messageVars: { codec: 'Dolby TrueHD Atmos' },
    });
  });

  it('judges the track being played before the others', () => {
    const item = makeItem({
      audio: [
        track({ index: 0, codec: 'dts', channels: 6, default: true }),
        track({ index: 1, codec: 'flac', channels: 2 }),
      ],
    });
    expect(streamNotice(item, 'transcode', MSE_CAPS, 1)?.messageVars).toEqual({ codec: 'FLAC' });
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
    expect(streamNotices(item, 'transcode', TIZEN_CAPS).map((n) => n.messageKey)).toEqual([
      'player.dolbyVisionAsHdr10',
      'player.audioSurround',
    ]);
    expect(streamNotices(item, 'direct', WEBOS_CAPS)).toEqual([]);
  });
});
