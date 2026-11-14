// Codec/capability probing: what video + audio the *current runtime* can decode.

import { Platform } from 'react-native';
import { isTizenRuntime, isWebOsRuntime } from '../platform';
import { ANDROID_TV_CAPS, APPLE_TV_CAPS, TIZEN_CAPS, WEBOS_CAPS } from './runtime-caps';

// `hvc1`/`hev1` are the two HEVC sample entry fourCCs; both are probed because
// platforms disagree on which they accept.
const PROBE = {
  hevcMain: 'video/mp4; codecs="hvc1.1.6.L93.B0"',
  hevcMainAlt: 'video/mp4; codecs="hev1.1.6.L93.B0"',
  hevcMain10: 'video/mp4; codecs="hvc1.2.4.L120.B0"',
  h264High: 'video/mp4; codecs="avc1.640028"',
  // The two profiles a 10-bit H.264 encode lands in: High 10 (`6E`) for 4:2:0
  // and High 4:4:4 Predictive (`F4`), which is what x264 writes at 4:4:4. Both
  // are probed because a runtime that draws one still shows the other black.
  h264High10: 'video/mp4; codecs="avc1.6E0028"',
  h264High444: 'video/mp4; codecs="avc1.F40028"',
  av1Main: 'video/mp4; codecs="av01.0.05M.08"',
  vp9: 'video/webm; codecs="vp09.00.10.08"',
} as const;

const DOLBY_VISION_PROBES: ReadonlyArray<readonly [number, readonly string[]]> = [
  [5, ['video/mp4; codecs="dvh1.05.06"', 'video/mp4; codecs="dvhe.05.06"']],
  [7, ['video/mp4; codecs="dvhe.07.06"', 'video/mp4; codecs="dvh1.07.06"']],
  [8, ['video/mp4; codecs="dvh1.08.06"', 'video/mp4; codecs="dvhe.08.06"']],
  [10, ['video/mp4; codecs="dav1.10.06"']],
];

// AC3/EAC3/DTS/TrueHD are not decodable by Chrome/Firefox (licensing); only
// Safari (macOS) and TVs handle AC3/EAC3.
const AUDIO_PROBE = {
  aac: 'audio/mp4; codecs="mp4a.40.2"',
  ac3: 'audio/mp4; codecs="ac-3"',
  eac3: 'audio/mp4; codecs="ec-3"',
  flac: 'audio/ogg; codecs="flac"',
  opus: 'audio/webm; codecs="opus"',
  mp3: 'audio/mpeg',
  vorbis: 'audio/webm; codecs="vorbis"',
} as const;

export interface AudioCapabilities {
  aac: boolean;
  ac3: boolean;
  eac3: boolean;
  dts: boolean;
  truehd: boolean;
  flac: boolean;
  opus: boolean;
  mp3: boolean;
  vorbis: boolean;
}

export interface FrameSize {
  width: number;
  height: number;
}

/** The largest frame each video decoder accepts, keyed by the codec name an
 * item carries (ffprobe's: `hevc`, `h264`, `av1`, `vp9`). A codec with no entry
 * is not gated on size. */
export type DecoderFrameLimits = Readonly<Record<string, FrameSize>>;

/** Which HDR systems this device renders, one answer each: a set that draws
 * HDR10 but not Dolby Vision is not the same set as one that draws both, which is
 * the whole reason a single `hdr` flag could not be matched against. */
export interface HdrCapabilities {
  hdr10: boolean;
  hdr10Plus: boolean;
  dolbyVision: boolean;
  hlg: boolean;
  /** The Dolby Vision profiles the decoder takes, where the runtime says which.
   * Absent means `dolbyVision` holds for every profile. */
  dolbyVisionProfiles?: readonly number[];
}

export interface PlaybackCapabilities {
  hevc: boolean;
  hevc10bit: boolean;
  h264: boolean;
  h26410bit: boolean;
  av1: boolean;
  vp9: boolean;
  hdr: HdrCapabilities;
  /** True where `hdr` is a statement about this device and its display, so it
   * can be declared to the server; absent where it is a guess. */
  hdrDeclared?: boolean;
  /** Absent where the runtime will not say, which gates nothing. */
  frameLimits?: DecoderFrameLimits;
  audio: AudioCapabilities;
  source: 'mediaSource' | 'videoElement' | 'platform-tv' | 'unknown';
}

function supportsType(type: string): boolean {
  const MS = (globalThis as { MediaSource?: { isTypeSupported(t: string): boolean } }).MediaSource;
  if (MS && typeof MS.isTypeSupported === 'function' && MS.isTypeSupported(type)) return true;

  if (typeof document !== 'undefined') {
    const v = document.createElement('video');
    const r = v.canPlayType(type);
    if (r === 'probably' || r === 'maybe') return true;
  }
  return false;
}

function detectHdr(): boolean {
  if (typeof globalThis.matchMedia !== 'function') return false;
  return (
    globalThis.matchMedia('(dynamic-range: high)').matches ||
    globalThis.matchMedia('(video-dynamic-range: high)').matches
  );
}

// What the panel reports covers HDR10 and HLG, which need nothing but a wide
// display. Dolby Vision needs a decoder, so it is probed per profile like a codec.
// HDR10+ is `false` everywhere in a browser: none of them apply its dynamic
// metadata, and the HDR10 base layer is what actually gets drawn.
function detectHdrFormats(): HdrCapabilities {
  const panel = detectHdr();
  const dolbyVisionProfiles = DOLBY_VISION_PROBES.filter(([, types]) =>
    types.some(supportsType),
  ).map(([profile]) => profile);
  return {
    hdr10: panel,
    hdr10Plus: false,
    dolbyVision: dolbyVisionProfiles.length > 0,
    hlg: panel,
    ...(dolbyVisionProfiles.length > 0 ? { dolbyVisionProfiles } : {}),
  };
}

// The native clients decode through the platform, not a `<video>` element:
// React Native has neither `canPlayType` nor `MediaSource`, so every probe
// below answers "no". `navigator.product`, not `typeof document`: a server-side
// render has no document either and must fall through to the browser answer it
// will hydrate into.
function isReactNative(): boolean {
  return (
    typeof navigator !== 'undefined' &&
    (navigator as { product?: string }).product === 'ReactNative'
  );
}

function platformTable(isTizen: boolean, isWebOS: boolean): PlaybackCapabilities {
  if (isTizen) return tizenCaps();
  if (isWebOS) return WEBOS_CAPS;
  return Platform.OS === 'ios' ? APPLE_TV_CAPS : ANDROID_TV_CAPS;
}

function tizenCaps(): PlaybackCapabilities {
  const avinfo = (globalThis as { webapis?: { avinfo?: { isHdrTvSupport?: () => boolean } } })
    .webapis?.avinfo;
  if (typeof avinfo?.isHdrTvSupport !== 'function') return TIZEN_CAPS;
  try {
    return avinfo.isHdrTvSupport()
      ? TIZEN_CAPS
      : { ...TIZEN_CAPS, hdr: { ...TIZEN_CAPS.hdr, hdr10: false, hdr10Plus: false, hlg: false } };
  } catch {
    return TIZEN_CAPS;
  }
}

export function detectCapabilities(frameLimits?: DecoderFrameLimits): PlaybackCapabilities {
  const ua = typeof navigator !== 'undefined' ? navigator.userAgent : '';
  const isTizen = isTizenRuntime(ua);
  const isWebOS = isWebOsRuntime(ua);
  const isNative = isReactNative();
  if (isTizen || isWebOS || isNative) {
    return { ...platformTable(isTizen, isWebOS), frameLimits };
  }

  const hevc = supportsType(PROBE.hevcMain) || supportsType(PROBE.hevcMainAlt);
  const usingMse = !!(globalThis as { MediaSource?: unknown }).MediaSource;
  const audio: AudioCapabilities = {
    aac: supportsType(AUDIO_PROBE.aac),
    ac3: supportsType(AUDIO_PROBE.ac3),
    eac3: supportsType(AUDIO_PROBE.eac3),
    dts: false, // never decodable in a browser
    truehd: false,
    flac: supportsType(AUDIO_PROBE.flac) || supportsType('audio/mp4; codecs="fLaC"'),
    opus: supportsType(AUDIO_PROBE.opus),
    mp3: supportsType(AUDIO_PROBE.mp3),
    vorbis: supportsType(AUDIO_PROBE.vorbis),
  };
  const hdr = detectHdrFormats();
  return {
    hevc,
    hevc10bit: supportsType(PROBE.hevcMain10),
    h264: supportsType(PROBE.h264High),
    h26410bit: supportsType(PROBE.h264High10) && supportsType(PROBE.h264High444),
    av1: supportsType(PROBE.av1Main),
    vp9: supportsType(PROBE.vp9),
    hdr,
    hdrDeclared: hdr.hdr10,
    frameLimits,
    audio,
    source: usingMse ? 'mediaSource' : 'videoElement',
  };
}

// Runtime support is a static platform capability, rather than a mutable
// session registry. Each JavaScript realm probes once at module initialization.
const DETECTED = detectCapabilities();
const SNAPSHOT: PlaybackCapabilities = Object.freeze({
  ...DETECTED,
  audio: Object.freeze(DETECTED.audio),
  hdr: Object.freeze(DETECTED.hdr),
});
export function capabilities(): PlaybackCapabilities {
  return SNAPSHOT;
}

/**
 * The audio codecs this device can decode, named the way the server names them
 * (ffprobe's `codec_name`, which is what an item's audio tracks carry).
 *
 * A device that never declares anything is served a copy of everything and plays
 * whatever it cannot decode as silence, which is why a player sends
 * {@link copyableAudioCodecs}, drawn from this list, as `copyCodecs`.
 */
export function decodableAudioCodecs(caps: PlaybackCapabilities = capabilities()): string[] {
  return Object.entries(caps.audio)
    .filter(([, decodable]) => decodable)
    .map(([codec]) => codec);
}

/** Audio codecs ffmpeg can stream-copy into the fMP4 HLS variant, preserving
 * surround; others (DTS/TrueHD/FLAC/Opus) fall back to a stereo-AAC transcode. */
export const FMP4_COPY_CODECS = new Set<string>(['aac', 'ac3', 'eac3']);

/**
 * What a master or a download may stream-copy: the codecs this device decodes
 * that an fMP4 can also carry. A master declares this rather than
 * {@link decodableAudioCodecs}, because the server copies whatever the declaration
 * names, so a device that decodes TrueHD would be served TrueHD in a container
 * that cannot hold it instead of the 5.1 E-AC-3 its `eac3` earns.
 */
export function copyableAudioCodecs(caps: PlaybackCapabilities = capabilities()): string[] {
  return decodableAudioCodecs(caps).filter((codec) => FMP4_COPY_CODECS.has(codec));
}

/**
 * What this device declares it can draw, named the way the server names it
 * (ffprobe's `codec_name`, which is what an item's video stream carries) plus one
 * name that is not a codec: `h26410bit`, for H.264 above 8 bits.
 *
 * This is what a player sends as `videoCodecs`: the server stream-copies video by
 * default and only re-encodes a source this list leaves out, so a device that
 * declares nothing is served a copy and shows a black picture for anything it
 * cannot decode. A codec name carries no bit depth, which is why the deep
 * H.264 case has a name of its own; HEVC has none, so `hevc` is declared only
 * where 10-bit HEVC decodes too, one name covering both depths.
 */
export function decodableVideoCodecs(caps: PlaybackCapabilities = capabilities()): string[] {
  const declared: PlaybackCapabilities = { ...caps, hevc: caps.hevc && caps.hevc10bit };
  return (['hevc', 'h264', 'h26410bit', 'av1', 'vp9'] as const).filter((codec) => declared[codec]);
}

/**
 * The HDR systems this device displays, in the words the server's `hdr` query
 * token takes (`hdr10`, `hdr10plus`, `hlg`, `dv`). Undefined where the table is a
 * guess rather than a statement about the device, which leaves the server to behave
 * as it did before the token existed; an empty list is a declared SDR display.
 */
export function hdrDeclaration(caps: PlaybackCapabilities = capabilities()): string[] | undefined {
  if (!caps.hdrDeclared) return undefined;
  const { hdr10, hdr10Plus, hlg, dolbyVision } = caps.hdr;
  return [
    hdr10 ? 'hdr10' : null,
    hdr10Plus ? 'hdr10plus' : null,
    hlg ? 'hlg' : null,
    dolbyVision ? 'dv' : null,
  ].filter((name): name is string => name !== null);
}

/** Everything a stream request declares about this device, from one table: the
 * audio it decodes, the video it draws, and the HDR it displays. */
export function deviceDeclaration(caps: PlaybackCapabilities = capabilities()) {
  return {
    copyCodecs: copyableAudioCodecs(caps),
    videoCodecs: decodableVideoCodecs(caps),
    hdr: hdrDeclaration(caps),
  };
}
