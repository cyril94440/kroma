// Streaming guesses optimistically because a rejected direct attempt falls back
// to the HLS master; downloading cannot, so `canRawDownload` is strictly
// stronger than `decideSource`.

import type { MediaItem } from '@kromatv/client/media';
import { recordingClient } from '@kromatv/client/test';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const rn = vi.hoisted(() => ({ os: 'ios' as 'ios' | 'android' }));
vi.mock('react-native', () => ({
  Platform: {
    get OS() {
      return rn.os;
    },
  },
}));

import {
  ANDROID_CAPS,
  canRawDownload,
  decideSource,
  downloadCopyCodecs,
  IOS_CAPS,
  mobileCaps,
  mobileHdr,
  mobileVideoCodecs,
} from './caps';

type Track = { codec: string; default?: boolean; channels?: number };
interface Over {
  container?: string;
  video?: { codec: string; bitDepth?: number };
  audio?: Track[];
}

// `container` is read by PRESENCE so a case can hand over a file whose
// container the server never reported; a `?? 'mp4'` default would hide that.
const item = (over: Over = {}): MediaItem =>
  ({
    id: 'itm_1',
    container: 'container' in over ? over.container : 'mp4',
    video: over.video ?? { codec: 'h264' },
    audioTracks: over.audio ?? [{ codec: 'aac', default: true }],
    subtitles: [],
  }) as unknown as MediaItem;

beforeEach(() => {
  rn.os = 'ios';
});

describe('the capability tables', () => {
  it('picks the table for the runtime it is on', () => {
    rn.os = 'ios';
    expect(mobileCaps()).toBe(IOS_CAPS);
    rn.os = 'android';
    expect(mobileCaps()).toBe(ANDROID_CAPS);
  });

  it('claims no AV1 on either, which is a deliberate under-claim', () => {
    // A17+ only on iOS, and pre-2023 Android SoCs lack it entirely.
    expect(IOS_CAPS.av1).toBe(false);
    expect(ANDROID_CAPS.av1).toBe(false);
  });

  it('gives Dolby to both: AVPlayer natively, ExoPlayer through the bundled FFmpeg decoders', () => {
    expect(IOS_CAPS.audio.ac3 && IOS_CAPS.audio.eac3).toBe(true);
    expect(ANDROID_CAPS.audio.ac3 && ANDROID_CAPS.audio.eac3).toBe(true);
  });

  it('gives VP9 and Opus to ExoPlayer and not to AVPlayer', () => {
    expect(ANDROID_CAPS.vp9 && ANDROID_CAPS.audio.opus).toBe(true);
    expect(IOS_CAPS.vp9 || IOS_CAPS.audio.opus).toBe(false);
  });

  it('gives DTS and TrueHD to ExoPlayer alone, which carries the FFmpeg decoders AVPlayer lacks', () => {
    expect(IOS_CAPS.audio.dts || IOS_CAPS.audio.truehd).toBe(false);
    expect(ANDROID_CAPS.audio.dts && ANDROID_CAPS.audio.truehd).toBe(true);
  });

  it('claims Dolby Vision on iOS only, because nothing can ask an Android handset', () => {
    expect(IOS_CAPS.hdr.dolbyVision).toBe(true);
    expect(ANDROID_CAPS.hdr.dolbyVision).toBe(false);
  });

  it('claims HDR10+ nowhere on iOS, which applies no dynamic metadata, and declares both tables', () => {
    expect(IOS_CAPS.hdr.hdr10Plus).toBe(false);
    expect(IOS_CAPS.hdrDeclared && ANDROID_CAPS.hdrDeclared).toBe(true);
  });

  it('declares each runtime display to the server', () => {
    rn.os = 'ios';
    expect(mobileHdr()).toEqual(['hdr10', 'hlg', 'dv']);
    rn.os = 'android';
    expect(mobileHdr()).toEqual(['hdr10', 'hdr10plus', 'hlg']);
  });
});

describe('the download codec sets', () => {
  it('asks only for audio this runtime decodes and ffmpeg can copy into fMP4', () => {
    rn.os = 'android';
    const copy = downloadCopyCodecs();
    expect(copy).toEqual(['aac', 'ac3', 'eac3']);
    expect(copy).not.toContain('dts');
    expect(copy).not.toContain('truehd');
  });

  it('keeps surround where the runtime really decodes it', () => {
    rn.os = 'ios';
    expect(downloadCopyCodecs()).toContain('ac3');
  });

  it('asks only for video this runtime decodes in hardware', () => {
    rn.os = 'ios';
    expect(mobileVideoCodecs()).toEqual(['hevc', 'h264']);
    rn.os = 'android';
    expect(mobileVideoCodecs()).toEqual(['hevc', 'h264', 'vp9']);
  });

  it('never claims the 10-bit H.264 neither phone decoder implements', () => {
    for (const os of ['ios', 'android'] as const) {
      rn.os = os;
      expect(mobileVideoCodecs()).not.toContain('h26410bit');
    }
  });

  it('never asks for a codec ffmpeg cannot stream-copy into fMP4', () => {
    for (const os of ['ios', 'android'] as const) {
      rn.os = os;
      expect(downloadCopyCodecs().every((c) => typeof c === 'string' && c.length > 0)).toBe(true);
    }
  });
});

describe('the master for audio the runtime cannot copy', () => {
  it('leaves surround DTS to the server E-AC-3 transcode where the runtime decodes Dolby', () => {
    const dts = item({ container: 'mkv', audio: [{ codec: 'dts', default: true, channels: 6 }] });
    for (const os of ['ios', 'android'] as const) {
      rn.os = os;
      expect(decideSource(dts).aacMaster).toBe(false);
    }
  });

  it('keeps a stereo track outside the copy set on AAC', () => {
    rn.os = 'ios';
    const flac = item({ audio: [{ codec: 'flac', default: true, channels: 2 }] });
    expect(decideSource(flac).aacMaster).toBe(true);
  });
});

describe('the exact master a phone asks for', () => {
  const { client } = recordingClient();
  const masterFor = (source: MediaItem) =>
    client.media
      .hlsMasterUrl(source.id, decideSource(source).aacMaster, 0, 0, {
        videoCodecs: mobileVideoCodecs(),
        hdr: mobileHdr(),
        copyCodecs: downloadCopyCodecs(),
      })
      .replace('http://kroma.test/api', '');

  it.each(['ios', 'android'] as const)(
    'asks %s for the copy program with Dolby declared and no TrueHD, so a TrueHD 7.1 track comes back as E-AC-3',
    (os) => {
      rn.os = os;
      const truehd = item({
        container: 'mkv',
        audio: [{ codec: 'truehd', default: true, channels: 8 }],
      });

      expect(masterFor(truehd)).toContain('/hls/copy/0/0/index.m3u8?copy=aac%2Cac3%2Ceac3&');
    },
  );

  it.each(['ios', 'android'] as const)(
    'asks %s for the same copy program for DTS 5.1 and for a stereo AAC track',
    (os) => {
      rn.os = os;
      const dts = item({ container: 'mkv', audio: [{ codec: 'dts', default: true, channels: 6 }] });
      const stereo = item({ audio: [{ codec: 'aac', default: true, channels: 2 }] });

      expect(masterFor(dts)).toContain('/hls/copy/0/0/index.m3u8?copy=aac%2Cac3%2Ceac3&');
      expect(masterFor(stereo)).toContain('/hls/copy/0/0/index.m3u8?copy=aac%2Cac3%2Ceac3&');
    },
  );
});

describe('deciding the streaming source', () => {
  it('goes direct for a plain single-audio MP4 on iOS', () => {
    rn.os = 'ios';
    expect(decideSource(item()).direct).toBe(true);
  });

  it('remuxes a container AVPlayer cannot demux', () => {
    rn.os = 'ios';
    expect(decideSource(item({ container: 'mkv' })).direct).toBe(false);
  });

  it('demuxes MKV on Android', () => {
    rn.os = 'android';
    expect(decideSource(item({ container: 'mkv' })).direct).toBe(true);
  });

  it('remuxes a MULTI-AUDIO file on iOS even when everything decodes', () => {
    rn.os = 'ios';
    const multi = item({ audio: [{ codec: 'aac', default: true }, { codec: 'ac3' }] });
    // AVFoundation exposes local audio selection only for alternate-grouped
    // tracks, so played direct the track picker would do nothing.
    expect(decideSource(multi).direct).toBe(false);
  });

  it('goes direct for the same file on Android', () => {
    rn.os = 'android';
    const multi = item({ audio: [{ codec: 'aac', default: true }, { codec: 'ac3' }] });
    // ExoPlayer selects tracks in place, and an undecodable one falls back.
    expect(decideSource(multi).direct).toBe(true);
  });

  it('remuxes when iOS cannot decode the single track it has', () => {
    rn.os = 'ios';
    expect(decideSource(item({ audio: [{ codec: 'opus', default: true }] })).direct).toBe(false);
  });

  it('remuxes a video codec the runtime does not claim', () => {
    for (const os of ['ios', 'android'] as const) {
      rn.os = os;
      expect(decideSource(item({ video: { codec: 'av1' } })).direct).toBe(false);
    }
  });

  it('always reports whether the master would need AAC', () => {
    rn.os = 'android';
    const unnamed = item({ container: 'mkv', audio: [{ codec: 'unknown', default: true }] });
    expect(decideSource(unnamed).aacMaster).toBe(true);
    expect(decideSource(item()).aacMaster).toBe(false);
  });

  it('treats a missing container as undemuxable rather than guessing', () => {
    rn.os = 'ios';
    expect(decideSource(item({ container: undefined })).direct).toBe(false);
  });

  it('is case-insensitive about the container', () => {
    rn.os = 'ios';
    // ffprobe reports `MOV` on some files and `mov` on others.
    expect(decideSource(item({ container: 'MP4' })).direct).toBe(true);
  });

  it('takes the only audio track when none is flagged default', () => {
    rn.os = 'ios';
    expect(decideSource(item({ audio: [{ codec: 'aac' }] })).direct).toBe(true);
    expect(decideSource(item({ audio: [{ codec: 'opus' }] })).direct).toBe(false);
  });
});

describe('deciding whether the raw file can be downloaded', () => {
  it('allows a file whose every part is decodable', () => {
    rn.os = 'ios';
    expect(canRawDownload(item())).toBe(true);
  });

  it('is STRICTER than the streaming decision', () => {
    rn.os = 'android';
    const undecodable = item({ container: 'mkv', audio: [{ codec: 'wmapro', default: true }] });
    expect(decideSource(undecodable).direct).toBe(true);
    expect(canRawDownload(undecodable)).toBe(false);
  });

  it('refuses a container the runtime cannot demux', () => {
    rn.os = 'ios';
    expect(canRawDownload(item({ container: 'mkv' }))).toBe(false);
  });

  it('refuses a video codec the runtime cannot decode', () => {
    rn.os = 'android';
    expect(canRawDownload(item({ container: 'mp4', video: { codec: 'av1' } }))).toBe(false);
  });

  it('refuses a file with NO audio track at all', () => {
    rn.os = 'android';
    expect(canRawDownload(item({ audio: [] }))).toBe(false);
  });

  it('refuses a multi-audio file on iOS, matching the streaming rule', () => {
    rn.os = 'ios';
    const multi = item({ audio: [{ codec: 'aac', default: true }, { codec: 'aac' }] });
    expect(canRawDownload(multi)).toBe(false);
  });

  it('allows a multi-audio file on Android when EVERY track decodes', () => {
    rn.os = 'android';
    const multi = item({
      container: 'mkv',
      audio: [{ codec: 'aac', default: true }, { codec: 'opus' }],
    });
    expect(canRawDownload(multi)).toBe(true);
  });

  it('refuses when only ONE of several tracks is undecodable', () => {
    rn.os = 'android';
    const mixed = item({
      container: 'mkv',
      audio: [{ codec: 'aac', default: true }, { codec: 'wmapro' }],
    });
    expect(canRawDownload(mixed)).toBe(false);
  });

  it('refuses a track with no codec reported', () => {
    rn.os = 'android';
    expect(canRawDownload(item({ audio: [{ codec: '' as string }] }))).toBe(false);
  });

  it('refuses a file whose container the server never reported', () => {
    rn.os = 'ios';
    expect(canRawDownload(item({ container: undefined }))).toBe(false);
  });
});
