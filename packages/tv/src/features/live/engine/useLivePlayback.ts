import { type RefObject, useCallback, useEffect, useRef, useState } from 'react';
import type { LiveBackend, LiveEngine, LiveListeners } from '#tv/features/live/engine/liveEngine';

export interface LiveState {
  playing: boolean;
  waiting: boolean;
  ready: boolean;
  failed: boolean;
  behind: boolean;
  aspect: number | undefined;
}

export interface LivePlayback<E extends LiveEngine> extends LiveState {
  engineRef: RefObject<E | null>;
  surfaceNonce: number;
  attachVideo: (video: HTMLVideoElement | null) => void;
  togglePlay: () => void;
  goLive: () => void;
}

const START: LiveState = {
  playing: false,
  waiting: true,
  ready: false,
  failed: false,
  behind: false,
  aspect: undefined,
};

const BEHIND_POLL_MS = 500;

/** Plays the live playlist `url` through the backend's engine and starts it the
 *  first time the engine is ready. A null `url` holds the screen waiting with no
 *  engine at all, which is how a stream that has not started yet looks. The
 *  engine asking to be rebuilt (the playlist moved under a new folder) builds a
 *  fresh one on the same URL. */
export function useLivePlayback<E extends LiveEngine>(
  backend: LiveBackend<E>,
  url: string | null,
): LivePlayback<E> {
  const engineRef = useRef<E | null>(null);
  const [video, setVideo] = useState<HTMLVideoElement | null>(null);
  const [state, setState] = useState<LiveState>(START);
  const [surfaceNonce, setSurfaceNonce] = useState(0);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    if (!url) return;
    let started = false;
    const patch = (next: Partial<LiveState>) => setState((s) => ({ ...s, ...next }));
    const listeners: LiveListeners = {
      onPlay: () => patch({ playing: true, waiting: false }),
      onPause: () => patch({ playing: false }),
      onWaiting: () => patch({ waiting: true }),
      onPlaying: () => patch({ waiting: false, failed: false }),
      onError: () => patch({ failed: true, waiting: false }),
      onLost: () => setAttempt((n) => n + 1),
      onAspect: (aspect) =>
        patch({ aspect: aspect > 0 && Number.isFinite(aspect) ? aspect : undefined }),
      onSurfaceChange: () => setSurfaceNonce((n) => n + 1),
      onReady: () => {
        patch({ ready: true, waiting: false });
        if (started) return;
        started = true;
        engineRef.current?.play();
      },
    };
    const engine = backend.createLiveEngine({ url, video, listeners });
    if (!engine) return;
    engineRef.current = engine;
    return () => {
      engineRef.current = null;
      engine.destroy();
      setState(START);
    };
  }, [backend, url, video, attempt]);

  useEffect(() => {
    if (!url) return;
    const timer = setInterval(() => {
      const behind = engineRef.current?.isBehindLive() ?? false;
      setState((s) => (s.behind === behind ? s : { ...s, behind }));
    }, BEHIND_POLL_MS);
    return () => clearInterval(timer);
  }, [url]);

  const togglePlay = useCallback(() => {
    const engine = engineRef.current;
    if (!engine) return;
    if (engine.isPaused()) engine.play();
    else engine.pause();
  }, []);

  const goLive = useCallback(() => {
    engineRef.current?.goLive();
    engineRef.current?.play();
  }, []);

  return { ...state, engineRef, surfaceNonce, attachVideo: setVideo, togglePlay, goLive };
}
