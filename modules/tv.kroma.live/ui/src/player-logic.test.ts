import { describe, expect, it } from 'vitest';
import { HLS_CONFIG, isBehindLive, latencyOf, pauseWasDeliberate, recoveryFor, wantsNative } from './player-logic';

describe('recovering a player', () => {
  it('retries the network, then reloads the source', () => {
    expect(recoveryFor('network', 0)).toBe('startLoad');
    expect(recoveryFor('network', 6)).toBe('reload');
  });

  it('reloads at once when the playlist is gone, as after a channel change', () => {
    expect(recoveryFor('network', 0, 404)).toBe('reload');
  });

  it('recovers a media error a few times and then reloads', () => {
    expect(recoveryFor('media', 2)).toBe('recoverMedia');
    expect(recoveryFor('media', 3)).toBe('reload');
    expect(recoveryFor('other', 0)).toBe('give-up');
  });
});

describe('the live edge', () => {
  it('shows the back-to-live control only when well behind', () => {
    expect(isBehindLive(10, 14)).toBe(false);
    expect(isBehindLive(10, 20)).toBe(true);
    expect(isBehindLive(10, null)).toBe(false);
  });

  it('keeps hls.js to a short live sync and an infinite duration so no seek bar is drawn', () => {
    expect(HLS_CONFIG.liveSyncDurationCount).toBe(4);
    expect(HLS_CONFIG.liveDurationInfinity).toBe(true);
  });

  it('uses the native player only where there is no MSE', () => {
    expect(wantsNative({ mse: false, nativeHls: true })).toBe(true);
    expect(wantsNative({ mse: true, nativeHls: true })).toBe(false);
  });

  it('reads a pause during a reconnection as deliberate only just after a gesture', () => {
    expect(pauseWasDeliberate(500)).toBe(true);
    expect(pauseWasDeliberate(2000)).toBe(false);
    expect(pauseWasDeliberate(null)).toBe(false);
    expect(latencyOf(1000, 9000)).toBe(8);
    expect(latencyOf(null, 9000)).toBeNull();
  });
});
