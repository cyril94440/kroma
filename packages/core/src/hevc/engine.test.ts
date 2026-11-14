import { describe, expect, it } from 'vitest';
import { canDirectPlay } from './directplay';
import { makeItem, track, UNPROBED } from './directplay.fixture';
import {
  avplayDirectPlayable,
  nativeDirectPlayable,
  type PlayEnv,
  selectEngine,
  surfaceCapabilities,
} from './engine';
import { AVPLAY_HLS_CAPS, MPV_CAPS, MSE_CAPS, SAFARI_CAPS } from './runtime-caps';

describe('direct play and the track a title opens on', () => {
  const dtsMp4 = makeItem({
    container: 'mp4',
    videoCodec: 'hevc',
    audio: [
      track({ index: 0, codec: 'truehd', channels: 8, default: true }),
      track({ index: 1, codec: 'eac3', channels: 6 }),
    ],
  });

  it('keeps a TrueHD-default file off tvOS, which has no decoder for it, and on Android TV', () => {
    expect(nativeDirectPlayable(dtsMp4, 'ios')).toBe(false);
    expect(nativeDirectPlayable(dtsMp4, 'android')).toBe(true);
  });

  it('opens the same file when the viewer picked a track the set decodes', () => {
    expect(nativeDirectPlayable(dtsMp4, 'ios', 1)).toBe(true);
  });

  it('applies to Tizen and webOS through their own tables', () => {
    const flac = makeItem({
      container: 'mkv',
      videoCodec: 'hevc',
      audio: [track({ index: 0, codec: 'flac', channels: 2 })],
    });
    expect(avplayDirectPlayable(flac)).toBe(true);
    expect(avplayDirectPlayable(dtsMp4, 0)).toBe(true);
    const unnamed = makeItem({
      container: 'mkv',
      videoCodec: 'hevc',
      audio: [track({ index: 0, codec: 'unknown' })],
    });
    expect(avplayDirectPlayable(unnamed)).toBe(false);
    expect(nativeDirectPlayable(unnamed, 'webos')).toBe(false);
  });

  it('refuses Dolby Vision profile 5 on Samsung and on tvOS-less sets alike, but not on LG', () => {
    const p5 = makeItem({
      container: 'mkv',
      videoCodec: 'hevc',
      bitDepth: 10,
      hdrFormat: 'dolbyVision',
      dolbyVisionProfile: 5,
      dolbyVisionCompatibility: 0,
      audio: [track({ index: 0, codec: 'eac3', channels: 6 })],
    });
    expect(avplayDirectPlayable(p5)).toBe(false);
    expect(nativeDirectPlayable(p5, 'tizen')).toBe(false);
    expect(nativeDirectPlayable(p5, 'webos')).toBe(true);
  });

  it('plays a profile 7 file on Samsung as the HDR10 it carries', () => {
    const p7 = makeItem({
      container: 'mkv',
      videoCodec: 'hevc',
      bitDepth: 10,
      hdrFormat: 'dolbyVision',
      dolbyVisionProfile: 7,
      dolbyVisionCompatibility: 6,
      audio: [track({ index: 0, codec: 'eac3', channels: 6 })],
    });
    expect(avplayDirectPlayable(p7)).toBe(true);
  });
});

describe('selectEngine with E-AC-3 surround on the server', () => {
  it('stream-copies a DTS 5.1 master for a browser that decodes E-AC-3 instead of forcing AAC', () => {
    const item = makeItem({
      container: 'mkv',
      videoCodec: 'hevc',
      audio: [track({ index: 0, codec: 'dts', channels: 6 })],
    });
    expect(
      selectEngine(item, { platform: 'web', safari: false, runtimeCaps: SAFARI_CAPS }),
    ).toEqual({
      kind: 'web-mse',
      aacMaster: false,
    });
    expect(selectEngine(item, { platform: 'web', safari: false, runtimeCaps: MSE_CAPS })).toEqual({
      kind: 'web-mse',
      aacMaster: true,
    });
  });
});

describe('surfaceCapabilities', () => {
  it('answers for the plane that decodes, not for the page that was probed', () => {
    expect(surfaceCapabilities('mpv')).toBe(MPV_CAPS);
    expect(surfaceCapabilities('avplay')).toBe(AVPLAY_HLS_CAPS);
  });
});

describe('nativeDirectPlayable', () => {
  it('opens the QuickTime family on both, but Matroska only on Media3', () => {
    const mkv = makeItem({
      container: 'mkv',
      videoCodec: 'hevc',
      audio: [track({ index: 0, codec: 'dts', channels: 6 })],
    });
    expect(nativeDirectPlayable(mkv, 'ios')).toBe(false);
    expect(nativeDirectPlayable(mkv, 'android')).toBe(true);
    const mp4 = makeItem({ container: 'MP4', videoCodec: 'h264', audio: [] });
    expect(nativeDirectPlayable(mp4, 'ios')).toBe(true);
    expect(nativeDirectPlayable(mp4, 'android')).toBe(true);
  });

  it('opens Matroska on a samsung set, as its own player does', () => {
    const mkv = makeItem({ container: 'mkv', videoCodec: 'hevc', audio: [] });
    expect(nativeDirectPlayable(mkv, 'tizen')).toBe(true);
  });

  it('opens Matroska and MPEG-TS on an lg set, and leaves WebM to the remux', () => {
    const mkv = makeItem({ container: 'mkv', videoCodec: 'hevc', audio: [] });
    const ts = makeItem({ container: 'ts', videoCodec: 'h264', audio: [] });
    const webm = makeItem({ container: 'webm', videoCodec: 'h264', audio: [] });
    expect(nativeDirectPlayable(mkv, 'webos')).toBe(true);
    expect(nativeDirectPlayable(ts, 'webos')).toBe(true);
    expect(nativeDirectPlayable(webm, 'webos')).toBe(false);
  });

  it('refuses AV1 on either, whatever the container', () => {
    const av1 = makeItem({ container: 'mp4', videoCodec: 'av1', audio: [] });
    expect(nativeDirectPlayable(av1, 'ios')).toBe(false);
    expect(nativeDirectPlayable(av1, 'android')).toBe(false);
  });

  it('refuses an item whose container was never probed', () => {
    expect(nativeDirectPlayable(UNPROBED, 'ios')).toBe(false);
    expect(nativeDirectPlayable(UNPROBED, 'android')).toBe(false);
    expect(avplayDirectPlayable(UNPROBED)).toBe(false);
  });
});

const WEB_CHROME: PlayEnv = { platform: 'web', safari: false };
const WEB_SAFARI: PlayEnv = { platform: 'web', safari: true };
const TIZEN: PlayEnv = { platform: 'tizen', safari: false };
const WEBOS: PlayEnv = { platform: 'webos', safari: false };
const DESKTOP: PlayEnv = { platform: 'desktop', safari: false };

describe('selectEngine', () => {
  it('routes a plain h264 + aac single-audio mp4 to direct-play on Chrome', () => {
    const item = makeItem({
      container: 'mp4',
      videoCodec: 'h264',
      audio: [track({ index: 0, codec: 'aac', channels: 2, default: true })],
    });
    expect(selectEngine(item, WEB_CHROME)).toEqual({ kind: 'direct', aacMaster: false });
  });

  it('routes an MKV to web-mse (not direct) on Chrome', () => {
    const item = makeItem({
      container: 'mkv',
      videoCodec: 'h264',
      audio: [track({ index: 0, codec: 'aac', channels: 6, default: true })],
    });
    expect(selectEngine(item, WEB_CHROME).kind).toBe('web-mse');
  });

  it('direct-plays an HEVC mp4 on Chrome when the engine caps decode HEVC', () => {
    const item = makeItem({
      container: 'mp4',
      videoCodec: 'hevc',
      audio: [track({ index: 0, codec: 'aac', channels: 2, default: true })],
    });
    expect(selectEngine(item, WEB_CHROME)).toEqual({ kind: 'direct', aacMaster: false });
  });

  it('keeps HEVC off direct-play when the runtime probes NO HEVC decode', () => {
    const item = makeItem({
      container: 'mp4',
      videoCodec: 'hevc',
      audio: [track({ index: 0, codec: 'aac', channels: 2, default: true })],
    });
    const noHevc: PlayEnv = {
      platform: 'web',
      safari: false,
      runtimeCaps: { ...MSE_CAPS, hevc: false, hevc10bit: false },
    };
    expect(selectEngine(item, noHevc)).toEqual({ kind: 'web-mse', aacMaster: false });
  });

  it('avplayDirectPlayable: HEVC+DTS MKV yes, AV1 no, unknown container no', () => {
    const mkv = makeItem({
      container: 'mkv',
      videoCodec: 'hevc',
      audio: [track({ index: 0, codec: 'dts', channels: 6, default: true })],
    });
    expect(avplayDirectPlayable(mkv)).toBe(true);
    const av1 = makeItem({
      container: 'mkv',
      videoCodec: 'av1',
      audio: [track({ index: 0, codec: 'aac', channels: 2, default: true })],
    });
    expect(avplayDirectPlayable(av1)).toBe(false);
    const iso = makeItem({
      container: 'iso',
      videoCodec: 'h264',
      audio: [track({ index: 0, codec: 'aac', channels: 2, default: true })],
    });
    expect(avplayDirectPlayable(iso)).toBe(false);
  });

  it('direct-plays HEVC + aac mp4 on Safari (native HEVC decode)', () => {
    const item = makeItem({
      container: 'mp4',
      videoCodec: 'hevc',
      audio: [track({ index: 0, codec: 'aac', channels: 2, default: true })],
    });
    expect(selectEngine(item, WEB_SAFARI)).toEqual({ kind: 'direct', aacMaster: false });
  });

  it('Safari cannot decode AV1 (no software decoder; HW is M3+ only)', () => {
    const av1 = makeItem({
      container: 'mkv',
      videoCodec: 'av1',
      audio: [track({ index: 0, codec: 'aac', channels: 2, default: true })],
    });
    expect(canDirectPlay(av1, MSE_CAPS).canDirectPlay).toBe(true);
    const verdict = canDirectPlay(av1, SAFARI_CAPS);
    expect(verdict.canDirectPlay).toBe(false);
    expect(verdict.messageKey).toBe('player.av1Unsupported');
  });

  it('HEVC + EAC3 (2 audio): tizen native (no aac), web-mse + aac, webos + aac', () => {
    const item = makeItem({
      container: 'mp4',
      videoCodec: 'hevc',
      audio: [
        track({ index: 0, codec: 'eac3', language: 'en', channels: 6, default: true }),
        track({ index: 1, codec: 'eac3', language: 'fr', channels: 6 }),
      ],
    });
    expect(selectEngine(item, TIZEN)).toEqual({ kind: 'tizen-avplay', aacMaster: false });
    expect(selectEngine(item, WEB_CHROME)).toEqual({ kind: 'web-mse', aacMaster: true });
    expect(selectEngine(item, WEBOS)).toEqual({ kind: 'webos', aacMaster: true });
    expect(selectEngine(item, DESKTOP)).toEqual({ kind: 'desktop-mpv', aacMaster: false });
  });

  it('legacy webOS (nativeHls): the master is handed to the native pipeline, stream-copied', () => {
    const item = makeItem({
      container: 'mkv',
      videoCodec: 'hevc',
      audio: [
        track({ index: 0, codec: 'eac3', language: 'en', channels: 6, default: true }),
        track({ index: 1, codec: 'eac3', language: 'fr', channels: 6 }),
      ],
    });
    // Chromium < 99 can't decode HEVC via MSE, but the TV pipeline decodes the
    // HLS master (surround included) itself, so no AAC transcode.
    expect(selectEngine(item, { ...WEBOS, nativeHls: true })).toEqual({
      kind: 'webos',
      aacMaster: false,
    });
  });

  it('always routes the Steam Deck to desktop-mpv (even a plain mp4)', () => {
    const item = makeItem({
      container: 'mp4',
      videoCodec: 'h264',
      audio: [track({ index: 0, codec: 'aac', channels: 2, default: true })],
    });
    expect(selectEngine(item, DESKTOP)).toEqual({ kind: 'desktop-mpv', aacMaster: false });
  });

  it('direct-plays a single-audio mp4 even when no track is flagged default', () => {
    const item = makeItem({
      container: 'mp4',
      videoCodec: 'h264',
      audio: [track({ index: 0, codec: 'aac', channels: 2 })],
    });
    expect(selectEngine(item, WEB_CHROME)).toEqual({ kind: 'direct', aacMaster: false });
  });

  it('routes an item with no probed container to the MSE master', () => {
    expect(selectEngine(UNPROBED, WEB_CHROME)).toEqual({ kind: 'web-mse', aacMaster: true });
  });

  it('direct-plays a plain mp4 on webOS, but always reports tizen-avplay on Tizen', () => {
    const item = makeItem({
      container: 'mp4',
      videoCodec: 'h264',
      audio: [track({ index: 0, codec: 'aac', channels: 2, default: true })],
    });
    expect(selectEngine(item, WEBOS)).toEqual({ kind: 'direct', aacMaster: false });
    expect(selectEngine(item, TIZEN)).toEqual({ kind: 'tizen-avplay', aacMaster: false });
  });
});
