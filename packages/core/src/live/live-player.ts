export const BEHIND_LIVE_SECONDS = 6;
export const STALL_RECOVERY_MS = 4000;
export const MAX_NETWORK_RETRIES = 6;
export const SEEK_AFTER_SECONDS = 1.5;

/** The hls.js settings a live stream is played with. They are plain data so a
 *  shell that does not ship hls.js never imports it. */
export const LIVE_HLS_CONFIG = {
  liveSyncDurationCount: 4,
  liveMaxLatencyDurationCount: 15,
  liveDurationInfinity: true,
  lowLatencyMode: false,
  backBufferLength: 30,
  manifestLoadingMaxRetry: 6,
  levelLoadingMaxRetry: 6,
  fragLoadingMaxRetry: 6,
} as const;

export type LiveFatalKind = 'network' | 'media' | 'other';

export type LiveRecovery = 'startLoad' | 'recoverMedia' | 'reload' | 'give-up';

/** What to do after a fatal player error: a playlist the server no longer has
 *  (404, 403) means the stream restarted under a new folder and is reloaded;
 *  otherwise the cheapest repair first, a reload once the retries are spent. */
export function liveRecoveryFor(
  kind: LiveFatalKind,
  attempts: number,
  httpStatus?: number,
): LiveRecovery {
  if (httpStatus === 404 || httpStatus === 403) return 'reload';
  if (kind === 'media') return attempts < 3 ? 'recoverMedia' : 'reload';
  if (kind === 'network') return attempts < MAX_NETWORK_RETRIES ? 'startLoad' : 'reload';
  return 'give-up';
}

export function isBehindLive(currentTime: number, livePosition: number | null): boolean {
  return livePosition !== null && livePosition - currentTime > BEHIND_LIVE_SECONDS;
}

export function wantsNativeHls(support: { mse: boolean; nativeHls: boolean }): boolean {
  return !support.mse && support.nativeHls;
}

export function latencyOf(playingDate: number | null, now: number): number | null {
  return playingDate === null ? null : Math.max(0, (now - playingDate) / 1000);
}
