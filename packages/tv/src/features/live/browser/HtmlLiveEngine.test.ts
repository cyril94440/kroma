// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HtmlLiveEngine } from '#tv/features/live/browser/HtmlLiveEngine';
import { fakeLiveListeners } from '#tv/features/live/engine/liveEngine.fixture';

const hls = vi.hoisted(() => ({
  supported: true,
  handlers: new Map<string, (event: string, data: unknown) => void>(),
  instance: null as null | Record<string, unknown>,
  startLoad: vi.fn(),
  recoverMediaError: vi.fn(),
  loadSource: vi.fn(),
  attachMedia: vi.fn(),
  destroy: vi.fn(),
}));

vi.mock('hls.js', () => ({
  default: class {
    static Events = { ERROR: 'hlsError', FRAG_LOADED: 'hlsFragLoaded' };
    static isSupported() {
      return hls.supported;
    }
    liveSyncPosition: number | null = 100;
    playingDate: Date | null = new Date(1_700_000_000_000);
    startLoad = hls.startLoad;
    recoverMediaError = hls.recoverMediaError;
    loadSource = hls.loadSource;
    attachMedia = hls.attachMedia;
    destroy = hls.destroy;
    constructor() {
      hls.instance = this as never;
    }
    on(event: string, handler: (event: string, data: unknown) => void) {
      hls.handlers.set(event, handler);
    }
  },
}));

const listeners = fakeLiveListeners;

function video() {
  const element = document.createElement('video');
  element.play = vi.fn(async () => undefined);
  element.load = vi.fn();
  element.pause = vi.fn();
  Object.defineProperty(element, 'readyState', { value: 4, configurable: true });
  return element;
}

const opened = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  hls.supported = true;
  hls.handlers.clear();
  for (const fn of [
    hls.startLoad,
    hls.recoverMediaError,
    hls.loadSource,
    hls.attachMedia,
    hls.destroy,
  ])
    fn.mockClear();
});
afterEach(() => vi.useRealTimers());

describe('the browser live engine', () => {
  it('opens the playlist on hls.js and reads the frame time from the programme date', async () => {
    const element = video();
    const engine = new HtmlLiveEngine(element, 'http://s/live.m3u8?t=x', listeners());
    await opened();

    expect(hls.loadSource).toHaveBeenCalledWith('http://s/live.m3u8?t=x');
    expect(hls.attachMedia).toHaveBeenCalledWith(element);
    expect(engine.frameTimeMs()).toBe(1_700_000_000_000);
    engine.destroy();
  });

  it('is behind the live edge only when well behind it, and goes back to it', async () => {
    const element = video();
    const engine = new HtmlLiveEngine(element, 'u', listeners());
    await opened();

    element.currentTime = 99;
    expect(engine.isBehindLive()).toBe(false);
    element.currentTime = 80;
    expect(engine.isBehindLive()).toBe(true);
    engine.goLive();
    expect(element.currentTime).toBe(100);
    engine.destroy();
  });

  it('retries the network, then asks to be rebuilt once the playlist is gone', async () => {
    const l = listeners();
    const engine = new HtmlLiveEngine(video(), 'u', l);
    await opened();
    const fail = hls.handlers.get('hlsError');

    fail?.('hlsError', { fatal: true, type: 'networkError' });
    expect(hls.startLoad).toHaveBeenCalledTimes(1);

    vi.useFakeTimers();
    fail?.('hlsError', { fatal: true, type: 'networkError', response: { code: 404 } });
    vi.advanceTimersByTime(2000);
    expect(l.onLost).toHaveBeenCalledTimes(1);
    engine.destroy();
  });

  it('recovers a media error in place and gives up on one it cannot name', async () => {
    const l = listeners();
    const engine = new HtmlLiveEngine(video(), 'u', l);
    await opened();
    const fail = hls.handlers.get('hlsError');

    fail?.('hlsError', { fatal: true, type: 'mediaError' });
    fail?.('hlsError', { fatal: true, type: 'otherError' });

    expect(hls.recoverMediaError).toHaveBeenCalledTimes(1);
    expect(l.onError).toHaveBeenCalledTimes(1);
    engine.destroy();
  });

  it('ignores a non-fatal error', async () => {
    const l = listeners();
    const engine = new HtmlLiveEngine(video(), 'u', l);
    await opened();

    hls.handlers.get('hlsError')?.('hlsError', { fatal: false, type: 'networkError' });

    expect(hls.startLoad).not.toHaveBeenCalled();
    engine.destroy();
  });

  it('pushes a picture stalled for four seconds back to the live edge', async () => {
    vi.useFakeTimers();
    const element = video();
    Object.defineProperty(element, 'readyState', { value: 1, configurable: true });
    Object.defineProperty(element, 'paused', { value: false, configurable: true });
    const engine = new HtmlLiveEngine(element, 'u', listeners());
    await vi.advanceTimersByTimeAsync(0);

    await vi.advanceTimersByTimeAsync(5000);

    expect(hls.startLoad).toHaveBeenCalled();
    expect(element.currentTime).toBe(100);
    engine.destroy();
  });

  it('reports an error when the browser has neither MSE nor native HLS', async () => {
    hls.supported = false;
    const l = listeners();
    const engine = new HtmlLiveEngine(video(), 'u', l);
    await opened();

    expect(l.onError).toHaveBeenCalledTimes(1);
    engine.destroy();
  });

  it('plays natively where there is no MSE but the browser speaks HLS', async () => {
    hls.supported = false;
    const element = video();
    element.canPlayType = () => 'maybe';
    const engine = new HtmlLiveEngine(element, 'http://s/n.m3u8', listeners());
    await opened();

    expect(element.src).toBe('http://s/n.m3u8');
    expect(hls.loadSource).not.toHaveBeenCalled();
    engine.destroy();
  });

  it('lets go of the stream when destroyed', async () => {
    const element = video();
    const engine = new HtmlLiveEngine(element, 'u', listeners());
    await opened();

    engine.destroy();

    expect(hls.destroy).toHaveBeenCalled();
    expect(element.getAttribute('src')).toBeNull();
  });
});
