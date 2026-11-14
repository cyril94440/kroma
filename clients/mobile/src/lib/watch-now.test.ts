// @vitest-environment jsdom
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { watchNow, play } = vi.hoisted(() => ({ watchNow: vi.fn(), play: vi.fn() }));

vi.mock('#mobile/lib/session', () => ({ useClient: () => ({ discovery: { watchNow } }) }));
vi.mock('#mobile/lib/play', () => ({ usePlay: () => ({ play, device: null }) }));

import { useWatchNowAction } from './watch-now';

const SHOW = { kind: 'show' as const, tmdbId: 1399 };

beforeEach(() => {
  watchNow.mockReset();
  play.mockReset();
});

describe('useWatchNowAction', () => {
  it('plays the item the server made ready the way a library title plays', async () => {
    watchNow.mockResolvedValue({ status: 'ready', itemId: 'e1' });

    const { result } = renderHook(() => useWatchNowAction(SHOW));
    act(() => result.current.watch({ season: 1, episode: 3 }));

    await waitFor(() => expect(play).toHaveBeenCalledWith('e1'));
    expect(watchNow).toHaveBeenCalledWith('tv', 1399, { season: 1, episode: 3 });
  });

  it('plays nothing for a title that was only requested', async () => {
    watchNow.mockResolvedValue({ status: 'requested' });

    const { result } = renderHook(() => useWatchNowAction(SHOW));
    act(() => result.current.watch());

    await waitFor(() => expect(result.current.state.phase).toBe('requested'));
    expect(play).not.toHaveBeenCalled();
  });
});
