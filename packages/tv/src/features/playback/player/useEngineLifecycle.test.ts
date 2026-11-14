// @vitest-environment jsdom

import type { MediaItem } from '@kromatv/client/media';
import { fakeClient } from '@kromatv/client/test';
import { act, renderHook, waitFor } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PlaybackCapabilitiesProvider } from '#tv/app/providers/playbackCapabilities';
import type { EngineListeners } from '#tv/features/playback/player/engine';

const H = vi.hoisted(() => ({
  stalled: false,
  vlc: false,
  listeners: null as EngineListeners | null,
  engine: {
    play: vi.fn(),
    destroy: vi.fn(),
    setAudioRendition: vi.fn(),
    audioFilterSupported: () => true,
  },
}));

vi.mock('#tv/features/playback/player/useStallGuard', () => ({
  useStallGuard: () => H.stalled,
}));

vi.mock('#tv/features/playback/player/playbackBackend', () => ({
  usePlaybackBackend: () => BACKEND,
}));

const BACKEND = {
  planEngine: () => ({
    eng: 'video',
    surface: 'video',
    playbackMode: 'direct',
    deviceLabel: 'a television',
    rebuildKey: 'video:true',
  }),
  createTvEngine: (args: { listeners: EngineListeners }) => {
    H.listeners = args.listeners;
    return H.engine;
  },
};

vi.mock('#tv/features/playback/player/useResolvedStart', () => ({
  useResolvedStart: () => ({ startSec: 0, resumedAt: null, setStartSec: vi.fn() }),
}));

vi.mock('#tv/features/playback/player/vlcPlane', () => ({ vlcAvailable: () => H.vlc }));

const { useEngineLifecycle } = await import('#tv/features/playback/player/useEngineLifecycle');

const CLIENT = fakeClient({ media: { streamUrl: () => '/api/items/vid1/stream' } });
const ITEM = { id: 'vid1', durationMs: 7_200_000 } as unknown as MediaItem;

function open() {
  const view = renderHook(() => useEngineLifecycle(CLIENT, ITEM));
  if (!H.listeners) throw new Error('expected the lifecycle to have built an engine');
  return { ...view, on: H.listeners };
}

describe('useEngineLifecycle stall guard', () => {
  beforeEach(() => {
    H.stalled = false;
    H.listeners = null;
  });

  it('shows a stuck playhead as buffering rather than as a healthy bar', async () => {
    H.stalled = true;
    const { result, on } = open();

    act(() => on.onPlay());

    await waitFor(() => expect(result.current.waiting).toBe(true));
  });

  it('leaves the buffering flag alone while nothing is stuck', async () => {
    const { result, on } = open();

    act(() => on.onPlay());

    await waitFor(() => expect(result.current.waiting).toBe(false));
    expect(result.current.playing).toBe(true);
  });

  it('arms the guard only once the engine is ready and has not failed', async () => {
    const { result, on } = open();

    act(() => on.onReady());

    await waitFor(() => expect(result.current.ready).toBe(true));
    expect(result.current.error).toBeNull();
  });
});

describe('useEngineLifecycle soundtrack', () => {
  beforeEach(() => {
    H.listeners = null;
  });

  it('asks the engine again when the track already playing is picked again', () => {
    const { result } = open();

    H.engine.setAudioRendition.mockClear();

    act(() => result.current.pickAudio(result.current.audioIndex));

    expect(H.engine.setAudioRendition).toHaveBeenCalledTimes(1);
  });
});

describe('useEngineLifecycle opening track', () => {
  it('plans the engine for the track the profile language picks, not the file default', () => {
    const planEngine = vi.spyOn(BACKEND, 'planEngine') as unknown as {
      mock: { calls: unknown[][] };
      mockRestore(): void;
    };
    const dubbed = {
      id: 'vid2',
      durationMs: 1000,
      audioTracks: [
        { index: 0, codec: 'eac3', language: 'eng', default: true },
        { index: 3, codec: 'dts', language: 'fra' },
      ],
    } as unknown as MediaItem;

    renderHook(() => useEngineLifecycle(CLIENT, dubbed, 'fr'));

    expect(planEngine.mock.calls.at(-1)?.[4]).toBe(3);
    planEngine.mockRestore();
  });
});

describe('useEngineLifecycle decoder ceiling', () => {
  const UHD = {
    id: 'uhd1',
    durationMs: 3_000_000,
    video: { codec: 'hevc', width: 3840, height: 2160, bitDepth: 10 },
    audioTracks: [],
  } as unknown as MediaItem;

  beforeEach(() => {
    H.listeners = null;
    H.vlc = true;
  });

  afterEach(() => {
    H.vlc = false;
  });

  it('still builds an engine for a picture over the ceiling, because the server scales it', async () => {
    const { result } = renderHook(() => useEngineLifecycle(CLIENT, UHD), {
      wrapper: ({ children }: { children: ReactNode }) =>
        createElement(PlaybackCapabilitiesProvider, {
          limits: { hevc: { width: 1920, height: 1920 } },
          children,
        }),
    });

    await waitFor(() => expect(H.listeners).not.toBeNull());
    expect(result.current.error).toBeNull();
  });

  it('does not fall back to VLC when the frame is what the platform player refused', async () => {
    const { result } = renderHook(() => useEngineLifecycle(CLIENT, UHD), {
      wrapper: ({ children }: { children: ReactNode }) =>
        createElement(PlaybackCapabilitiesProvider, {
          limits: { hevc: { width: 1920, height: 1920 } },
          children,
        }),
    });
    if (!H.listeners) throw new Error('expected the lifecycle to have built an engine');

    act(() => H.listeners?.onError());

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.surface).not.toBe('vlc');
  });
});

describe('useEngineLifecycle source failure', () => {
  beforeEach(() => {
    H.listeners = null;
    H.vlc = false;
  });

  afterEach(() => vi.unstubAllGlobals());

  it('tells the viewer why the source of a title cannot be read', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ error: 'x', code: 'error.sourceRefused' }), {
          status: 502,
        }),
      ),
    );
    const { result, on } = open();

    act(() => on.onError());

    await waitFor(() => expect(result.current.error?.messageKey).toBe('error.sourceRefused'));
    expect(result.current.error?.hintKey).toBe('player.sourceFailedHint');
  });

  it('keeps the generic message when the server gives no reason', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('', { status: 206 })));
    const { result, on } = open();

    act(() => on.onError());

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error?.messageKey).toBe('player.cantPlay');
  });
});
