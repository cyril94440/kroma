import type { KromaClient } from '@kromatv/client';
import type { ChannelView } from '@kromatv/client/live';
import { useCallback, useEffect, useState } from 'react';

interface ChannelList {
  favourites: ChannelView[];
  channels: ChannelView[];
  hasMore: boolean;
  loadMore: () => void;
}

const PER_PAGE = 24;

/** The quick-access channels and the full channel list, a page at a time. */
export function useChannelList(client: Pick<KromaClient, 'live'>, open: boolean): ChannelList {
  const [favourites, setFavourites] = useState<ChannelView[]>([]);
  const [channels, setChannels] = useState<ChannelView[]>([]);
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(0);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    client.live.favourites().then(
      (list) => {
        if (!cancelled) setFavourites(list);
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [client, open]);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    client.live.channels({ page: page + 1, perPage: PER_PAGE }).then(
      (answer) => {
        if (cancelled) return;
        setTotal(answer.total);
        setChannels((current) => (page === 0 ? answer.channels : [...current, ...answer.channels]));
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [client, open, page]);

  const loadMore = useCallback(() => setPage((current) => current + 1), []);
  return { favourites, channels, hasMore: channels.length < total, loadMore };
}
