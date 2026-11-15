// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const H = vi.hoisted(() => {
  const instances: FakeHls[] = [];
  class FakeHls {
    static Events = { MANIFEST_PARSED: 'm', FRAG_LOADED: 'f', ERROR: 'e' };
    static ErrorTypes = { NETWORK_ERROR: 'network', MEDIA_ERROR: 'media' };
    static isSupported = () => true;
    handlers = new Map<string, (...args: unknown[]) => void>();
    liveSyncPosition: number | null = 114;
    playingDate: Date | null = new Date(9000);
    startLoad = vi.fn();
    recoverMediaError = vi.fn();
    destroy = vi.fn();
    attachMedia = vi.fn();
    loadSource = vi.fn();
    constructor() {
      instances.push(this);
    }
    on(name: string, fn: (...args: unknown[]) => void) {
      this.handlers.set(name, fn);
    }
    emit(name: string, ...args: unknown[]) {
      this.handlers.get(name)?.(...args);
    }
  }
  return { instances, FakeHls };
});

vi.mock('hls.js', () => ({ default: H.FakeHls }));

import { useHls } from './use-hls';

function video(extra: Record<string, unknown> = {}) {
  const el = document.createElement('video');
  Object.defineProperty(el, 'readyState', { value: 4, configurable: true });
  el.play = vi.fn(() => Promise.resolve());
  el.pause = vi.fn();
  Object.assign(el, extra);
  return el;
}

beforeEach(() => {
  vi.useFakeTimers();
  H.instances.length = 0;
});
afterEach(() => vi.useRealTimers());

describe('useHls', () => {
  it('loads the playlist and starts playing once the manifest is parsed', async () => {
    const el = video();
    renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}));
    const hls = H.instances[0]!;

    expect(hls.loadSource).toHaveBeenCalledWith('https://x/a.m3u8');
    await act(async () => hls.emit('m'));
    expect(el.play).toHaveBeenCalled();
  });

  it('asks for a tap when the browser refuses to autoplay', async () => {
    const el = video({ play: vi.fn(() => Promise.reject(new Error('blocked'))) });
    const { result } = renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}));

    await act(async () => H.instances[0]!.emit('m'));

    expect(result.current.blocked).toBe(true);
  });

  it('shows "back to live" once the picture is further behind than the threshold', () => {
    const el = video({ currentTime: 100 });
    const { result } = renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}));

    act(() => {
      vi.advanceTimersByTime(600);
    });
    expect(result.current.behind).toBe(true);

    el.currentTime = 112;
    act(() => {
      vi.advanceTimersByTime(600);
    });
    expect(result.current.behind).toBe(false);
  });

  it('goLive jumps to the live edge and resumes, which clears the badge', () => {
    const el = video({ currentTime: 80 });
    const { result } = renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}));

    act(() => result.current.goLive());

    expect(el.currentTime).toBe(114);
    expect(el.play).toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(600);
    });
    expect(result.current.behind).toBe(false);
  });

  it('follows the element: pause and play events update the playing state', () => {
    const el = video();
    const { result } = renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}));
    Object.defineProperty(el, 'paused', { value: false, configurable: true });
    act(() => {
      el.dispatchEvent(new Event('play'));
    });
    expect(result.current.playing).toBe(true);
    Object.defineProperty(el, 'paused', { value: true, configurable: true });
    act(() => {
      el.dispatchEvent(new Event('pause'));
    });
    expect(result.current.playing).toBe(false);
  });

  it('toggle pauses a running player and resumes a paused one', () => {
    const el = video();
    const { result } = renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}));
    Object.defineProperty(el, 'paused', { value: false, configurable: true });
    act(() => result.current.toggle());
    expect(el.pause).toHaveBeenCalled();
    Object.defineProperty(el, 'paused', { value: true, configurable: true });
    act(() => result.current.toggle());
    expect(el.play).toHaveBeenCalled();
  });

  it('retries a network error in place, then reloads the stream and tells the page', () => {
    const el = video();
    const onLost = vi.fn();
    renderHook(() => useHls(el, 'https://x/a.m3u8', onLost));
    const first = H.instances[0]!;

    act(() => first.emit('e', {}, { fatal: true, type: 'network', response: { code: 404 } }));
    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(onLost).toHaveBeenCalledTimes(1);
    expect(first.destroy).toHaveBeenCalled();
    expect(H.instances.length).toBe(2);
  });

  it('retries media errors by recovering, and gives up on anything else', () => {
    const el = video();
    const { result } = renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}));
    const hls = H.instances[0]!;

    act(() => hls.emit('e', {}, { fatal: true, type: 'media' }));
    expect(hls.recoverMediaError).toHaveBeenCalled();

    act(() => hls.emit('e', {}, { fatal: true, type: 'other' }));
    expect(result.current.failed).toBe(true);
  });

  it('ignores a non fatal error', () => {
    const el = video();
    const { result } = renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}));

    act(() => H.instances[0]!.emit('e', {}, { fatal: false, type: 'network' }));

    expect(result.current.failed).toBe(false);
  });

  it('kicks the loader and jumps to the live edge when the picture stalls for too long', () => {
    const el = video({ currentTime: 100 });
    Object.defineProperty(el, 'readyState', { value: 1, configurable: true });
    Object.defineProperty(el, 'paused', { value: false, configurable: true });
    renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}));
    const hls = H.instances[0]!;

    act(() => {
      vi.advanceTimersByTime(6000);
    });

    expect(hls.startLoad).toHaveBeenCalled();
    expect(el.currentTime).toBe(114);
  });

  it('exposes a stable handle whose position is the frame on screen', () => {
    const el = video();
    const { result, rerender } = renderHook(() => useHls(el, 'https://x/a.m3u8', () => {}, 'c1'));
    const first = result.current.handle;
    rerender();

    expect(result.current.handle).toBe(first);
    expect(first?.getPositionMs()).toBe(9000);
  });

  it('destroys the player when the url goes away', () => {
    const el = video();
    const { rerender } = renderHook(
      ({ url }: { url: string | null }) => useHls(el, url, () => {}),
      { initialProps: { url: 'https://x/a.m3u8' as string | null } },
    );
    const hls = H.instances[0]!;

    rerender({ url: null });

    expect(hls.destroy).toHaveBeenCalled();
  });
});
