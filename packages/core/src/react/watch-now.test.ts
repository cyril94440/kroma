// @vitest-environment jsdom
import type { KromaClient } from '@kromatv/client';
import type { WatchNowResult } from '@kromatv/client/discovery';
import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useWatchNow, type WatchedTitle } from './watch-now';

const DUNE: WatchedTitle = { kind: 'movie', tmdbId: 438_631 };
const SHOW: WatchedTitle = { kind: 'show', tmdbId: 1399 };

function stub(answer: () => Promise<WatchNowResult>) {
  const watchNow = vi.fn(answer);
  return { client: { discovery: { watchNow } } as unknown as KromaClient, watchNow };
}

describe('watching a title the library does not hold', () => {
  it('rests until asked', () => {
    const { client } = stub(async () => ({ status: 'requested' }));

    const { result } = renderHook(() => useWatchNow(client, DUNE));

    expect(result.current.state).toEqual({ phase: 'idle' });
  });

  it('searches, then hands back the item to play', async () => {
    const { client, watchNow } = stub(async () => ({
      status: 'ready',
      itemId: 'm1',
      fileId: 'f1',
    }));

    const { result } = renderHook(() => useWatchNow(client, DUNE));
    act(() => result.current.watch());

    expect(result.current.state).toEqual({ phase: 'searching' });
    await waitFor(() =>
      expect(result.current.state).toEqual({ phase: 'ready', itemId: 'm1', fileId: 'f1' }),
    );
    expect(watchNow).toHaveBeenCalledWith('movie', 438_631, undefined);
  });

  it('opens the item once when it is ready, and not for a title that was only requested', async () => {
    const onReady = vi.fn();
    const ready = stub(async () => ({ status: 'ready', itemId: 'm1' }));
    const asked = stub(async () => ({ status: 'requested' }));

    const first = renderHook(() => useWatchNow(ready.client, DUNE, { onReady }));
    const second = renderHook(() => useWatchNow(asked.client, DUNE, { onReady }));
    act(() => first.result.current.watch());
    act(() => second.result.current.watch());
    await waitFor(() => expect(first.result.current.state.phase).toBe('ready'));
    await waitFor(() => expect(second.result.current.state.phase).toBe('requested'));

    expect(onReady).toHaveBeenCalledTimes(1);
    expect(onReady).toHaveBeenCalledWith('m1');
  });

  it('does not open anything once the screen is gone', async () => {
    let finish: (r: WatchNowResult) => void = () => {};
    const { client } = stub(() => new Promise((resolve) => (finish = resolve)));
    const onReady = vi.fn();
    const { result, unmount } = renderHook(() => useWatchNow(client, DUNE, { onReady }));
    act(() => result.current.watch());

    unmount();
    await act(async () => finish({ status: 'ready', itemId: 'm1' }));

    expect(onReady).not.toHaveBeenCalled();
  });

  it('names a show the way the routes do and carries the episode', async () => {
    const { client, watchNow } = stub(async () => ({ status: 'ready', itemId: 'e1' }));

    const { result } = renderHook(() => useWatchNow(client, SHOW));
    act(() => result.current.watch({ season: 2, episode: 3 }));

    await waitFor(() => expect(result.current.state.phase).toBe('ready'));
    expect(watchNow).toHaveBeenCalledWith('tv', 1399, { season: 2, episode: 3 });
  });

  it('says the title was requested when nothing could be fetched at once', async () => {
    const { client } = stub(async () => ({ status: 'requested', requestId: 'r1' }));

    const { result } = renderHook(() => useWatchNow(client, DUNE));
    act(() => result.current.watch());

    await waitFor(() => expect(result.current.state).toEqual({ phase: 'requested' }));
  });

  it('carries the reason and the words of a refusal', async () => {
    const { client } = stub(async () => ({
      status: 'unavailable',
      reason: 'noRelease',
      message: 'Nothing fits.',
    }));

    const { result } = renderHook(() => useWatchNow(client, DUNE));
    act(() => result.current.watch());

    await waitFor(() =>
      expect(result.current.state).toEqual({
        phase: 'unavailable',
        reason: 'noRelease',
        message: 'Nothing fits.',
      }),
    );
  });

  it('is an error when the call fails, and asking again is allowed', async () => {
    let calls = 0;
    const { client } = stub(async () => {
      calls += 1;
      if (calls === 1) throw new Error('dropped');
      return { status: 'ready', itemId: 'm1' };
    });

    const { result } = renderHook(() => useWatchNow(client, DUNE));
    act(() => result.current.watch());
    await waitFor(() => expect(result.current.state).toEqual({ phase: 'error' }));
    act(() => result.current.watch());

    await waitFor(() => expect(result.current.state.phase).toBe('ready'));
  });

  it('asks once for a double press', async () => {
    let finish: (r: WatchNowResult) => void = () => {};
    const { client, watchNow } = stub(() => new Promise((resolve) => (finish = resolve)));

    const { result } = renderHook(() => useWatchNow(client, DUNE));
    act(() => {
      result.current.watch();
      result.current.watch();
    });
    await act(async () => finish({ status: 'requested' }));

    expect(watchNow).toHaveBeenCalledTimes(1);
  });

  it('goes back to rest on reset, but not in the middle of a search', async () => {
    let finish: (r: WatchNowResult) => void = () => {};
    const { client } = stub(() => new Promise((resolve) => (finish = resolve)));
    const { result } = renderHook(() => useWatchNow(client, DUNE));

    act(() => result.current.watch());
    act(() => result.current.reset());
    expect(result.current.state.phase).toBe('searching');
    await act(async () => finish({ status: 'requested' }));
    act(() => result.current.reset());

    expect(result.current.state).toEqual({ phase: 'idle' });
  });

  it('asks nothing before a server has been reached', () => {
    const { result } = renderHook(() => useWatchNow(null, DUNE));
    act(() => result.current.watch());

    expect(result.current.state).toEqual({ phase: 'idle' });
  });

  it('does not set state once the screen is gone', async () => {
    let finish: (r: WatchNowResult) => void = () => {};
    const { client } = stub(() => new Promise((resolve) => (finish = resolve)));
    const { result, unmount } = renderHook(() => useWatchNow(client, DUNE));
    act(() => result.current.watch());

    unmount();

    await expect(
      act(async () => finish({ status: 'ready', itemId: 'm1' })),
    ).resolves.toBeUndefined();
  });
});
