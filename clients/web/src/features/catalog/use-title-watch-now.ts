import { ItemId } from '@kromatv/client/media';
import { type TitleWatchNow, useWatchNow } from '@kromatv/core/react';
import { useAuth } from '#web/shared/lib/auth';
import type { TitleView } from '#web/shared/lib/titleView';

export interface TitleWatchNowState extends TitleWatchNow {
  offered: boolean;
}

/** "Watch now" on a title the library does not hold. When the server answers
 * that the title plays, it is opened the way the Play button of a title in the
 * library opens it. */
export function useTitleWatchNow(view: TitleView, play: (id: ItemId) => void): TitleWatchNowState {
  const { client } = useAuth();
  const watch = useWatchNow(
    client,
    { kind: view.kind, tmdbId: view.tmdbId ?? 0 },
    { onReady: (itemId) => play(ItemId.parse(itemId)) },
  );
  return { ...watch, offered: view.watchNow && view.tmdbId != null };
}
