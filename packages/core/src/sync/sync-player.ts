export interface PlayerSnapshot {
  positionMs: number | null;
  playing: boolean;
  channelId: string | null;
}

export interface MoveRange {
  backMs: number;
  forwardMs: number;
}

export interface SyncPlayer {
  getPositionMs(): number | null;
  isReady(): boolean;
  getBufferAheadMs(): number;
  getMoveRange(): MoveRange | null;
  seekToLiveEdge(): void;
  seekByMs(deltaMs: number): void;
  setPlaybackRate(rate: number): void;
  subscribe(listener: (snapshot: PlayerSnapshot) => void): () => void;
}

export function clampMove(deltaMs: number, range: MoveRange | null): number {
  if (!range) return deltaMs;
  if (deltaMs >= 0) return Math.min(deltaMs, Math.max(range.forwardMs, 0));
  return Math.max(deltaMs, -Math.max(range.backMs, 0));
}
