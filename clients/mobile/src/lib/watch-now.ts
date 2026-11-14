import type { RequestKind } from '@kromatv/client/requests';
import { type TitleWatchNow, useWatchNow } from '@kromatv/core/react';
import { usePlay } from '#mobile/lib/play';
import { useClient } from '#mobile/lib/session';

/** "Watch now" on a title the library does not hold: once the server has it
 * playing, it opens where a library title's Play opens it, on the connected TV
 * when one is driven and on this phone otherwise. */
export function useWatchNowAction(title: { kind: RequestKind; tmdbId: number }): TitleWatchNow {
  const client = useClient();
  const { play } = usePlay();
  return useWatchNow(client, title, { onReady: (itemId) => void play(itemId) });
}
