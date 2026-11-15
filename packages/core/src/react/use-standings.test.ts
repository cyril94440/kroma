// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useStandings } from './use-standings';
import type { LiveClient } from './use-live-room';

const table = {
  lignes: [{ rang: 1, club: 'PSG', pts: 28, diff: 12, match: '', outcome: null }],
  matchs_comptes: 1,
};

const clientOf = (standings: ReturnType<typeof vi.fn>) =>
  ({ sport: { standings } }) as unknown as LiveClient;

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('useStandings', () => {
  it('asks again every 2.5 s while the table is being built, then shows it', async () => {
    const standings = vi
      .fn()
      .mockResolvedValueOnce({ pending: true, standings: null })
      .mockResolvedValueOnce({ pending: false, standings: table });
    const client = clientOf(standings);
    const view = renderHook(() => useStandings(client, 'Ligue 1', true));
    await act(() => vi.advanceTimersByTimeAsync(0));

    expect(view.result.current.state).toBe('loading');
    await act(() => vi.advanceTimersByTimeAsync(2500));

    expect(view.result.current).toEqual({ state: 'ready', table });
    expect(standings).toHaveBeenCalledTimes(2);
  });

  it('says the table is unavailable when the module has none', async () => {
    const standings = vi.fn().mockResolvedValue({ pending: false, standings: null });
    const client = clientOf(standings);
    const view = renderHook(() => useStandings(client, 'Liga', true));

    await act(() => vi.advanceTimersByTimeAsync(0));

    expect(view.result.current.state).toBe('unavailable');
  });

  it('reads nothing while its panel is closed', async () => {
    const standings = vi.fn();
    const client = clientOf(standings);
    renderHook(() => useStandings(client, 'Liga', false));

    await act(() => vi.advanceTimersByTimeAsync(5000));

    expect(standings).not.toHaveBeenCalled();
  });
});
