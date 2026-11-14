// @vitest-environment jsdom
import { renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { expect, it } from 'vitest';
import { PlaybackCapabilitiesProvider, usePlaybackCapabilities } from './playbackCapabilities';

it('keeps decoder limits isolated between mounted televisions', () => {
  const a = renderHook(usePlaybackCapabilities, {
    wrapper: ({ children }: { children: ReactNode }) => (
      <PlaybackCapabilitiesProvider limits={{ hevc: { width: 1920, height: 1080 } }}>
        {children}
      </PlaybackCapabilitiesProvider>
    ),
  });
  const b = renderHook(usePlaybackCapabilities, {
    wrapper: ({ children }: { children: ReactNode }) => (
      <PlaybackCapabilitiesProvider limits={{ hevc: { width: 3840, height: 2160 } }}>
        {children}
      </PlaybackCapabilitiesProvider>
    ),
  });
  expect(a.result.current.frameLimits?.hevc?.width).toBe(1920);
  expect(b.result.current.frameLimits?.hevc?.width).toBe(3840);
  a.unmount();
  expect(b.result.current.frameLimits?.hevc?.width).toBe(3840);
  b.unmount();
});

it('refines the static table with what the television reports about HDR', () => {
  const { result } = renderHook(usePlaybackCapabilities, {
    wrapper: ({ children }: { children: ReactNode }) => (
      <PlaybackCapabilitiesProvider
        hdrSupport={{
          dolbyVisionDecoderProfiles: [5, 8],
          displayHdrTypes: ['dolbyVision', 'hdr10'],
        }}
      >
        {children}
      </PlaybackCapabilitiesProvider>
    ),
  });

  expect(result.current.hdr.dolbyVision).toBe(true);
  expect(result.current.hdr.dolbyVisionProfiles).toEqual([5, 8]);
  expect(result.current.hdr.hlg).toBe(false);
});
