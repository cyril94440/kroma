import type { KromaClient } from '@kromatv/client';
import type { WatchNowEpisode, WatchNowResult } from '@kromatv/client/discovery';
import type { RequestKind } from '@kromatv/client/requests';
import { useCallback, useEffect, useRef, useState } from 'react';

export interface WatchedTitle {
  kind: RequestKind;
  tmdbId: number;
}

export type WatchNowState =
  | { phase: 'idle' }
  | { phase: 'searching' }
  | { phase: 'ready'; itemId: string; fileId: string | null }
  | { phase: 'requested' }
  | { phase: 'unavailable'; reason: 'noEngine' | 'noIndexer' | 'noRelease'; message: string }
  | { phase: 'error' };

export interface TitleWatchNow {
  state: WatchNowState;
  /** Does nothing while a search is running, so a double press asks once. */
  watch: (episode?: WatchNowEpisode) => void;
  reset: () => void;
}

export interface WatchNowOptions {
  /** Called once with the item to play when the title is ready, while the screen
   * is still mounted. */
  onReady?: (itemId: string) => void;
}

const IDLE: WatchNowState = { phase: 'idle' };

function stateOf(result: WatchNowResult): WatchNowState {
  switch (result.status) {
    case 'ready':
      return { phase: 'ready', itemId: result.itemId, fileId: result.fileId ?? null };
    case 'requested':
      return { phase: 'requested' };
    case 'unavailable':
      return { phase: 'unavailable', reason: result.reason, message: result.message };
  }
}

/** Watch a title the library does not hold: the server fetches the best release
 * it can deliver at once and answers when the title plays. The caller moves on
 * when the state turns `ready`; a failed or dropped call is `error`, and asking
 * again is safe because the server answers a title it already fetched. */
export function useWatchNow(
  client: Pick<KromaClient, 'discovery'> | null,
  title: Readonly<WatchedTitle>,
  { onReady }: Readonly<WatchNowOptions> = {},
): TitleWatchNow {
  const [state, setState] = useState<WatchNowState>(IDLE);
  const running = useRef(false);
  const mounted = useRef(true);
  const ready = useRef(onReady);
  const { kind, tmdbId } = title;

  useEffect(() => {
    ready.current = onReady;
  }, [onReady]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const watch = useCallback(
    (episode?: WatchNowEpisode) => {
      if (!client || running.current) return;
      running.current = true;
      setState({ phase: 'searching' });
      client.discovery.watchNow(kind === 'show' ? 'tv' : 'movie', tmdbId, episode).then(
        (result) => {
          running.current = false;
          if (!mounted.current) return;
          setState(stateOf(result));
          if (result.status === 'ready') ready.current?.(result.itemId);
        },
        () => {
          running.current = false;
          if (mounted.current) setState({ phase: 'error' });
        },
      );
    },
    [client, kind, tmdbId],
  );

  const reset = useCallback(() => {
    if (!running.current) setState(IDLE);
  }, []);

  return { state, watch, reset };
}
