import { describe, expect, it } from 'vitest';
import { qualityTone } from './badge';

describe('qualityTone', () => {
  it.each(['HDR', 'HDR10', 'HDR10+', 'HLG', 'Dolby Vision'])('gives %s the HDR hue', (label) => {
    expect(qualityTone(label)).toBe('HDR');
  });

  it('keeps H.265 and the amber default for resolution', () => {
    expect(qualityTone('H.265')).toBe('H.265');
    expect(qualityTone('4K')).toBe('4K');
  });

  it.each(['Dolby Atmos', 'DTS:X'])(
    'gives the object-based audio badge %s the info hue',
    (label) => {
      expect(qualityTone(label)).toBe('info');
    },
  );
});
