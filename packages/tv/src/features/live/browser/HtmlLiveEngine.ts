import {
  isBehindLive,
  LIVE_HLS_CONFIG,
  type LiveFatalKind,
  liveRecoveryFor,
  type MoveRange,
  STALL_RECOVERY_MS,
  wantsNativeHls,
} from '@kromatv/core';
import type { LiveEngine, LiveListeners } from '#tv/features/live/engine/liveEngine';

type HlsInstance = import('hls.js').default;

const RELOAD_DELAY_MS = 1500;
const WATCHDOG_MS = 500;
const READY_STATE = 3;
const WINDOW_MARGIN_START_S = 0.5;
const WINDOW_MARGIN_END_S = 4;
const NATIVE_HLS = 'application/vnd.apple.mpegurl';

interface WithStartDate {
  getStartDate?: () => Date;
}

function kindOf(type: string): LiveFatalKind {
  if (type === 'networkError') return 'network';
  if (type === 'mediaError') return 'media';
  return 'other';
}

/** A live stream on the page's own `<video>`: hls.js where the browser has
 *  media source extensions, the browser's own HLS otherwise. Fatal errors climb
 *  the cheapest repair first and end in `onLost`, which rebuilds the engine; a
 *  picture stalled for four seconds is pushed back to the live edge. */
export class HtmlLiveEngine implements LiveEngine {
  private readonly video: HTMLVideoElement;
  private readonly listeners: LiveListeners;
  private hls: HlsInstance | null = null;
  private destroyed = false;
  private stalledAt: number | null = null;
  private readonly watchdog: ReturnType<typeof setInterval>;
  private readonly unbind: () => void;

  constructor(video: HTMLVideoElement, url: string, listeners: LiveListeners) {
    this.video = video;
    this.listeners = listeners;
    this.unbind = this.bind();
    this.watchdog = setInterval(() => this.watch(), WATCHDOG_MS);
    void this.open(url);
  }

  private bind(): () => void {
    const v = this.video;
    const events: [string, () => void][] = [
      ['play', () => this.listeners.onPlay()],
      ['pause', () => this.listeners.onPause()],
      ['waiting', () => this.listeners.onWaiting()],
      ['playing', () => this.listeners.onPlaying()],
      ['canplay', () => this.listeners.onReady()],
      ['loadeddata', () => this.listeners.onReady()],
      ['error', () => this.listeners.onError()],
      [
        'resize',
        () => {
          if (v.videoHeight > 0) this.listeners.onAspect?.(v.videoWidth / v.videoHeight);
        },
      ],
    ];
    for (const [type, fn] of events) v.addEventListener(type, fn);
    return () => {
      for (const [type, fn] of events) v.removeEventListener(type, fn);
    };
  }

  private async open(url: string): Promise<void> {
    const { default: Hls } = await import('hls.js');
    if (this.destroyed) return;
    const mse = Hls.isSupported();
    if (wantsNativeHls({ mse, nativeHls: this.video.canPlayType(NATIVE_HLS) !== '' })) {
      this.video.src = url;
      return;
    }
    if (!mse) {
      this.listeners.onError();
      return;
    }
    const hls = new Hls({ ...LIVE_HLS_CONFIG });
    this.hls = hls;
    const attempts = { network: 0, media: 0 };
    hls.attachMedia(this.video);
    hls.loadSource(url);
    hls.on(Hls.Events.FRAG_LOADED, () => {
      attempts.network = 0;
    });
    hls.on(Hls.Events.ERROR, (_event, data) => {
      if (!data.fatal) return;
      const kind = kindOf(data.type);
      const tries = kind === 'media' ? attempts.media++ : attempts.network++;
      const action = liveRecoveryFor(kind, tries, data.response?.code);
      if (action === 'startLoad') hls.startLoad();
      else if (action === 'recoverMedia') hls.recoverMediaError();
      else if (action === 'reload') setTimeout(() => this.lost(), RELOAD_DELAY_MS);
      else this.listeners.onError();
    });
  }

  private lost(): void {
    if (!this.destroyed) this.listeners.onLost();
  }

  private liveEdge(): number | null {
    return this.hls?.liveSyncPosition ?? null;
  }

  private watch(): void {
    const stalled = this.video.readyState < READY_STATE && !this.video.paused;
    if (!stalled) {
      this.stalledAt = null;
      return;
    }
    this.stalledAt ??= Date.now();
    if (Date.now() - this.stalledAt <= STALL_RECOVERY_MS || !this.hls) return;
    this.stalledAt = null;
    this.hls.startLoad();
    this.goLive();
  }

  play(): void {
    (this.video.play() as Promise<void> | undefined)?.catch(() => undefined);
  }
  pause(): void {
    this.video.pause();
  }
  isPaused(): boolean {
    return this.video.paused;
  }
  isReady(): boolean {
    return this.video.readyState >= READY_STATE;
  }
  isBehindLive(): boolean {
    return isBehindLive(this.video.currentTime, this.liveEdge());
  }
  goLive(): void {
    const edge = this.liveEdge();
    if (edge !== null) this.video.currentTime = edge;
  }
  frameTimeMs(): number | null {
    const playing = this.hls?.playingDate;
    if (playing) return playing.getTime();
    const start = (this.video as WithStartDate).getStartDate?.().getTime();
    return start === undefined || Number.isNaN(start) ? null : start + this.video.currentTime * 1000;
  }
  bufferAheadMs(): number {
    const { buffered, currentTime } = this.video;
    return buffered.length ? (buffered.end(buffered.length - 1) - currentTime) * 1000 : 0;
  }
  moveRange(): MoveRange | null {
    const { seekable, currentTime } = this.video;
    if (!seekable.length) return null;
    return {
      backMs: (currentTime - (seekable.start(0) + WINDOW_MARGIN_START_S)) * 1000,
      forwardMs: (seekable.end(seekable.length - 1) - WINDOW_MARGIN_END_S - currentTime) * 1000,
    };
  }
  seekByMs(deltaMs: number): void {
    this.video.currentTime = Math.max(0, this.video.currentTime + deltaMs / 1000);
  }
  setRate(rate: number): void {
    this.video.playbackRate = rate;
  }
  destroy(): void {
    this.destroyed = true;
    clearInterval(this.watchdog);
    this.unbind();
    this.hls?.destroy();
    this.hls = null;
    this.video.pause();
    this.video.removeAttribute('src');
    this.video.load();
  }
}
