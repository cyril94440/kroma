import { describe, expect, it } from 'vitest';
import { observedAudioDelivery, predictAudioDelivery } from './audio-delivery';
import { track } from './directplay.fixture';

const SURROUND_SET = ['aac', 'ac3', 'eac3'];
const truehd = track({ index: 0, codec: 'truehd', channels: 8 });
const dts51 = track({ index: 0, codec: 'dts', channels: 6 });

describe('predictAudioDelivery', () => {
  it('copies a track whose codec the device declared', () => {
    expect(
      predictAudioDelivery(track({ index: 0, codec: 'eac3', channels: 6 }), 'copy', SURROUND_SET),
    ).toEqual({ kind: 'copy' });
  });

  it('copies when the file has no audio track or the probe named no codec', () => {
    expect(predictAudioDelivery(undefined, 'copy', SURROUND_SET)).toEqual({ kind: 'copy' });
    expect(predictAudioDelivery(track({ index: 0, codec: '' }), 'copy', [])).toEqual({
      kind: 'copy',
    });
  });

  it('keeps a surround bed as E-AC-3 5.1 when eac3 is declared', () => {
    expect(predictAudioDelivery(truehd, 'copy', SURROUND_SET)).toEqual({
      kind: 'surround',
      target: 'eac3',
    });
  });

  it('falls to AC-3 5.1 when only ac3 is declared', () => {
    expect(predictAudioDelivery(dts51, 'copy', ['aac', 'ac3'])).toEqual({
      kind: 'surround',
      target: 'ac3',
    });
  });

  it('is stereo AAC when no Dolby codec is declared, or the track has under five channels', () => {
    expect(predictAudioDelivery(dts51, 'copy', ['aac'])).toEqual({
      kind: 'stereo',
      leveled: false,
    });
    const stereoFlac = track({ index: 0, codec: 'flac', channels: 2 });
    expect(predictAudioDelivery(stereoFlac, 'copy', SURROUND_SET)).toEqual({
      kind: 'stereo',
      leveled: false,
    });
  });

  it('is stereo AAC for every explicit aac mode, whatever the device declared', () => {
    expect(predictAudioDelivery(truehd, 'aac', SURROUND_SET)).toEqual({
      kind: 'stereo',
      leveled: false,
    });
    expect(predictAudioDelivery(truehd, 'aac-night', SURROUND_SET)).toEqual({
      kind: 'stereo',
      leveled: true,
    });
  });

  it('does not call an already stereo AAC track converted', () => {
    expect(predictAudioDelivery(track({ index: 0, codec: 'aac', channels: 2 }), 'aac', [])).toEqual(
      { kind: 'copy' },
    );
  });
});

describe('observedAudioDelivery', () => {
  it.each([
    ['http://s/api/items/i/hls/copy/0/0/index.m3u8', { kind: 'copy' }],
    ['http://s/api/items/i/hls/eac3/0/0/index.m3u8', { kind: 'surround', target: 'eac3' }],
    ['http://s/api/items/i/hls/h264-eac3/0/0/index.m3u8', { kind: 'surround', target: 'eac3' }],
    ['http://s/api/items/i/hls/h264-1080-ac3/0/0/index.m3u8', { kind: 'surround', target: 'ac3' }],
    ['http://s/api/items/i/hls/aac/0/0/index.m3u8', { kind: 'stereo', leveled: false }],
    ['http://s/api/items/i/hls/aac-standard/0/0/index.m3u8', { kind: 'stereo', leveled: true }],
  ])('reads %s', (url, expected) => {
    expect(observedAudioDelivery(url)).toEqual(expected);
  });

  it('answers null for a URL that names no mode', () => {
    expect(observedAudioDelivery('http://s/stream')).toBeNull();
    expect(observedAudioDelivery(undefined)).toBeNull();
  });
});
