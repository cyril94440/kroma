import { describe, expect, it } from 'vitest';
import {
  LIVE_HLS_CONFIG,
  canRebuild,
  isBehindLive,
  latencyOf,
  liveRecoveryFor,
  wantsNativeHls,
} from './live-player';

describe('recovering a player', () => {
  it('retries the network, then reloads the source', () => {
    expect(liveRecoveryFor('network', 0)).toBe('startLoad');
    expect(liveRecoveryFor('network', 6)).toBe('reload');
  });

  it('reloads at once when the playlist is gone, as after a channel change', () => {
    expect(liveRecoveryFor('network', 0, 404)).toBe('reload');
  });

  it('recovers a media error a few times and then reloads', () => {
    expect(liveRecoveryFor('media', 2)).toBe('recoverMedia');
    expect(liveRecoveryFor('media', 3)).toBe('reload');
    expect(liveRecoveryFor('other', 0)).toBe('give-up');
  });
});

describe('the live edge', () => {
  it('shows the back-to-live control only when well behind', () => {
    expect(isBehindLive(10, 14)).toBe(false);
    expect(isBehindLive(10, 20)).toBe(true);
    expect(isBehindLive(10, null)).toBe(false);
  });

  it('keeps hls.js to a short live sync and an infinite duration so no seek bar is drawn', () => {
    expect(LIVE_HLS_CONFIG.liveSyncDurationCount).toBe(4);
    expect(LIVE_HLS_CONFIG.liveDurationInfinity).toBe(true);
  });

  it('uses the native player only where there is no MSE', () => {
    expect(wantsNativeHls({ mse: false, nativeHls: true })).toBe(true);
    expect(wantsNativeHls({ mse: true, nativeHls: true })).toBe(false);
  });

  it('measures the latency of the frame on screen against the clock', () => {
    expect(latencyOf(1000, 9000)).toBe(8);
    expect(latencyOf(null, 9000)).toBeNull();
  });
});

describe('rebuilding a player', () => {
  it('rebuilds three times and then gives up', () => {
    expect([0, 1, 2, 3].map(canRebuild)).toEqual([true, true, true, false]);
  });
});
