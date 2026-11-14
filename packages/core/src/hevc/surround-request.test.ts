import { ItemId } from '@kromatv/client/media';
import { recordingClient } from '@kromatv/client/test';
import { describe, expect, it } from 'vitest';
import { masterNeedsAac } from './audio-support';
import { selectEngine } from './engine';
import {
  type AudioCapabilities,
  copyableAudioCodecs,
  deviceDeclaration,
  type PlaybackCapabilities,
} from './capabilities';
import { makeItem, track } from './directplay.fixture';
import { MSE_CAPS } from './runtime-caps';

const NO_AUDIO: AudioCapabilities = {
  aac: true,
  ac3: false,
  eac3: false,
  dts: false,
  truehd: false,
  flac: false,
  opus: false,
  mp3: false,
  vorbis: false,
};

const device = (audio: Partial<AudioCapabilities>): PlaybackCapabilities => ({
  ...MSE_CAPS,
  audio: { ...NO_AUDIO, ...audio },
});

const EAC3_DEVICE = device({ eac3: true, ac3: true });
const AC3_ONLY_DEVICE = device({ ac3: true });
const DOLBY_AND_LOSSLESS_DEVICE = device({ eac3: true, ac3: true, dts: true, truehd: true });
const NEITHER_DEVICE = device({});

const TRUEHD_71 = makeItem({ audio: [track({ index: 0, codec: 'truehd', channels: 8 })] });
const DTS_51 = makeItem({ audio: [track({ index: 0, codec: 'dts', channels: 6 })] });
const STEREO_AAC = makeItem({ audio: [track({ index: 0, codec: 'aac', channels: 2 })] });

const { client } = recordingClient();

function masterRequest(item: ReturnType<typeof makeItem>, caps: PlaybackCapabilities): string {
  return client.media
    .hlsMasterUrl(ItemId.parse('m1'), masterNeedsAac(item, caps), 0, 0, deviceDeclaration(caps))
    .replace('http://kroma.test/api', '');
}

describe('the master a device is sent for a surround source', () => {
  it('asks for the stream-copy program and never declares TrueHD, so the server re-encodes it to E-AC-3', () => {
    expect(masterRequest(TRUEHD_71, EAC3_DEVICE)).toBe(
      '/items/m1/hls/copy/0/0/index.m3u8?copy=aac%2Cac3%2Ceac3&video=hevc%2Ch264%2Cav1%2Cvp9',
    );
  });

  it('declares only what an fMP4 holds even where the device also decodes DTS and TrueHD', () => {
    expect(masterRequest(DTS_51, DOLBY_AND_LOSSLESS_DEVICE)).toBe(
      '/items/m1/hls/copy/0/0/index.m3u8?copy=aac%2Cac3%2Ceac3&video=hevc%2Ch264%2Cav1%2Cvp9',
    );
  });

  it('declares AC-3 without E-AC-3 for a device that only takes AC-3, so the server picks AC-3', () => {
    expect(masterRequest(DTS_51, AC3_ONLY_DEVICE)).toBe(
      '/items/m1/hls/copy/0/0/index.m3u8?copy=aac%2Cac3&video=hevc%2Ch264%2Cav1%2Cvp9',
    );
  });

  it('asks for the stereo AAC program, with no copy set, where the device decodes neither Dolby codec', () => {
    expect(masterRequest(TRUEHD_71, NEITHER_DEVICE)).toBe(
      '/items/m1/hls/aac/0/0/index.m3u8?video=hevc%2Ch264%2Cav1%2Cvp9',
    );
  });

  it('keeps a stereo AAC source on the stream-copy program', () => {
    expect(masterRequest(STEREO_AAC, NEITHER_DEVICE)).toBe(
      '/items/m1/hls/copy/0/0/index.m3u8?copy=aac&video=hevc%2Ch264%2Cav1%2Cvp9',
    );
  });
});

describe('copyableAudioCodecs', () => {
  it('names what the device decodes and an fMP4 can carry, in the server spelling', () => {
    expect(copyableAudioCodecs(DOLBY_AND_LOSSLESS_DEVICE)).toEqual(['aac', 'ac3', 'eac3']);
  });

  it('names nothing for a device that decodes none of them', () => {
    expect(copyableAudioCodecs(device({ aac: false }))).toEqual([]);
  });
});

describe('the engine decision a browser or a webOS set feeds the master', () => {
  const mkv = (item: ReturnType<typeof makeItem>) => ({ ...item, container: 'mkv' });

  it('sends a web browser decoding E-AC-3 to the copy program for TrueHD 7.1', () => {
    const decision = selectEngine(mkv(TRUEHD_71), {
      platform: 'web',
      safari: false,
      runtimeCaps: EAC3_DEVICE,
    });

    expect(decision).toEqual({ kind: 'web-mse', aacMaster: false });
  });

  it('sends a web browser with no Dolby decoder to the AAC program for DTS 5.1', () => {
    const decision = selectEngine(mkv(DTS_51), {
      platform: 'web',
      safari: false,
      runtimeCaps: NEITHER_DEVICE,
    });

    expect(decision).toEqual({ kind: 'web-mse', aacMaster: true });
  });

  it('sends a webOS set playing the master natively to the copy program, and its MSE path to AAC', () => {
    const native = selectEngine(mkv(TRUEHD_71), {
      platform: 'webos',
      safari: false,
      nativeHls: true,
    });
    const mse = selectEngine(mkv(TRUEHD_71), { platform: 'webos', safari: false });

    expect(native).toEqual({ kind: 'webos', aacMaster: false });
    expect(mse).toEqual({ kind: 'webos', aacMaster: true });
  });

  it('leaves AVPlay and mpv on the copy program, whatever the source', () => {
    expect(selectEngine(TRUEHD_71, { platform: 'tizen', safari: false }).aacMaster).toBe(false);
    expect(selectEngine(TRUEHD_71, { platform: 'desktop', safari: false }).aacMaster).toBe(false);
  });
});
