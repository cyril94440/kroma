// @vitest-environment jsdom

import { act } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  cleanupEngineFixture,
  P,
  renderHook,
  resetEngineFixture,
} from '#mobile/player/engine/engine.fixture';

const { useLiveEngine } = await import('./use-live-engine');

interface LivePlayer {
  currentOffsetFromLive: number | null;
  currentLiveTimestamp: number | null;
  seekBy: ReturnType<typeof vi.fn>;
  replaceAsync: ReturnType<typeof vi.fn>;
  play: ReturnType<typeof vi.fn>;
  emit(type: string, payload: unknown): void;
}

const live = () => P.box.current as unknown as LivePlayer;

beforeEach(() => {
  resetEngineFixture();
  vi.useFakeTimers();
  Object.assign(live(), {
    currentOffsetFromLive: null,
    currentLiveTimestamp: null,
    seekBy: vi.fn(),
  });
});
afterEach(() => {
  vi.useRealTimers();
  cleanupEngineFixture();
});

const URL = 'http://kroma.test/api/module-files/tv.kroma.live/run/index.m3u8?t=x';

describe('the phone live engine', () => {
  it('opens nothing without a playlist', async () => {
    renderHook(() => useLiveEngine(null));
    await act(() => vi.advanceTimersByTimeAsync(0));

    expect(live().replaceAsync).not.toHaveBeenCalled();
  });

  it('opens the playlist as HLS and plays', async () => {
    renderHook(() => useLiveEngine(URL));
    await act(() => vi.advanceTimersByTimeAsync(0));

    expect(live().replaceAsync).toHaveBeenCalledWith({ uri: URL, contentType: 'hls' });
    expect(live().play).toHaveBeenCalled();
  });

  it('is behind the live edge past six seconds and goes back by the offset', async () => {
    const { result } = renderHook(() => useLiveEngine(URL));

    live().currentOffsetFromLive = 3;
    await act(() => vi.advanceTimersByTimeAsync(600));
    expect(result.current.behind).toBe(false);

    live().currentOffsetFromLive = 11;
    await act(() => vi.advanceTimersByTimeAsync(600));
    expect(result.current.behind).toBe(true);

    act(() => result.current.goLive());
    expect(live().seekBy).toHaveBeenCalledWith(11);
  });

  it('reads the frame time from the playlist programme date', () => {
    const { result } = renderHook(() => useLiveEngine(URL));

    expect(result.current.frameTimeMs()).toBeNull();
    live().currentLiveTimestamp = 1_700_000_000_000;
    expect(result.current.frameTimeMs()).toBe(1_700_000_000_000);
  });

  it('reopens the source after a stalled load and gives up after three rebuilds', async () => {
    const { result } = renderHook(() => useLiveEngine(URL));
    await act(() => vi.advanceTimersByTimeAsync(0));
    const opened = () => live().replaceAsync.mock.calls.length;
    const first = opened();

    for (let i = 0; i < 4; i += 1) {
      await act(async () => {
        live().emit('statusChange', { status: 'error' });
        await vi.advanceTimersByTimeAsync(0);
      });
    }

    expect(opened()).toBe(first + 3);
    expect(result.current.failed).toBe(true);
  });

  it('clears a failure once the picture is ready', async () => {
    const { result } = renderHook(() => useLiveEngine(URL));
    await act(async () => {
      live().emit('statusChange', { status: 'readyToPlay' });
    });

    expect(result.current.failed).toBe(false);
    expect(result.current.waiting).toBe(false);
  });

  it('gives the watch party the frame, the buffer and the three moves of the player', () => {
    const { result } = renderHook(() => useLiveEngine(URL));
    Object.assign(live(), {
      currentLiveTimestamp: 1_700_000_000_000,
      currentOffsetFromLive: 5,
      bufferedPosition: 14,
      currentTime: 10,
      status: 'readyToPlay',
      playbackRate: 1,
    });
    const source = result.current.sync.current;

    source.seekByMs(-2500);
    source.goLive();
    source.setRate(1.05);

    expect([
      source.frameTimeMs(),
      source.bufferAheadMs(),
      source.isReady(),
      source.moveRange(),
      live().seekBy.mock.calls,
      (live() as unknown as { playbackRate: number }).playbackRate,
    ]).toEqual([1_700_000_000_000, 4000, true, null, [[-2.5], [5]], 1.05]);
  });
});
