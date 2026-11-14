import { describe, expect, it } from 'vitest';
import type { HdrCapabilities, PlaybackCapabilities } from './capabilities';
import { APPLE_TV_CAPS, MSE_CAPS, NATIVE_TV_CAPS, TIZEN_CAPS, WEBOS_CAPS } from './runtime-caps';
import {
  dolbyVisionBaseLayer,
  decodesDolbyVision,
  type PictureOutcome,
  pictureOutcome,
} from './dolby-vision';

const withHdr = (hdr: Partial<HdrCapabilities>): PlaybackCapabilities => ({
  ...NATIVE_TV_CAPS,
  hdr: { ...NATIVE_TV_CAPS.hdr, ...hdr },
});

const NO_DV = withHdr({ dolbyVision: false });
const SDR_PANEL = withHdr({ dolbyVision: false, hdr10: false, hdr10Plus: false, hlg: false });

const dv = (profile: number | null, compatibility: number | null = null) => ({
  hdr: true,
  hdrFormat: 'dolbyVision' as const,
  dolbyVisionProfile: profile,
  dolbyVisionCompatibility: compatibility,
});

describe('the base layer a Dolby Vision stream carries', () => {
  it.each([
    [0, 'none'],
    [1, 'hdr10'],
    [2, 'sdr'],
    [4, 'hlg'],
    [6, 'hdr10'],
  ] as const)('reads compatibility id %i as %s', (id, base) => {
    expect(dolbyVisionBaseLayer({ dolbyVisionProfile: 8, dolbyVisionCompatibility: id })).toBe(
      base,
    );
  });

  it('lets the signalled id outrank the profile number', () => {
    expect(dolbyVisionBaseLayer({ dolbyVisionProfile: 5, dolbyVisionCompatibility: 1 })).toBe(
      'hdr10',
    );
    expect(dolbyVisionBaseLayer({ dolbyVisionProfile: 8, dolbyVisionCompatibility: 0 })).toBe(
      'none',
    );
  });

  it('falls back to the profile only when the probe carried no id', () => {
    expect(dolbyVisionBaseLayer({ dolbyVisionProfile: 5 })).toBe('none');
    expect(dolbyVisionBaseLayer({ dolbyVisionProfile: 7 })).toBe('hdr10');
    expect(dolbyVisionBaseLayer({ dolbyVisionProfile: 8 })).toBeNull();
    expect(dolbyVisionBaseLayer({})).toBeNull();
  });

  it('ignores an id it does not know and reads the profile', () => {
    expect(dolbyVisionBaseLayer({ dolbyVisionProfile: 5, dolbyVisionCompatibility: 9 })).toBe(
      'none',
    );
  });
});

describe('Dolby Vision on a device that decodes it', () => {
  it.each([5, 7, 8, 10])('draws profile %i natively', (profile) => {
    expect(pictureOutcome(dv(profile), WEBOS_CAPS)).toEqual({ kind: 'native' });
  });

  it('honours a per-profile decoder list over the single flag', () => {
    const profile8Only = withHdr({ dolbyVisionProfiles: [8] });
    expect(decodesDolbyVision(profile8Only, 8)).toBe(true);
    expect(decodesDolbyVision(profile8Only, 5)).toBe(false);
    expect(pictureOutcome(dv(5, 0), profile8Only)).toEqual({ kind: 'refused' });
    expect(pictureOutcome(dv(7, 6), profile8Only)).toEqual({
      kind: 'base',
      source: 'dolbyVision',
      as: 'hdr10',
    });
  });
});

describe('Dolby Vision on an HDR10 device with no Dolby Vision decoder', () => {
  const cases: Array<[string, number | null, number | null, PictureOutcome]> = [
    ['profile 5 has no base layer', 5, 0, { kind: 'refused' }],
    ['profile 5 with no id is refused from the profile alone', 5, null, { kind: 'refused' }],
    [
      'profile 7 plays its HDR10 base layer',
      7,
      6,
      { kind: 'base', source: 'dolbyVision', as: 'hdr10' },
    ],
    [
      'profile 7 with no id plays as HDR10',
      7,
      null,
      { kind: 'base', source: 'dolbyVision', as: 'hdr10' },
    ],
    ['profile 8.1 plays as HDR10', 8, 1, { kind: 'base', source: 'dolbyVision', as: 'hdr10' }],
    ['profile 8.4 plays as HLG', 8, 4, { kind: 'base', source: 'dolbyVision', as: 'hlg' }],
    ['profile 8.2 plays as SDR', 8, 2, { kind: 'base', source: 'dolbyVision', as: 'sdr' }],
    [
      'profile 8 with no id is assumed to degrade as HDR10',
      8,
      null,
      { kind: 'base', source: 'dolbyVision', as: 'hdr10' },
    ],
    [
      'a stream that names no profile is assumed to degrade',
      null,
      null,
      {
        kind: 'base',
        source: 'dolbyVision',
        as: 'hdr10',
      },
    ],
  ];

  it.each(cases)('%s', (_name, profile, id, outcome) => {
    expect(pictureOutcome(dv(profile, id), NO_DV)).toEqual(outcome);
  });

  it('refuses profile 5 on Samsung, which has no Dolby Vision decoder', () => {
    expect(pictureOutcome(dv(5, 0), TIZEN_CAPS)).toEqual({ kind: 'refused' });
    expect(pictureOutcome(dv(8, 1), TIZEN_CAPS)).toMatchObject({ kind: 'base', as: 'hdr10' });
  });

  it('plays an HLG-compatible stream as HLG only where HLG is drawn', () => {
    expect(pictureOutcome(dv(8, 4), withHdr({ dolbyVision: false, hlg: false }))).toEqual({
      kind: 'toneMapped',
    });
  });
});

describe('Dolby Vision on an SDR display', () => {
  it('tone-maps the HDR10 base layer instead of drawing it', () => {
    expect(pictureOutcome(dv(7, 6), SDR_PANEL)).toEqual({ kind: 'toneMapped' });
    expect(pictureOutcome(dv(8, 1), SDR_PANEL)).toEqual({ kind: 'toneMapped' });
  });

  it('still refuses a stream with no base layer at all', () => {
    expect(pictureOutcome(dv(5, 0), SDR_PANEL)).toEqual({ kind: 'refused' });
  });

  it('draws an SDR-compatible base layer as it is', () => {
    expect(pictureOutcome(dv(8, 2), SDR_PANEL)).toEqual({
      kind: 'base',
      source: 'dolbyVision',
      as: 'sdr',
    });
  });
});

describe('the other HDR variants', () => {
  const hdr = (hdrFormat: 'hdr10' | 'hdr10Plus' | 'hlg' | null) => ({
    hdr: true,
    hdrFormat,
  });

  it('draws HDR10+ natively where its dynamic metadata is applied', () => {
    expect(pictureOutcome(hdr('hdr10Plus'), TIZEN_CAPS)).toEqual({ kind: 'native' });
  });

  it('draws HDR10+ as HDR10 where it is not', () => {
    expect(pictureOutcome(hdr('hdr10Plus'), WEBOS_CAPS)).toEqual({
      kind: 'base',
      source: 'hdr10Plus',
      as: 'hdr10',
    });
    expect(pictureOutcome(hdr('hdr10Plus'), APPLE_TV_CAPS)).toEqual({ kind: 'native' });
  });

  it('tone-maps every HDR variant on an SDR display', () => {
    for (const format of ['hdr10', 'hdr10Plus', 'hlg'] as const) {
      expect(pictureOutcome(hdr(format), SDR_PANEL)).toEqual({ kind: 'toneMapped' });
    }
  });

  it('draws HDR10 and HLG on a wide panel, with or without a Dolby Vision decoder', () => {
    expect(pictureOutcome(hdr('hdr10'), MSE_CAPS)).toEqual({ kind: 'native' });
    expect(pictureOutcome(hdr('hlg'), MSE_CAPS)).toEqual({ kind: 'native' });
  });

  it('assumes an HDR picture of unnamed variant is drawn', () => {
    expect(pictureOutcome(hdr(null), SDR_PANEL)).toEqual({ kind: 'native' });
  });

  it('has nothing to say about SDR or no video', () => {
    expect(pictureOutcome({ hdr: false }, SDR_PANEL)).toEqual({ kind: 'sdr' });
    expect(pictureOutcome(null, SDR_PANEL)).toEqual({ kind: 'sdr' });
  });
});
