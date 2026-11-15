import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createPlayerHandle, SNAPSHOT_EVERY_MS } from './player-handle';
import { programmeTime } from './video-control';

function range(start: number, end: number) {
  return { length: 1, start: () => start, end: () => end };
}

function fakeElement(init: Partial<Record<string, unknown>> = {}) {
  const listeners = new Map<string, Set<() => void>>();
  const element = {
    currentTime: 100,
    paused: false,
    playbackRate: 1,
    readyState: 4,
    seekable: range(60, 120),
    buffered: range(60, 108),
    addEventListener: (name: string, fn: () => void) => {
      const set = listeners.get(name) ?? new Set();
      set.add(fn);
      listeners.set(name, set);
    },
    removeEventListener: (name: string, fn: () => void) => listeners.get(name)?.delete(fn),
    fire: (name: string) => listeners.get(name)?.forEach((fn) => fn()),
    listenerCount: () => [...listeners.values()].reduce((n, set) => n + set.size, 0),
    ...init,
  };
  return element;
}

function build(element = fakeElement(), playingDate: number | null = 1_700_000_000_000) {
  const source = {
    element: element as unknown as HTMLVideoElement,
    playingDate: () => playingDate,
    liveSyncPosition: () => 114 as number | null,
    channelId: () => 'c1' as string | null,
  };
  return { element, source, handle: createPlayerHandle(source) };
}

describe('createPlayerHandle', () => {
  it('reports the programme date time of the frame on screen, null when unknown', () => {
    expect(build().handle.getPositionMs()).toBe(1_700_000_000_000);
    expect(build(fakeElement(), null).handle.getPositionMs()).toBeNull();
  });

  it('seeks to the live edge the player computed and ignores an unknown edge', () => {
    const { element, source, handle } = build();
    handle.seekToLiveEdge();
    expect(element.currentTime).toBe(114);

    element.currentTime = 100;
    source.liveSyncPosition = () => null;
    handle.seekToLiveEdge();
    expect(element.currentTime).toBe(100);
  });

  it('seeks by a signed delta in milliseconds and never before zero', () => {
    const { element, handle } = build();
    handle.seekByMs(-2500);
    expect(element.currentTime).toBe(97.5);
    handle.seekByMs(-1e9);
    expect(element.currentTime).toBe(0);
  });

  it('sets the playback rate', () => {
    const { element, handle } = build();
    handle.setPlaybackRate(1.05);
    expect(element.playbackRate).toBe(1.05);
  });

  it('says how far a seek may go inside the safe zone of the window', () => {
    const { handle } = build();
    expect(handle.getMoveRange()).toEqual({ backMs: 39500, forwardMs: 16000 });
    expect(build(fakeElement({ seekable: { length: 0 } })).handle.getMoveRange()).toBeNull();
  });

  it('reports a negative forward range when already past the safe zone', () => {
    const { handle } = build(fakeElement({ currentTime: 118 }));
    expect(handle.getMoveRange()?.forwardMs).toBe(-2000);
  });

  it('measures the buffer ahead and readiness', () => {
    const { element, handle } = build();
    expect(handle.getBufferAheadMs()).toBe(8000);
    expect(handle.isReady()).toBe(true);
    element.readyState = 2;
    expect(handle.isReady()).toBe(false);
    expect(build(fakeElement({ buffered: { length: 0 } })).handle.getBufferAheadMs()).toBe(0);
  });
});

describe('PlayerHandle.subscribe', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('emits a snapshot on a timer and on play, pause and seek', () => {
    const { element, handle } = build();
    const seen: { playing: boolean }[] = [];

    handle.subscribe((snapshot) => seen.push(snapshot));
    vi.advanceTimersByTime(SNAPSHOT_EVERY_MS * 2);
    element.paused = true;
    element.fire('pause');
    element.fire('seeked');

    expect(seen.map((s) => s.playing)).toEqual([true, true, false, false]);
  });

  it('carries the channel and position and stops on unsubscribe', () => {
    const { element, handle } = build();
    const listener = vi.fn();

    const stop = handle.subscribe(listener);
    vi.advanceTimersByTime(SNAPSHOT_EVERY_MS);
    expect(listener).toHaveBeenCalledWith({
      positionMs: 1_700_000_000_000,
      playing: true,
      channelId: 'c1',
    });
    stop();
    vi.advanceTimersByTime(SNAPSHOT_EVERY_MS * 4);
    expect(listener).toHaveBeenCalledTimes(1);
    expect(element.listenerCount()).toBe(0);
  });
});

describe('programmeTime', () => {
  const element = (extra: object) => ({ currentTime: 10, ...extra }) as unknown as HTMLVideoElement;

  it('prefers the date hls.js reports for the frame on screen', () => {
    expect(programmeTime(element({}), new Date(5000))).toBe(5000);
  });

  it('falls back to a native player start date plus the playhead', () => {
    expect(programmeTime(element({ getStartDate: () => new Date(1000) }), null)).toBe(11_000);
  });

  it('is null with neither, or with an invalid start date', () => {
    expect(programmeTime(element({}), null)).toBeNull();
    expect(programmeTime(element({ getStartDate: () => new Date(Number.NaN) }), null)).toBeNull();
  });
});
