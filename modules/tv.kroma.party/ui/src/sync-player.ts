import { z } from 'zod';

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

const method = <T extends (...args: never[]) => unknown>() =>
  z.custom<T>((value) => typeof value === 'function');

export const AsideProps = z.object({
  player: z.object({
    getPositionMs: method<SyncPlayer['getPositionMs']>(),
    isReady: method<SyncPlayer['isReady']>(),
    getBufferAheadMs: method<SyncPlayer['getBufferAheadMs']>(),
    getMoveRange: method<SyncPlayer['getMoveRange']>(),
    seekToLiveEdge: method<SyncPlayer['seekToLiveEdge']>(),
    seekByMs: method<SyncPlayer['seekByMs']>(),
    setPlaybackRate: method<SyncPlayer['setPlaybackRate']>(),
    subscribe: method<SyncPlayer['subscribe']>(),
  }),
  channel: z.object({ id: z.string(), name: z.string() }).nullable().catch(null),
  playing: z.boolean().catch(false),
});
export type AsideProps = z.infer<typeof AsideProps>;

export const OverlayProps = AsideProps.pick({ channel: true, playing: true }).extend({
  immersive: z.boolean().catch(false),
});

export function clampMove(deltaMs: number, range: MoveRange | null): number {
  if (!range) return deltaMs;
  return Math.max(-range.backMs, Math.min(deltaMs, range.forwardMs));
}
