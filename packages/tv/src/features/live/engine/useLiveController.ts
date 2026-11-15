import { type PlayerAudioTrack, type PlayerController, type PlayerSub, useT } from '@kromatv/ui';
import { useMemo } from 'react';
import type { LiveEngine } from '#tv/features/live/engine/liveEngine';
import type { LivePlayback } from '#tv/features/live/engine/useLivePlayback';

const NOOP = (): undefined => undefined;
const NO_STATS = () => ({});
const NO_AUDIO: PlayerAudioTrack[] = [];
const NO_SUBTITLES: PlayerSub[] = [];

/** The shared player's controller over a live stream: no position to seek, a
 *  way back to the live edge, and the television's own volume. */
export function useLiveController<E extends LiveEngine>(live: LivePlayback<E>): PlayerController {
  const t = useT();
  const { playing, waiting, ready, failed, behind, aspect, togglePlay, goLive } = live;
  return useMemo(
    () => ({
      cur: 0,
      dur: 0,
      bufEnd: null,
      seekPreview: null,
      playing,
      waiting,
      waitReason: ready ? 'buffering' : 'loading',
      ready,
      error: failed ? t('player.cantPlay') : null,
      errorHint: failed ? t('player.cantPlayHint') : null,
      endedNonce: 0,
      surface: 'video',
      aspect,
      togglePlay,
      seekTo: NOOP,
      skip: NOOP,
      scrubPreview: NOOP,
      scrubCommit: NOOP,
      volume: 1,
      muted: false,
      setVolume: NOOP,
      toggleMute: NOOP,
      rate: 1,
      setRate: NOOP,
      audioTracks: NO_AUDIO,
      audioIndex: 0,
      setAudio: NOOP,
      subtitles: NO_SUBTITLES,
      subtitleIndex: null,
      setSubtitle: NOOP,
      qualities: [{ id: 'auto', label: t('player.qualityAuto') }],
      qualityId: 'auto',
      setQuality: NOOP,
      audioFilter: 'off',
      setAudioFilter: NOOP,
      audioFilterSupported: false,
      pipActive: false,
      togglePip: NOOP,
      fullscreen: false,
      toggleFullscreen: NOOP,
      live: { behind, goLive },
      getStats: NO_STATS,
    }),
    [playing, waiting, ready, failed, behind, aspect, togglePlay, goLive, t],
  );
}
