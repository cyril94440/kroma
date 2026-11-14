import Hls from 'hls.js';
import { type RefObject, useCallback, useEffect, useRef, useState } from 'react';
import {
  HLS_CONFIG,
  isBehindLive,
  type Recovery,
  recoveryFor,
  STALL_RECOVERY_MS,
  wantsNative,
} from './player-logic';

export interface HlsState {
  behind: boolean;
  playing: boolean;
  blocked: boolean;
  failed: boolean;
  goLive: () => void;
  toggle: () => void;
}

export function useHls(video: RefObject<HTMLVideoElement | null>, url: string | null, onLost: () => void): HlsState {
  const [behind, setBehind] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [blocked, setBlocked] = useState(false);
  const [failed, setFailed] = useState(false);
  const hlsRef = useRef<Hls | null>(null);
  const lost = useRef(onLost);
  useEffect(() => {
    lost.current = onLost;
  });

  useEffect(() => {
    const element = video.current;
    if (!element || !url) return;
    setFailed(false);
    const attempts = { network: 0, media: 0 };
    const start = () =>
      element.play().then(
        () => setBlocked(false),
        () => setBlocked(true),
      );
    const native = wantsNative({
      mse: Hls.isSupported(),
      nativeHls: element.canPlayType('application/vnd.apple.mpegurl') !== '',
    });
    if (native) {
      element.src = url;
      void start();
      return () => element.removeAttribute('src');
    }
    const hls = new Hls({ ...HLS_CONFIG });
    hlsRef.current = hls;
    hls.attachMedia(element);
    hls.loadSource(url);
    hls.on(Hls.Events.MANIFEST_PARSED, () => void start());
    hls.on(Hls.Events.FRAG_LOADED, () => {
      attempts.network = 0;
    });
    hls.on(Hls.Events.ERROR, (_event, data) => {
      if (!data.fatal) return;
      const kind = data.type === Hls.ErrorTypes.NETWORK_ERROR ? 'network' : data.type === Hls.ErrorTypes.MEDIA_ERROR ? 'media' : 'other';
      const tries = kind === 'media' ? attempts.media++ : attempts.network++;
      const action: Recovery = recoveryFor(kind, tries, data.response?.code);
      if (action === 'startLoad') hls.startLoad();
      else if (action === 'recoverMedia') hls.recoverMediaError();
      else if (action === 'reload') lost.current();
      else setFailed(true);
    });
    return () => {
      hls.destroy();
      hlsRef.current = null;
    };
  }, [video, url]);

  useEffect(() => {
    const element = video.current;
    if (!element) return;
    let stalledAt: number | null = null;
    const timer = setInterval(() => {
      const hls = hlsRef.current;
      setBehind(isBehindLive(element.currentTime, hls?.liveSyncPosition ?? null));
      if (element.readyState < 3 && !element.paused) {
        stalledAt ??= Date.now();
        if (Date.now() - stalledAt > STALL_RECOVERY_MS && hls) {
          stalledAt = null;
          hls.startLoad();
          if (hls.liveSyncPosition) element.currentTime = hls.liveSyncPosition;
        }
      } else stalledAt = null;
    }, 500);
    const sync = () => setPlaying(!element.paused);
    element.addEventListener('play', sync);
    element.addEventListener('pause', sync);
    return () => {
      clearInterval(timer);
      element.removeEventListener('play', sync);
      element.removeEventListener('pause', sync);
    };
  }, [video, url]);

  const goLive = useCallback(() => {
    const element = video.current;
    const position = hlsRef.current?.liveSyncPosition;
    if (element && position) element.currentTime = position;
    void element?.play();
  }, [video]);

  const toggle = useCallback(() => {
    const element = video.current;
    if (!element) return;
    if (element.paused) void element.play();
    else element.pause();
  }, [video]);

  return { behind, playing, blocked, failed, goLive, toggle };
}
