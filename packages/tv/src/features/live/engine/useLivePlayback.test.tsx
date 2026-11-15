// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  CreateLiveArgs,
  LiveBackend,
  LiveEngine,
} from '#tv/features/live/engine/liveEngine';
import { useLivePlayback } from '#tv/features/live/engine/useLivePlayback';

function fakeBackend() {
  const created: { args: CreateLiveArgs; engine: LiveEngine & { behind: boolean } }[] = [];
  const backend: LiveBackend<LiveEngine & { behind: boolean }> = {
    createLiveEngine: (args) => {
      const engine = {
        behind: false,
        paused: true,
        play: vi.fn(function (this: { paused: boolean }) {
          this.paused = false;
        }),
        pause: vi.fn(function (this: { paused: boolean }) {
          this.paused = true;
        }),
        isPaused() {
          return this.paused;
        },
        isReady: () => true,
        isBehindLive() {
          return this.behind;
        },
        goLive: vi.fn(),
        frameTimeMs: () => null,
        bufferAheadMs: () => 0,
        moveRange: () => null,
        seekByMs: vi.fn(),
        setRate: vi.fn(),
        destroy: vi.fn(),
      };
      created.push({ args, engine });
      return engine;
    },
    Surface: () => null,
  };
  return { backend, created };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('playing a live stream', () => {
  it('builds no engine while there is no playlist', () => {
    const { backend, created } = fakeBackend();

    renderHook(() => useLivePlayback(backend, null));

    expect(created).toHaveLength(0);
  });

  it('starts the engine once, the first time it is ready', () => {
    const { backend, created } = fakeBackend();
    const { result } = renderHook(() => useLivePlayback(backend, 'u1'));

    act(() => {
      created[0]?.args.listeners.onReady();
      created[0]?.args.listeners.onReady();
    });

    expect(created[0]?.engine.play).toHaveBeenCalledTimes(1);
    expect(result.current.ready).toBe(true);
  });

  it('rebuilds the engine on the same playlist when it asks to be', () => {
    const { backend, created } = fakeBackend();
    renderHook(() => useLivePlayback(backend, 'u1'));

    act(() => created[0]?.args.listeners.onLost());

    expect(created).toHaveLength(2);
    expect(created[0]?.engine.destroy).toHaveBeenCalled();
    expect(created[1]?.args.url).toBe('u1');
  });

  it('builds a new engine for a new playlist and releases the old one', () => {
    const { backend, created } = fakeBackend();
    const { rerender } = renderHook(({ url }) => useLivePlayback(backend, url), {
      initialProps: { url: 'a' as string | null },
    });

    rerender({ url: 'b' });

    expect(created.map((c) => c.args.url)).toEqual(['a', 'b']);
    expect(created[0]?.engine.destroy).toHaveBeenCalled();
  });

  it('follows the live edge and goes back to it on request', () => {
    const { backend, created } = fakeBackend();
    const { result } = renderHook(() => useLivePlayback(backend, 'u1'));

    act(() => {
      if (created[0]) created[0].engine.behind = true;
      vi.advanceTimersByTime(600);
    });
    expect(result.current.behind).toBe(true);

    act(() => result.current.goLive());
    expect(created[0]?.engine.goLive).toHaveBeenCalled();
    expect(created[0]?.engine.play).toHaveBeenCalled();
  });

  it('toggles between play and pause', () => {
    const { backend, created } = fakeBackend();
    const { result } = renderHook(() => useLivePlayback(backend, 'u1'));

    act(() => result.current.togglePlay());
    act(() => result.current.togglePlay());

    expect(created[0]?.engine.play).toHaveBeenCalledTimes(1);
    expect(created[0]?.engine.pause).toHaveBeenCalledTimes(1);
  });

  it('reports a playback error and clears it once the picture plays', () => {
    const { backend, created } = fakeBackend();
    const { result } = renderHook(() => useLivePlayback(backend, 'u1'));

    act(() => created[0]?.args.listeners.onError());
    expect(result.current.failed).toBe(true);
    act(() => created[0]?.args.listeners.onPlaying());
    expect(result.current.failed).toBe(false);
  });
});
