// @vitest-environment jsdom
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { watchNow, item, go, toast, client } = vi.hoisted(() => {
  const watchNow = vi.fn();
  const item = vi.fn();
  return {
    watchNow,
    item,
    go: vi.fn(),
    toast: vi.fn(),
    client: { discovery: { watchNow }, media: { item } },
  };
});

vi.mock('#tv/app/router', () => ({ useClient: () => client, useNav: () => ({ go }) }));
vi.mock('@kromatv/ui', () => ({ useT: () => (key: string) => key }));
vi.mock('@kromatv/ui/kit', () => ({ toast }));

import { useWatchNowPlay } from './useWatchNowPlay';

const DUNE = { kind: 'movie' as const, tmdbId: 438_631 };

beforeEach(() => {
  for (const fn of [watchNow, item, go, toast]) fn.mockReset();
});

describe('useWatchNowPlay', () => {
  it('opens the player on the item the server made ready', async () => {
    watchNow.mockResolvedValue({ status: 'ready', itemId: 'm1' });
    item.mockResolvedValue({ id: 'm1' });

    const { result } = renderHook(() => useWatchNowPlay(DUNE));
    act(() => result.current.watch());

    await waitFor(() => expect(go).toHaveBeenCalledWith('player', { item: { id: 'm1' } }));
    expect(toast).not.toHaveBeenCalled();
  });

  it('tells the viewer when the title was requested instead', async () => {
    watchNow.mockResolvedValue({ status: 'requested' });

    const { result } = renderHook(() => useWatchNowPlay(DUNE));
    act(() => result.current.watch());

    await waitFor(() =>
      expect(toast).toHaveBeenCalledWith({ message: 'discover.watchNowRequested' }),
    );
    expect(go).not.toHaveBeenCalled();
  });

  it('gives the servers own words when it cannot', async () => {
    watchNow.mockResolvedValue({ status: 'unavailable', reason: 'noEngine', message: 'No.' });

    const { result } = renderHook(() => useWatchNowPlay(DUNE));
    act(() => result.current.watch());

    await waitFor(() => expect(toast).toHaveBeenCalledWith({ message: 'No.' }));
  });

  it('says the connection dropped when the call fails', async () => {
    watchNow.mockRejectedValue(new Error('dropped'));

    const { result } = renderHook(() => useWatchNowPlay(DUNE));
    act(() => result.current.watch());

    await waitFor(() => expect(toast).toHaveBeenCalledWith({ message: 'discover.watchNowError' }));
  });

  it('says so when the ready item cannot be fetched', async () => {
    watchNow.mockResolvedValue({ status: 'ready', itemId: 'm1' });
    item.mockRejectedValue(new Error('gone'));

    const { result } = renderHook(() => useWatchNowPlay(DUNE));
    act(() => result.current.watch());

    await waitFor(() => expect(toast).toHaveBeenCalledWith({ message: 'discover.watchNowError' }));
    expect(go).not.toHaveBeenCalled();
  });
});
