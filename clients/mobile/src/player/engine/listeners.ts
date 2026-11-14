import type { AudioTrack, VideoPlayer } from 'expo-video';
import { type Dispatch, type SetStateAction, useEffect } from 'react';
import { type EngineCore, nativeTrackKey, patchCore } from './types';

interface ListenerInput {
  player: VideoPlayer;
  core: EngineCore;
  fail: (error?: unknown) => void;
  localUri: string | undefined;
  clearFailure: () => void;
  setCur: Dispatch<SetStateAction<number>>;
  setBuffered: Dispatch<SetStateAction<number>>;
  setDur: Dispatch<SetStateAction<number>>;
  setPlaying: Dispatch<SetStateAction<boolean>>;
  setWaiting: Dispatch<SetStateAction<boolean>>;
  setReady: Dispatch<SetStateAction<boolean>>;
  setEndedNonce: Dispatch<SetStateAction<number>>;
  setLocalAudio: Dispatch<SetStateAction<AudioTrack[]>>;
  setAudioIndex: Dispatch<SetStateAction<number>>;
}

export function useEngineListeners(input: ListenerInput): void {
  const {
    player,
    core,
    fail,
    localUri,
    clearFailure,
    setCur,
    setBuffered,
    setDur,
    setPlaying,
    setWaiting,
    setReady,
    setEndedNonce,
    setLocalAudio,
    setAudioIndex,
  } = input;
  useEffect(() => {
    const subs = [
      player.addListener('statusChange', ({ status, error }) => {
        if (!core.active || core.terminal) return;
        if (status === 'error') {
          fail(error);
          return;
        }
        if (status === 'loading') setWaiting(true);
        if (status === 'readyToPlay') {
          setReady(true);
          setWaiting(false);
          clearFailure();
          // Local files: surface the tracks actually in the file (iOS often
          // has them ready here without ever firing the change event).
          if (localUri) setLocalAudio(player.availableAudioTracks);
          if (core.pendingSeek != null) {
            player.currentTime = core.pendingSeek;
            patchCore(core, { elSec: core.pendingSeek, pendingSeek: null });
          }
          if (core.mode === 'direct' && player.duration > 0) setDur(player.duration);
          if (core.resumeOnLoad) {
            patchCore(core, { resumeOnLoad: false });
            player.play();
          }
        }
      }),
      player.addListener('timeUpdate', ({ currentTime, bufferedPosition }) => {
        if (!core.active || core.terminal) return;
        const advanced = currentTime - core.elSec;
        if (player.playing && advanced > 0 && advanced < 2) setWaiting(false);
        if (currentTime > 0.5) patchCore(core, { started: true });
        patchCore(core, { elSec: currentTime, bufSec: Math.max(0, bufferedPosition) });
        setCur(core.baseSec + currentTime);
        setBuffered(core.baseSec + Math.max(currentTime, core.bufSec));
      }),
      player.addListener('playingChange', ({ isPlaying }) => {
        if (!core.active || core.terminal) return;
        setPlaying(isPlaying);
      }),
      // ExoPlayer reports ENDED for the empty initial source and again on
      // every source swap: a bare event is not proof playback reached the
      // end. Only trust it once this source has produced playback time and
      // the clock sits near the actual end.
      player.addListener('playToEnd', () => {
        if (!core.active || core.terminal) return;
        if (!core.started) return;
        const end = Math.max(player.duration, 0);
        if (end > 0 && player.currentTime < end - 5) return;
        setEndedNonce((n) => n + 1);
      }),
      player.addListener('availableAudioTracksChange', ({ availableAudioTracks }) => {
        if (!core.active || core.terminal) return;
        if (localUri) setLocalAudio(availableAudioTracks);
      }),
      // Keep the selected ordinal honest whichever side initiated the switch
      // (our setter, the system Now Playing UI, or the platform's default pick).
      player.addListener('audioTrackChange', ({ audioTrack }) => {
        if (!core.active || core.terminal) return;
        if (!localUri || !audioTrack) return;
        const i = player.availableAudioTracks.findIndex(
          (t) => nativeTrackKey(t) === nativeTrackKey(audioTrack),
        );
        if (i >= 0) {
          patchCore(core, { audioIndex: i });
          setAudioIndex(i);
        }
      }),
    ];
    return () => {
      for (const s of subs) s.remove();
    };
  }, [player, core, fail, localUri, clearFailure]);
}
