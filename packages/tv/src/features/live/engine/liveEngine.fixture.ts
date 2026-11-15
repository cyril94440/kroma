import { vi } from 'vitest';

export function fakeLiveListeners() {
  return {
    onPlay: vi.fn<() => void>(),
    onPause: vi.fn<() => void>(),
    onWaiting: vi.fn<() => void>(),
    onPlaying: vi.fn<() => void>(),
    onReady: vi.fn<() => void>(),
    onError: vi.fn<() => void>(),
    onLost: vi.fn<() => void>(),
    onAspect: vi.fn<(aspect: number) => void>(),
    onSurfaceChange: vi.fn<() => void>(),
  };
}
