import type { AudioTrack } from '@kromatv/client/media';
import { describe, expect, it } from 'vitest';
import { audioBadgeForTrack, audioBadgeForTracks, audioFormatName, spatialOf } from './audio-label';

const track = (p: Partial<AudioTrack> & { codec: string }): AudioTrack =>
  ({ index: 0, channels: null, language: null, default: false, ...p }) as AudioTrack;

describe('audioFormatName', () => {
  it.each([
    [{ codec: 'truehd', spatial: 'atmos' as const }, 'Dolby TrueHD Atmos'],
    [{ codec: 'truehd' }, 'Dolby TrueHD'],
    [{ codec: 'eac3', spatial: 'atmos' as const }, 'Dolby Digital+ Atmos'],
    [{ codec: 'eac3' }, 'Dolby Digital+'],
    [{ codec: 'ac3' }, 'Dolby Digital'],
    [{ codec: 'dts', profile: 'DTS-HD MA' }, 'DTS-HD MA'],
    [{ codec: 'dts', profile: 'DTS-HD MA + DTS:X', spatial: 'dtsx' as const }, 'DTS:X'],
    [{ codec: 'dts', profile: 'DTS-HD HRA' }, 'DTS-HD HRA'],
    [{ codec: 'dts', profile: 'DTS-ES' }, 'DTS-ES'],
    [{ codec: 'dts', profile: 'DTS 96/24' }, 'DTS 96/24'],
    [{ codec: 'dts', profile: 'DTS' }, 'DTS'],
    [{ codec: 'dts' }, 'DTS'],
    [{ codec: 'aac' }, 'AAC'],
    [{ codec: 'flac' }, 'FLAC'],
    [{ codec: 'pcm_s24le' }, 'PCM'],
    [{ codec: 'wmapro' }, 'WMAPRO'],
  ])('names %j as %s', (fields, name) => {
    expect(audioFormatName(track(fields))).toBe(name);
  });

  it('reads Atmos off the profile string when the payload predates `spatial`', () => {
    expect(
      audioFormatName(track({ codec: 'eac3', profile: 'Dolby Digital Plus + Dolby Atmos' })),
    ).toBe('Dolby Digital+ Atmos');
  });
});

describe('spatialOf', () => {
  it('prefers the probe field over the profile', () => {
    expect(spatialOf(track({ codec: 'dts', spatial: 'dtsx', profile: 'DTS' }))).toBe('dtsx');
  });

  it('reads DTS:X from a profile string', () => {
    expect(spatialOf(track({ codec: 'dts', profile: 'DTS-HD MA + DTS:X' }))).toBe('dtsx');
  });

  it('is null for a channel-based track and for no track', () => {
    expect(spatialOf(track({ codec: 'ac3', profile: 'Dolby Digital' }))).toBeNull();
    expect(spatialOf(null)).toBeNull();
  });
});

describe('audio badges', () => {
  it('badges only object-based tracks, by their proper names', () => {
    expect(audioBadgeForTrack(track({ codec: 'truehd', spatial: 'atmos' }))).toBe('Dolby Atmos');
    expect(audioBadgeForTrack(track({ codec: 'dts', spatial: 'dtsx' }))).toBe('DTS:X');
    expect(audioBadgeForTrack(track({ codec: 'eac3' }))).toBeNull();
  });

  it('picks Atmos over DTS:X when an item carries both, and none when it carries neither', () => {
    const atmos = track({ codec: 'eac3', spatial: 'atmos' });
    const dtsx = track({ codec: 'dts', spatial: 'dtsx' });
    expect(audioBadgeForTracks([dtsx, atmos])).toBe('Dolby Atmos');
    expect(audioBadgeForTracks([dtsx])).toBe('DTS:X');
    expect(audioBadgeForTracks([track({ codec: 'aac' })])).toBeNull();
    expect(audioBadgeForTracks(undefined)).toBeNull();
  });
});
