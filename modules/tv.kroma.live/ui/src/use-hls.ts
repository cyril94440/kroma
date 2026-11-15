import Hls from 'hls.js';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { box } from './box';
import { createPlayerHandle, type PlayerHandle } from './player-handle';
import {
  HLS_CONFIG,
  isBehindLive,
  type FatalKind,
  type Recovery,
  recoveryFor,
  STALL_RECOVERY_MS,
  wantsNative,
} from './player-logic';
import * as control from './video-control';

const RELOAD_DELAY_MS = 1500;

interface HlsState {
  behind: boolean;
  playing: boolean;
  blocked: boolean;
  failed: boolean;
  goLive: () => void;
  toggle: () => void;
  handle: PlayerHandle | null;
}

function kindOf(type: string): FatalKind {
  if (type === Hls.ErrorTypes.NETWORK_ERROR) return 'network';
  if (type === Hls.ErrorTypes.MEDIA_ERROR) return 'media';
  return 'other';
}

export function useHls(
  element: HTMLVideoElement | null,
  url: string | null,
  onLost: () => void,
  channelId: string | null = null,
): HlsState {
  const [behind, setBehind] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [blocked, setBlocked] = useState(false);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [live] = useState(() => box<Hls | null>(null));
  const lost = useRef(onLost);
  const [channel] = useState(() => box(channelId));
  useEffect(() => {
    lost.current = onLost;
    channel.set(channelId);
  });
  const handle = useMemo(
    () =>
      element
        ? createPlayerHandle({
            element,
            playingDate: () => control.programmeTime(element, live.get()?.playingDate ?? null),
            liveSyncPosition: () => live.get()?.liveSyncPosition ?? null,
            channelId: () => channel.get(),
          })
        : null,
    [element],
  );

  useEffect(() => {
    if (!element || !url) return;
    const attempts = { network: 0, media: 0 };
    const start = () =>
      control.play(element).then(
        () => setBlocked(false),
        () => setBlocked(true),
      );
    if (wantsNative({ mse: Hls.isSupported(), nativeHls: control.supportsNativeHls(element) })) {
      const detach = control.attachNative(element, url);
      void start();
      return detach;
    }
    const hls = new Hls({ ...HLS_CONFIG });
    live.set(hls);
    hls.attachMedia(element);
    hls.loadSource(url);
    hls.on(Hls.Events.MANIFEST_PARSED, () => void start());
    hls.on(Hls.Events.FRAG_LOADED, () => {
      attempts.network = 0;
    });
    hls.on(Hls.Events.ERROR, (_event, data) => {
      if (!data.fatal) return;
      const kind = kindOf(data.type);
      const tries = kind === 'media' ? attempts.media++ : attempts.network++;
      const action: Recovery = recoveryFor(kind, tries, data.response?.code);
      if (action === 'startLoad') hls.startLoad();
      else if (action === 'recoverMedia') hls.recoverMediaError();
      else if (action === 'reload') {
        setTimeout(() => {
          lost.current();
          setAttempt((n) => n + 1);
        }, RELOAD_DELAY_MS);
      } else setFailed(true);
    });
    return () => {
      hls.destroy();
      live.set(null);
    };
  }, [element, url, attempt]);

  useEffect(() => {
    if (!element) return;
    let stalledAt: number | null = null;
    const timer = setInterval(() => {
      const hls = live.get();
      setBehind(isBehindLive(control.position(element), hls?.liveSyncPosition ?? null));
      if (!control.isStalled(element)) {
        stalledAt = null;
        return;
      }
      stalledAt ??= Date.now();
      if (Date.now() - stalledAt > STALL_RECOVERY_MS && hls) {
        stalledAt = null;
        hls.startLoad();
        if (hls.liveSyncPosition) control.seek(element, hls.liveSyncPosition);
      }
    }, 500);
    const sync = () => setPlaying(!control.isPaused(element));
    element.addEventListener('play', sync);
    element.addEventListener('pause', sync);
    return () => {
      clearInterval(timer);
      element.removeEventListener('play', sync);
      element.removeEventListener('pause', sync);
    };
  }, [element, url]);

  const goLive = useCallback(() => {
    if (!element) return;
    const edge = live.get()?.liveSyncPosition;
    if (edge) control.seek(element, edge);
    void control.play(element);
  }, [element]);

  const toggle = useCallback(() => {
    if (!element) return;
    if (control.isPaused(element)) void control.play(element);
    else control.pause(element);
  }, [element]);

  return { behind, playing, blocked, failed, goLive, toggle, handle };
}
