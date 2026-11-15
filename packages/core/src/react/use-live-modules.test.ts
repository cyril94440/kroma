// @vitest-environment jsdom
import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useLiveModules } from './use-live-modules';

const asClient = (list: () => Promise<unknown>) =>
  ({ baseUrl: 'http://s', modules: { list } }) as unknown as Parameters<typeof useLiveModules>[0];

describe('which live modules the server runs', () => {
  it('reads none until the server has answered', () => {
    const { result } = renderHook(() =>
      useLiveModules(asClient(() => new Promise(() => undefined))),
    );

    expect(result.current).toEqual({
      modules: { live: false, party: false, sport: false },
      ready: false,
    });
  });

  it('reads the enabled ones from the module list', async () => {
    const client = asClient(async () => [{ id: 'tv.kroma.live', name: 'Live' }]);
    const { result } = renderHook(() => useLiveModules(client));

    await waitFor(() => expect(result.current.ready).toBe(true));
    expect(result.current.modules.live).toBe(true);
    expect(result.current.modules.party).toBe(false);
  });

  it('reads none when the list cannot be fetched', async () => {
    const client = asClient(vi.fn().mockRejectedValue(new Error('down')));
    const { result } = renderHook(() => useLiveModules(client));

    await waitFor(() => expect(result.current.ready).toBe(true));
    expect(result.current.modules.live).toBe(false);
  });

  it('keeps its answer when the same server gets a new client, and drops it for another server', async () => {
    const first = asClient(async () => [{ id: 'tv.kroma.live', name: 'Live' }]);
    const { result, rerender } = renderHook(({ client }) => useLiveModules(client), {
      initialProps: { client: first },
    });
    await waitFor(() => expect(result.current.modules.live).toBe(true));

    rerender({ client: asClient(() => new Promise(() => undefined)) });
    expect(result.current.modules.live).toBe(true);

    const other = {
      baseUrl: 'http://other',
      modules: { list: () => new Promise(() => undefined) },
    };
    rerender({ client: other as unknown as typeof first });
    expect(result.current).toEqual({
      modules: { live: false, party: false, sport: false },
      ready: false,
    });
  });

  it('asks nothing without a client', () => {
    const { result } = renderHook(() => useLiveModules(null));

    expect(result.current.ready).toBe(false);
  });
});
