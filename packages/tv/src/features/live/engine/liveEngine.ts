import type { ComponentType } from 'react';
import type { MoveRange } from '@kromatv/core';
import type { LivePlayback } from '#tv/features/live/engine/useLivePlayback';

export interface LiveListeners {
  onPlay(): void;
  onPause(): void;
  onWaiting(): void;
  onPlaying(): void;
  onReady(): void;
  onError(): void;
  onLost(): void;
  onAspect?(aspect: number): void;
  onSurfaceChange?(): void;
}

/** A live HLS stream on one platform's player. What it reports is read live,
 *  so a caller never holds a stale position: `frameTimeMs` is the server time
 *  of the frame on screen (the playlist's programme date time), which is what
 *  the sync and the match panel measure the viewer's delay against. */
export interface LiveEngine {
  play(): void;
  pause(): void;
  isPaused(): boolean;
  isReady(): boolean;
  isBehindLive(): boolean;
  goLive(): void;
  frameTimeMs(): number | null;
  bufferAheadMs(): number;
  moveRange(): MoveRange | null;
  seekByMs(deltaMs: number): void;
  setRate(rate: number): void;
  destroy(): void;
}

export interface CreateLiveArgs {
  url: string;
  video: HTMLVideoElement | null;
  listeners: LiveListeners;
}

export interface LiveSurfaceProps<E extends LiveEngine> {
  live: LivePlayback<E>;
  title: string;
}

/** One shell's live playback: the engine that opens a ticketed HLS URL and the
 *  surface it draws on. `createLiveEngine` answers `null` while the surface it
 *  needs has not mounted yet, and is asked again once it has. */
export interface LiveBackend<E extends LiveEngine> {
  createLiveEngine(args: CreateLiveArgs): E | null;
  Surface: ComponentType<LiveSurfaceProps<E>>;
}
