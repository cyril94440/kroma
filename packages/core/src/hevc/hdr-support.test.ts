import { describe, expect, it } from 'vitest';
import { hdrDeclaration } from './capabilities';
import { canDirectPlay } from './directplay';
import { makeItem } from './directplay.fixture';
import { parseHdrSupport, withHdrSupport } from './hdr-support';
import { ANDROID_TV_CAPS, APPLE_TV_CAPS } from './runtime-caps';

const dolbyVision = (profile: number, compatibility?: number) =>
  makeItem({
    videoCodec: 'hevc',
    bitDepth: 10,
    hdrFormat: 'dolbyVision',
    dolbyVisionProfile: profile,
    dolbyVisionCompatibility: compatibility,
    audio: [],
  });

const direct = (profile: number, compatibility: number | undefined, caps = ANDROID_TV_CAPS) =>
  canDirectPlay(dolbyVision(profile, compatibility), caps).canDirectPlay;

describe('parseHdrSupport', () => {
  it('reads an Android answer', () => {
    expect(
      parseHdrSupport({
        dolbyVisionDecoderProfiles: [5, 8],
        displayHdrTypes: ['dolbyVision', 'hdr10'],
      }),
    ).toEqual({ dolbyVisionDecoderProfiles: [5, 8], displayHdrTypes: ['dolbyVision', 'hdr10'] });
  });

  it('reads an Apple answer that names the display alone', () => {
    expect(parseHdrSupport({ displayHdrTypes: ['hlg'] })).toEqual({ displayHdrTypes: ['hlg'] });
  });

  it.each([
    ['null', null],
    ['undefined', undefined],
    ['a string', 'dolbyVision'],
    ['a type no display reports', { displayHdrTypes: ['hlgPlus'] }],
    ['a profile that is not a number', { dolbyVisionDecoderProfiles: ['5'] }],
    ['a negative profile', { dolbyVisionDecoderProfiles: [-1] }],
    ['a display list that is not a list', { displayHdrTypes: 'hdr10' }],
  ])('rejects %s', (_name, answer) => {
    expect(parseHdrSupport(answer)).toBeNull();
  });
});

describe('withHdrSupport on Android TV', () => {
  it('keeps the static table when the device did not answer', () => {
    expect(withHdrSupport(ANDROID_TV_CAPS, null)).toBe(ANDROID_TV_CAPS);
    expect(withHdrSupport(ANDROID_TV_CAPS, undefined)).toBe(ANDROID_TV_CAPS);
    expect(withHdrSupport(ANDROID_TV_CAPS, {})).toBe(ANDROID_TV_CAPS);
    expect(hdrDeclaration(withHdrSupport(ANDROID_TV_CAPS, null))).toEqual([
      'hdr10',
      'hdr10plus',
      'hlg',
    ]);
  });

  it('claims Dolby Vision when a decoder exists and the display reports it', () => {
    const caps = withHdrSupport(ANDROID_TV_CAPS, {
      dolbyVisionDecoderProfiles: [5, 8],
      displayHdrTypes: ['dolbyVision', 'hdr10', 'hlg'],
    });

    expect(caps.hdr).toEqual({
      hdr10: true,
      hdr10Plus: false,
      dolbyVision: true,
      hlg: true,
      dolbyVisionProfiles: [5, 8],
    });
    expect(caps.hdrDeclared).toBe(true);
    expect(hdrDeclaration(caps)).toEqual(['hdr10', 'hlg', 'dv']);
  });

  it('does not claim Dolby Vision on a display that lacks it', () => {
    const caps = withHdrSupport(ANDROID_TV_CAPS, {
      dolbyVisionDecoderProfiles: [5, 8],
      displayHdrTypes: ['hdr10', 'hdr10Plus', 'hlg'],
    });

    expect(caps.hdr.dolbyVision).toBe(false);
    expect(caps.hdr.dolbyVisionProfiles).toBeUndefined();
    expect(hdrDeclaration(caps)).toEqual(['hdr10', 'hdr10plus', 'hlg']);
  });

  it('does not claim Dolby Vision without a decoder', () => {
    const caps = withHdrSupport(ANDROID_TV_CAPS, {
      dolbyVisionDecoderProfiles: [],
      displayHdrTypes: ['dolbyVision', 'hdr10'],
    });

    expect(caps.hdr.dolbyVision).toBe(false);
    expect(hdrDeclaration(caps)).toEqual(['hdr10']);
  });

  it('does not claim Dolby Vision when the display was not asked', () => {
    const caps = withHdrSupport(ANDROID_TV_CAPS, { dolbyVisionDecoderProfiles: [5] });

    expect(caps.hdr).toEqual({ ...ANDROID_TV_CAPS.hdr, dolbyVision: false });
    expect(hdrDeclaration(caps)).toEqual(['hdr10', 'hdr10plus', 'hlg']);
  });

  it('declares an empty list for an SDR display', () => {
    const caps = withHdrSupport(ANDROID_TV_CAPS, {
      dolbyVisionDecoderProfiles: [5, 8],
      displayHdrTypes: [],
    });

    expect(caps.hdr).toEqual({ hdr10: false, hdr10Plus: false, dolbyVision: false, hlg: false });
    expect(hdrDeclaration(caps)).toEqual([]);
  });

  it('keeps an AV1-only Dolby Vision decoder out of the declaration', () => {
    const caps = withHdrSupport(ANDROID_TV_CAPS, {
      dolbyVisionDecoderProfiles: [10],
      displayHdrTypes: ['dolbyVision', 'hdr10'],
    });

    expect(caps.hdr.dolbyVision).toBe(true);
    expect(hdrDeclaration(caps)).toEqual(['hdr10']);
  });
});

describe('withHdrSupport on Apple TV', () => {
  it('claims Dolby Vision for every profile when the display mode lists it', () => {
    const caps = withHdrSupport(APPLE_TV_CAPS, {
      displayHdrTypes: ['dolbyVision', 'hdr10', 'hlg'],
    });

    expect(caps.hdr).toEqual({ hdr10: true, hdr10Plus: false, dolbyVision: true, hlg: true });
    expect(hdrDeclaration(caps)).toEqual(['hdr10', 'hlg', 'dv']);
  });

  it('claims no Dolby Vision on an HDR10 display', () => {
    const caps = withHdrSupport(APPLE_TV_CAPS, { displayHdrTypes: ['hdr10', 'hlg'] });

    expect(caps.hdr.dolbyVision).toBe(false);
    expect(hdrDeclaration(caps)).toEqual(['hdr10', 'hlg']);
  });

  it('declares an empty list when no HDR mode is available', () => {
    expect(hdrDeclaration(withHdrSupport(APPLE_TV_CAPS, { displayHdrTypes: [] }))).toEqual([]);
  });
});

describe('Dolby Vision direct play after the device answered', () => {
  const shield = withHdrSupport(ANDROID_TV_CAPS, {
    dolbyVisionDecoderProfiles: [5, 8],
    displayHdrTypes: ['dolbyVision', 'hdr10', 'hlg'],
  });
  const hdr10Box = withHdrSupport(ANDROID_TV_CAPS, {
    dolbyVisionDecoderProfiles: [],
    displayHdrTypes: ['hdr10', 'hlg'],
  });
  const sdrBox = withHdrSupport(ANDROID_TV_CAPS, { displayHdrTypes: [] });

  it('plays profile 5 only where a profile 5 decoder and a Dolby Vision display exist', () => {
    expect(direct(5, undefined, shield)).toBe(true);
    expect(direct(5, undefined, hdr10Box)).toBe(false);
    expect(direct(5, undefined, ANDROID_TV_CAPS)).toBe(false);
  });

  it('plays profile 8 as Dolby Vision on the Shield and as its HDR10 layer elsewhere', () => {
    expect(direct(8, 1, shield)).toBe(true);
    expect(direct(8, 1, hdr10Box)).toBe(true);
  });

  it('plays profile 7 as its HDR10 base layer where only profiles 5 and 8 decode', () => {
    expect(direct(7, 6, shield)).toBe(true);
    expect(direct(7, 6, hdr10Box)).toBe(true);
  });

  it('refuses profile 5 on an SDR display, whose base layer does not exist', () => {
    expect(direct(5, undefined, sdrBox)).toBe(false);
  });
});
