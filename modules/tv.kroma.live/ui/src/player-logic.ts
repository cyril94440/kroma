const BEHIND_LIVE_SECONDS = 6;
export const STALL_RECOVERY_MS = 4000;
const MAX_NETWORK_RETRIES = 6;

export const HLS_CONFIG = {
  liveSyncDurationCount: 4,
  liveMaxLatencyDurationCount: 15,
  liveDurationInfinity: true,
  lowLatencyMode: false,
  backBufferLength: 30,
  manifestLoadingMaxRetry: 6,
  levelLoadingMaxRetry: 6,
  fragLoadingMaxRetry: 6,
} as const;

export type FatalKind = 'network' | 'media' | 'other';

export type Recovery = 'startLoad' | 'recoverMedia' | 'reload' | 'give-up';

export function recoveryFor(kind: FatalKind, attempts: number, httpStatus?: number): Recovery {
  if (httpStatus === 404 || httpStatus === 403) return 'reload';
  if (kind === 'media') return attempts < 3 ? 'recoverMedia' : 'reload';
  if (kind === 'network') return attempts < MAX_NETWORK_RETRIES ? 'startLoad' : 'reload';
  return 'give-up';
}

export function isBehindLive(currentTime: number, livePosition: number | null): boolean {
  return livePosition !== null && livePosition - currentTime > BEHIND_LIVE_SECONDS;
}

export function wantsNative(support: { mse: boolean; nativeHls: boolean }): boolean {
  return !support.mse && support.nativeHls;
}

export function pauseWasDeliberate(msSinceGesture: number | null): boolean {
  return msSinceGesture !== null && msSinceGesture < 1500;
}

export function latencyOf(playingDate: number | null, now: number): number | null {
  return playingDate === null ? null : Math.max(0, (now - playingDate) / 1000);
}
