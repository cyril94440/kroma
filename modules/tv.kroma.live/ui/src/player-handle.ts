export interface PlayerChannel {
  id: string;
  name: string;
}

export interface PlayerSnapshot {
  positionMs: number | null;
  playing: boolean;
  channelId: string | null;
}

/** What `tv.kroma.live/player.aside` and `tv.kroma.live/player.overlay` receive
 *  as `player`: a plain object of functions, stable across renders of the page.
 *  `getPositionMs` is the wall-clock time of the frame on screen, from the
 *  playlist's programme date time, or null when the player does not know it. */
export interface MoveRange {
  backMs: number;
  forwardMs: number;
}

/** `getMoveRange` is how far a seek may go from here and stay inside the safe
 *  zone of the player's window, `forwardMs` negative when already past it;
 *  null when the window is unknown. `seekByMs` takes a negative delta to go
 *  back and does not clamp. */
export interface PlayerHandle {
  getPositionMs(): number | null;
  isReady(): boolean;
  getBufferAheadMs(): number;
  getMoveRange(): MoveRange | null;
  seekToLiveEdge(): void;
  seekByMs(deltaMs: number): void;
  setPlaybackRate(rate: number): void;
  subscribe(listener: (snapshot: PlayerSnapshot) => void): () => void;
}

export interface HandleSource {
  element: HTMLVideoElement;
  playingDate: () => number | null;
  liveSyncPosition: () => number | null;
  channelId: () => string | null;
}

export const SNAPSHOT_EVERY_MS = 250;
const READY_STATE = 3;
const WINDOW_MARGIN_START_S = 0.5;
const WINDOW_MARGIN_END_S = 4;

export function createPlayerHandle(source: HandleSource): PlayerHandle {
  const { element } = source;
  const snapshot = (): PlayerSnapshot => ({
    positionMs: source.playingDate(),
    playing: !element.paused,
    channelId: source.channelId(),
  });
  return {
    getPositionMs: () => source.playingDate(),
    isReady: () => element.readyState >= READY_STATE,
    getBufferAheadMs: () => {
      const { buffered, currentTime } = element;
      return buffered.length ? (buffered.end(buffered.length - 1) - currentTime) * 1000 : 0;
    },
    getMoveRange: () => {
      const { seekable, currentTime } = element;
      if (!seekable.length) return null;
      return {
        backMs: (currentTime - (seekable.start(0) + WINDOW_MARGIN_START_S)) * 1000,
        forwardMs: (seekable.end(seekable.length - 1) - WINDOW_MARGIN_END_S - currentTime) * 1000,
      };
    },
    seekToLiveEdge: () => {
      const live = source.liveSyncPosition();
      if (live !== null) element.currentTime = live;
    },
    seekByMs: (deltaMs) => {
      element.currentTime = Math.max(0, element.currentTime + deltaMs / 1000);
    },
    setPlaybackRate: (rate) => {
      element.playbackRate = rate;
    },
    subscribe: (listener) => {
      const emit = () => listener(snapshot());
      const timer = setInterval(emit, SNAPSHOT_EVERY_MS);
      element.addEventListener('play', emit);
      element.addEventListener('pause', emit);
      element.addEventListener('seeked', emit);
      return () => {
        clearInterval(timer);
        element.removeEventListener('play', emit);
        element.removeEventListener('pause', emit);
        element.removeEventListener('seeked', emit);
      };
    },
  };
}
