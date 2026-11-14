import { describe, expect, it } from 'vitest';
import { surfaceCapabilities } from './engine';
import { AVPLAY_HLS_CAPS, MPV_CAPS, SAFARI_CAPS } from './runtime-caps';

describe('surfaceCapabilities', () => {
  it('answers for the plane that decodes, not for the page that was probed', () => {
    expect(surfaceCapabilities('mpv')).toBe(MPV_CAPS);
    expect(surfaceCapabilities('avplay')).toBe(AVPLAY_HLS_CAPS);
  });

  it('reads AVPlay through the HDR the set itself reported, with the audio its HLS path plays', () => {
    const sdrSet = { ...SAFARI_CAPS, hdr: { ...SAFARI_CAPS.hdr, hdr10: false, hlg: false } };
    const caps = surfaceCapabilities('avplay', sdrSet);
    expect(caps.hdr).toBe(sdrSet.hdr);
    expect(caps.audio).toBe(AVPLAY_HLS_CAPS.audio);
  });
});
