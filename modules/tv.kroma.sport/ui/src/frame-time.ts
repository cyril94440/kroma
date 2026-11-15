import { useCallback, useSyncExternalStore } from 'react';
import { z } from 'zod';

const method = <T extends (...args: never[]) => unknown>() =>
  z.custom<T>((value) => typeof value === 'function');

export interface FramePlayer {
  getPositionMs(): number | null;
}

export const AsideProps = z.object({
  player: z.object({ getPositionMs: method<FramePlayer['getPositionMs']>() }).passthrough(),
  channel: z.object({ id: z.string(), name: z.string() }).nullable().catch(null),
  playing: z.boolean().catch(false),
});
export type AsideProps = z.infer<typeof AsideProps>;

/** The server time of the frame on screen, read once a second from the player handle; `null` when nothing plays. */
export function useFrameTime(player: FramePlayer | null): number | null {
  const subscribe = useCallback((notify: () => void) => {
    const timer = setInterval(notify, 1000);
    return () => clearInterval(timer);
  }, []);
  const read = useCallback(() => {
    const position = player?.getPositionMs() ?? null;
    return position === null ? null : Math.floor(position / 1000) * 1000;
  }, [player]);
  return useSyncExternalStore(subscribe, read, () => null);
}
