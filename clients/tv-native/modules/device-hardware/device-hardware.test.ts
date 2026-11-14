import { beforeEach, describe, expect, it, vi } from 'vitest';

const load = async (native: unknown) => {
  vi.resetModules();
  vi.doMock('expo', () => ({ requireOptionalNativeModule: () => native }));
  return import('./device-hardware');
};

beforeEach(() => {
  vi.doUnmock('expo');
});

describe('hdrSupport', () => {
  it('returns the native answer', async () => {
    const { hdrSupport } = await load({
      hdrSupport: () => ({ dolbyVisionDecoderProfiles: [5, 8], displayHdrTypes: ['dolbyVision'] }),
    });

    expect(hdrSupport()).toEqual({
      dolbyVisionDecoderProfiles: [5, 8],
      displayHdrTypes: ['dolbyVision'],
    });
  });

  it('is null without the native module', async () => {
    const { hdrSupport } = await load(null);

    expect(hdrSupport()).toBeNull();
  });

  it('is null on a native build that predates the query', async () => {
    const { hdrSupport } = await load({ cpuCores: () => 4 });

    expect(hdrSupport()).toBeNull();
  });

  it('is null when the native call throws', async () => {
    const { hdrSupport } = await load({
      hdrSupport: () => {
        throw new Error('display service unavailable');
      },
    });

    expect(hdrSupport()).toBeNull();
  });

  it.each([
    ['a number', 3],
    ['a list of strings in the profile field', { dolbyVisionDecoderProfiles: ['5'] }],
    ['an unknown display type', { displayHdrTypes: ['hdr12'] }],
  ])('is null for %s', async (_name, answer) => {
    const { hdrSupport } = await load({ hdrSupport: () => answer });

    expect(hdrSupport()).toBeNull();
  });
});
