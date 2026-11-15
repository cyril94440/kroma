import { BEHIND_LIVE_SECONDS, canRebuild, STALL_RECOVERY_MS } from '@kromatv/core';
import { useVideoPlayer, type VideoPlayer } from 'expo-video';
import { useCallback, useEffect, useRef, useState } from 'react';
import { stopNativePlayer } from '#mobile/player/engine/shutdown';
import { usePlaybackOwner } from '#mobile/player/playback-owner';

const WATCHDOG_MS = 500;

interface LiveEngine {
  player: VideoPlayer;
  playing: boolean;
  waiting: boolean;
  failed: boolean;
  behind: boolean;
  frameTimeMs(): number | null;
  goLive(): void;
  togglePlay(): void;
  shutdown(): void;
}

/** The phone's live player over expo-video (AVPlayer, Media3): one stream, no
 *  seek, a way back to the live edge. It takes the app's single playback lease,
 *  rebuilds the source when the load outlasts the stall budget or errors (up to
 *  three times), and reads the frame time from the playlist's programme date. */
export function useLiveEngine(url: string | null): LiveEngine {
  const active = useRef(true);
  const rebuilds = useRef(0);
  const loadingSince = useRef<number | null>(null);
  const [playing, setPlaying] = useState(false);
  const [waiting, setWaiting] = useState(true);
  const [failed, setFailed] = useState(false);
  const [behind, setBehind] = useState(false);
  const [attempt, setAttempt] = useState(0);

  const player = useVideoPlayer(null, (p) => {
    p.staysActiveInBackground = false;
    p.showNowPlayingNotification = false;
  });

  const shutdown = useCallback(() => {
    if (!active.current) return;
    active.current = false;
    stopNativePlayer(player);
  }, [player]);
  usePlaybackOwner(shutdown);

  const rebuild = useCallback(() => {
    if (!canRebuild(rebuilds.current)) {
      setFailed(true);
      setWaiting(false);
      return;
    }
    rebuilds.current += 1;
    setAttempt((n) => n + 1);
  }, []);

  useEffect(() => {
    if (!url) return;
    player
      .replaceAsync({ uri: url, contentType: 'hls' })
      .then(() => {
        if (active.current) player.play();
      })
      .catch(rebuild);
  }, [player, url, attempt, rebuild]);

  useEffect(() => {
    const subscriptions = [
      player.addListener('playingChange', ({ isPlaying }) => setPlaying(isPlaying)),
      player.addListener('statusChange', ({ status }) => {
        if (status === 'loading') {
          loadingSince.current ??= Date.now();
          setWaiting(true);
          return;
        }
        loadingSince.current = null;
        if (status === 'error') rebuild();
        if (status === 'readyToPlay') {
          rebuilds.current = 0;
          setWaiting(false);
          setFailed(false);
        }
      }),
    ];
    const timer = setInterval(() => {
      const offset = player.currentOffsetFromLive;
      setBehind(offset !== null && offset > BEHIND_LIVE_SECONDS);
      const since = loadingSince.current;
      if (since !== null && Date.now() - since > STALL_RECOVERY_MS) {
        loadingSince.current = null;
        rebuild();
      }
    }, WATCHDOG_MS);
    return () => {
      clearInterval(timer);
      for (const subscription of subscriptions) subscription.remove();
    };
  }, [player, rebuild]);

  const goLive = useCallback(() => {
    const offset = player.currentOffsetFromLive;
    if (offset !== null && offset > 0) player.seekBy(offset);
    player.play();
  }, [player]);

  const togglePlay = useCallback(() => {
    if (player.playing) player.pause();
    else player.play();
  }, [player]);

  const frameTimeMs = useCallback(() => player.currentLiveTimestamp, [player]);

  return { player, playing, waiting, failed, behind, frameTimeMs, goLive, togglePlay, shutdown };
}
