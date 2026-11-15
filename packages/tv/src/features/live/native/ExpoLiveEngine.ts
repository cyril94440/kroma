import { BEHIND_LIVE_SECONDS, canRebuild, type MoveRange, STALL_RECOVERY_MS } from '@kromatv/core';
import { createVideoPlayer, type VideoPlayer } from 'expo-video';
import type { LiveEngine, LiveListeners } from '#tv/features/live/engine/liveEngine';

const WATCHDOG_MS = 500;
const READY_STATUS = 'readyToPlay';

/** A live stream on expo-video's platform player (AVPlayer on Apple TV and
 *  iOS, Media3 on Android). The frame time is the player's own programme date
 *  time (`currentLiveTimestamp`), the live edge is the offset it reports from
 *  it. A load that outlasts the stall budget rebuilds the player, and an error
 *  rebuilds it up to three times before it is reported. */
export class ExpoLiveEngine implements LiveEngine {
  readonly player: VideoPlayer;
  private readonly listeners: LiveListeners;
  private readonly subscriptions: { remove(): void }[];
  private readonly watchdog: ReturnType<typeof setInterval>;
  private loadingSince: number | null = null;
  private rebuilds = 0;
  private ready = false;

  constructor(url: string, listeners: LiveListeners) {
    const player = createVideoPlayer({ uri: url, contentType: 'hls' });
    this.player = player;
    this.listeners = listeners;
    this.subscriptions = [
      player.addListener('playingChange', ({ isPlaying }) => {
        if (isPlaying) listeners.onPlay();
        else listeners.onPause();
      }),
      player.addListener('statusChange', ({ status }) => {
        if (status === 'loading') {
          this.loadingSince ??= Date.now();
          listeners.onWaiting();
          return;
        }
        this.loadingSince = null;
        if (status === 'error') this.failed();
        if (status !== READY_STATUS) return;
        this.ready = true;
        this.rebuilds = 0;
        listeners.onPlaying();
        listeners.onReady();
      }),
      player.addListener('videoTrackChange', ({ videoTrack }) => {
        const size = videoTrack?.size;
        if (size && size.height > 0) listeners.onAspect?.(size.width / size.height);
      }),
    ];
    this.watchdog = setInterval(() => this.watch(), WATCHDOG_MS);
    listeners.onSurfaceChange?.();
  }

  private failed(): void {
    if (!canRebuild(this.rebuilds)) {
      this.listeners.onError();
      return;
    }
    this.rebuilds += 1;
    this.listeners.onLost();
  }

  private watch(): void {
    if (this.loadingSince === null || Date.now() - this.loadingSince <= STALL_RECOVERY_MS) return;
    this.loadingSince = null;
    this.failed();
  }

  play(): void {
    this.player.play();
  }
  pause(): void {
    this.player.pause();
  }
  isPaused(): boolean {
    return !this.player.playing;
  }
  isReady(): boolean {
    return this.ready;
  }
  isBehindLive(): boolean {
    const offset = this.player.currentOffsetFromLive;
    return offset !== null && offset > BEHIND_LIVE_SECONDS;
  }
  goLive(): void {
    const offset = this.player.currentOffsetFromLive;
    if (offset !== null && offset > 0) this.player.seekBy(offset);
  }
  frameTimeMs(): number | null {
    return this.player.currentLiveTimestamp;
  }
  bufferAheadMs(): number {
    return Math.max(0, this.player.bufferedPosition - this.player.currentTime) * 1000;
  }
  moveRange(): MoveRange | null {
    return null;
  }
  seekByMs(deltaMs: number): void {
    this.player.seekBy(deltaMs / 1000);
  }
  setRate(rate: number): void {
    this.player.playbackRate = rate;
  }
  destroy(): void {
    clearInterval(this.watchdog);
    for (const subscription of this.subscriptions) subscription.remove();
    try {
      this.player.pause();
      this.player.release();
    } catch {
      return;
    }
  }
}
