import type { MediaItem } from '@kromatv/client/media';
import {
  audioTracksOf,
  canDecodeAudioCodec,
  decodesSelectedAudio,
  masterNeedsAac,
} from './audio-support';
import type { PlaybackCapabilities } from './capabilities';
import { capabilities } from './capabilities';
import { canDirectPlay } from './directplay';
import {
  ANDROID_TV_CAPS,
  APPLE_TV_CAPS,
  AVPLAY_HLS_CAPS,
  MPV_CAPS,
  MSE_CAPS,
  SAFARI_CAPS,
  TIZEN_CAPS,
  WEBOS_CAPS,
} from './runtime-caps';

export type PlayerEngineKind = 'direct' | 'web-mse' | 'tizen-avplay' | 'webos' | 'desktop-mpv';

export interface PlayEnv {
  platform: 'web' | 'tizen' | 'webos' | 'desktop';
  safari: boolean;
  runtimeCaps?: PlaybackCapabilities;
  // Legacy webOS (Chromium < 99) cannot decode HEVC through MSE but plays the
  // master natively; prefer the platform's HLS pipeline over MSE/hls.js there.
  nativeHls?: boolean;
}

export interface EngineDecision {
  kind: PlayerEngineKind;
  aacMaster: boolean;
}

const MP4_CONTAINERS = new Set(['mp4', 'mov', 'm4v', 'm4a', 'isom']);

function plainCompatibleMp4(item: MediaItem, caps: PlaybackCapabilities): boolean {
  const container = (item.container ?? '').toLowerCase();
  if (!MP4_CONTAINERS.has(container)) return false;
  if (!canDirectPlay(item, caps).canDirectPlay) return false;
  const tracks = audioTracksOf(item);
  if (tracks.length !== 1) return false;
  const def = tracks.find((t) => t.default) ?? tracks[0];
  return canDecodeAudioCodec(def?.codec, caps);
}

// Containers Samsung AVPlay demuxes natively from a plain HTTP(S) URL with
// Range support.
const AVPLAY_CONTAINERS = new Set(['mp4', 'mov', 'm4v', 'mkv', 'webm', 'ts', 'm2ts']);

/** Whether Tizen's native AVPlay can play the ORIGINAL file with no server
 * remux: the container, the picture and the track it opens on all have to be
 * ones this set takes. */
export function avplayDirectPlayable(item: MediaItem, audioIndex?: number): boolean {
  const container = (item.container ?? '').toLowerCase();
  if (!AVPLAY_CONTAINERS.has(container)) return false;
  return (
    canDirectPlay(item, TIZEN_CAPS).canDirectPlay &&
    decodesSelectedAudio(item, TIZEN_CAPS, audioIndex)
  );
}

// AVFoundation demuxes the QuickTime/MP4 family and nothing else: no Matroska,
// no WebM, no MPEG-TS, and no framework flag turns one on.
const APPLE_CONTAINERS = new Set(['mp4', 'mov', 'm4v']);

// LG's AV format tables for webOS TV name the QuickTime family, Matroska and
// MPEG-TS; WebM and M2TS are in none of them, so those take the remux.
const LG_CONTAINERS = new Set(['mp4', 'mov', 'm4v', 'mkv', 'ts']);

const NATIVE_CAPS = {
  ios: APPLE_TV_CAPS,
  android: ANDROID_TV_CAPS,
  tizen: TIZEN_CAPS,
  webos: WEBOS_CAPS,
} as const;

const NATIVE_CONTAINERS = {
  ios: APPLE_CONTAINERS,
  android: AVPLAY_CONTAINERS,
  tizen: AVPLAY_CONTAINERS,
  webos: LG_CONTAINERS,
} as const;

/**
 * Whether the native clients (AVPlayer on Apple TV, Media3 on Android TV,
 * capi-media-player on a Samsung set, uMediaServer on an LG set) can open the
 * ORIGINAL file rather than the server's remux. Split by platform because
 * container support is where the players differ.
 */
export function nativeDirectPlayable(
  item: MediaItem,
  os: keyof typeof NATIVE_CONTAINERS,
  audioIndex?: number,
): boolean {
  const container = (item.container ?? '').toLowerCase();
  if (!NATIVE_CONTAINERS[os].has(container)) return false;
  const caps = NATIVE_CAPS[os];
  return canDirectPlay(item, caps).canDirectPlay && decodesSelectedAudio(item, caps, audioIndex);
}

/**
 * Pick the playback engine and master variant for an item in an environment.
 * Engines that keep a direct→master error fallback (mpv, AVPlay, bare `<video>`)
 * may be chosen optimistically.
 */
export function selectEngine(item: MediaItem, env: PlayEnv): EngineDecision {
  if (env.platform === 'desktop') {
    return { kind: 'desktop-mpv', aacMaster: false };
  }
  if (env.platform === 'tizen') {
    return { kind: 'tizen-avplay', aacMaster: false };
  }
  if (env.platform === 'webos') {
    if (plainCompatibleMp4(item, WEBOS_CAPS)) return { kind: 'direct', aacMaster: false };
    // The webOS MSE/hls.js path cannot decode AC3/EAC3; the TV's native pipeline can.
    return { kind: 'webos', aacMaster: !env.nativeHls };
  }
  const caps = env.safari ? SAFARI_CAPS : MSE_CAPS;
  // Prefer the probed runtime over the static table: modern Chromium
  // hardware-decodes HEVC in a bare `<video>` where available.
  if (plainCompatibleMp4(item, env.runtimeCaps ?? caps))
    return { kind: 'direct', aacMaster: false };
  return { kind: 'web-mse', aacMaster: masterNeedsAac(item, env.runtimeCaps ?? caps) };
}

/** The capability table of the plane a title is drawn on: mpv and AVPlay decode
 * on their own, so the runtime the page was probed in says nothing about them. */
export function surfaceCapabilities(surface: string): PlaybackCapabilities {
  if (surface === 'mpv') return MPV_CAPS;
  if (surface === 'avplay') return AVPLAY_HLS_CAPS;
  return capabilities();
}
