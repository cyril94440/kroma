import { ItemId } from '@kromatv/client/media';
import type { RequestKind } from '@kromatv/client/requests';
import { type TitleWatchNow, useWatchNow } from '@kromatv/core/react';
import { useT } from '@kromatv/ui';
import { toast } from '@kromatv/ui/kit';
import { useEffect, useRef } from 'react';
import { useClient, useNav } from '#tv/app/router';

/** "Watch now" on a title the library does not hold: when the server answers
 * that it plays, the player opens on it the way a library title's Play does, and
 * every other answer is a toast. */
export function useWatchNowPlay(title: { kind: RequestKind; tmdbId: number }): TitleWatchNow {
  const t = useT();
  const client = useClient();
  const nav = useNav();
  const watch = useWatchNow(client, title, {
    onReady: (itemId) => {
      client.media.item(ItemId.parse(itemId)).then(
        (item) => nav.go('player', { item }),
        () => toast({ message: t('discover.watchNowError') }),
      );
    },
  });
  const { state } = watch;
  const told = useRef<typeof state | null>(null);

  useEffect(() => {
    if (told.current === state) return;
    told.current = state;
    if (state.phase === 'requested') toast({ message: t('discover.watchNowRequested') });
    if (state.phase === 'unavailable') toast({ message: state.message });
    if (state.phase === 'error') toast({ message: t('discover.watchNowError') });
  }, [state, t]);

  return watch;
}
