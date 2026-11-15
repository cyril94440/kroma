import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LiveListeners } from '#tv/features/live/engine/liveEngine';
import { ExpoLiveEngine } from './ExpoLiveEngine';

const { players, FakePlayer } = vi.hoisted(() => {
  class FakePlayer {
    source: { uri: string; contentType?: string };
    currentTime = 0;
    bufferedPosition = 0;
    currentLiveTimestamp: number | null = null;
    currentOffsetFromLive: number | null = null;
    playing = false;
    playbackRate = 1;
    released = false;
    seeks: number[] = [];
    handlers = new Map<string, ((payload: never) => void)[]>();

    constructor(source: { uri: string; contentType?: string }) {
      this.source = source;
    }
    play(): void {
      this.playing = true;
    }
    pause(): void {
      this.playing = false;
    }
    release(): void {
      this.released = true;
    }
    seekBy(seconds: number): void {
      this.seeks.push(seconds);
    }
    addListener(event: string, handler: (payload: never) => void): { remove(): void } {
      this.handlers.set(event, [...(this.handlers.get(event) ?? []), handler]);
      return {
        remove: () =>
          this.handlers.set(
            event,
            (this.handlers.get(event) ?? []).filter((h) => h !== handler),
          ),
      };
    }
    emit(event: string, payload: unknown = {}): void {
      for (const handler of this.handlers.get(event) ?? []) handler(payload as never);
    }
  }
  return { players: [] as FakePlayer[], FakePlayer };
});

vi.mock('expo-video', () => ({
  createVideoPlayer: (source: { uri: string; contentType?: string }) => {
    const player = new FakePlayer(source);
    players.push(player);
    return player;
  },
}));

function listeners(): LiveListeners & Record<keyof LiveListeners, ReturnType<typeof vi.fn>> {
  return {
    onPlay: vi.fn(),
    onPause: vi.fn(),
    onWaiting: vi.fn(),
    onPlaying: vi.fn(),
    onReady: vi.fn(),
    onError: vi.fn(),
    onLost: vi.fn(),
    onAspect: vi.fn(),
    onSurfaceChange: vi.fn(),
  };
}

beforeEach(() => {
  players.length = 0;
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

describe('the native live engine', () => {
  it('opens the playlist as HLS and tells the surface a player exists', () => {
    const l = listeners();
    new ExpoLiveEngine('http://s/live.m3u8', l);

    expect(players[0]?.source).toEqual({ uri: 'http://s/live.m3u8', contentType: 'hls' });
    expect(l.onSurfaceChange).toHaveBeenCalled();
  });

  it('reports ready when the player is, and the frame time from the programme date', () => {
    const l = listeners();
    const engine = new ExpoLiveEngine('u', l);
    const player = players[0];
    if (!player) throw new Error('no player');
    player.currentLiveTimestamp = 1_700_000_000_000;

    player.emit('statusChange', { status: 'readyToPlay' });

    expect(l.onReady).toHaveBeenCalled();
    expect(engine.isReady()).toBe(true);
    expect(engine.frameTimeMs()).toBe(1_700_000_000_000);
  });

  it('is behind the live edge past six seconds and seeks forward by the offset to return', () => {
    const engine = new ExpoLiveEngine('u', listeners());
    const player = players[0];
    if (!player) throw new Error('no player');

    player.currentOffsetFromLive = 3;
    expect(engine.isBehindLive()).toBe(false);
    player.currentOffsetFromLive = 12;
    expect(engine.isBehindLive()).toBe(true);
    engine.goLive();

    expect(player.seeks).toEqual([12]);
  });

  it('knows nothing of the edge when the stream carries no programme date', () => {
    const engine = new ExpoLiveEngine('u', listeners());

    expect(engine.isBehindLive()).toBe(false);
    expect(engine.frameTimeMs()).toBeNull();
    engine.goLive();
    expect(players[0]?.seeks).toEqual([]);
  });

  it('rebuilds after a load that outlasts the stall budget', () => {
    const l = listeners();
    new ExpoLiveEngine('u', l);

    players[0]?.emit('statusChange', { status: 'loading' });
    vi.advanceTimersByTime(5000);

    expect(l.onLost).toHaveBeenCalledTimes(1);
  });

  it('rebuilds on an error three times, then reports it', () => {
    const l = listeners();
    new ExpoLiveEngine('u', l);

    for (let i = 0; i < 4; i += 1) players[0]?.emit('statusChange', { status: 'error' });

    expect(l.onLost).toHaveBeenCalledTimes(3);
    expect(l.onError).toHaveBeenCalledTimes(1);
  });

  it('moves relative to where it is and releases the player when destroyed', () => {
    const engine = new ExpoLiveEngine('u', listeners());
    engine.seekByMs(-2500);
    engine.destroy();

    expect(players[0]?.seeks).toEqual([-2.5]);
    expect(players[0]?.released).toBe(true);
  });
});
