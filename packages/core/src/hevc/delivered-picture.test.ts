import { describe, expect, it } from 'vitest';
import { deliveredVideo } from './delivered-picture';
import { makeItem } from './directplay.fixture';
import { MSE_CAPS, TIZEN_CAPS, WEBOS_CAPS } from './runtime-caps';

const video = (p: Parameters<typeof makeItem>[0]) =>
  makeItem({ videoCodec: 'hevc', bitDepth: 10, ...p }).video;

const P7 = video({
  hdrFormat: 'dolbyVision',
  dolbyVisionProfile: 7,
  dolbyVisionCompatibility: 6,
  audio: [],
});

describe('deliveredVideo', () => {
  it('keeps profile 7 as Dolby Vision for the original file on a device that decodes it', () => {
    expect(deliveredVideo(P7, WEBOS_CAPS, 'file')?.hdrFormat).toBe('dolbyVision');
  });

  it('delivers profile 7 as HDR10 over HLS, on a device that decodes Dolby Vision too', () => {
    expect(deliveredVideo(P7, WEBOS_CAPS, 'stream')).toMatchObject({
      hdr: true,
      hdrFormat: 'hdr10',
    });
  });

  it('delivers HDR as SDR where the display is tone-mapped', () => {
    const sdr = { ...WEBOS_CAPS, hdr: { ...WEBOS_CAPS.hdr, hdr10: false, hlg: false } };
    expect(deliveredVideo(P7, sdr, 'stream')).toMatchObject({ hdr: false, hdrFormat: null });
  });

  it('names the layer a device without a decoder draws', () => {
    const p8 = video({
      hdrFormat: 'dolbyVision',
      dolbyVisionProfile: 8,
      dolbyVisionCompatibility: 4,
      audio: [],
    });
    expect(deliveredVideo(p8, TIZEN_CAPS, 'stream')?.hdrFormat).toBe('hlg');
  });

  it('leaves SDR and an HDR10 title on an HDR10 device untouched', () => {
    const sdr = video({ audio: [] });
    const hdr10 = video({ hdrFormat: 'hdr10', audio: [] });
    expect(deliveredVideo(sdr, MSE_CAPS, 'stream')).toBe(sdr);
    expect(deliveredVideo(hdr10, MSE_CAPS, 'stream')).toBe(hdr10);
    expect(deliveredVideo(null, MSE_CAPS, 'stream')).toBeNull();
  });
});
