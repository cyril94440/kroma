// @vitest-environment jsdom
import type { DiscoverEntry } from '@kromatv/client/discovery';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('#web/shared/lib/web-runtime', async (importOriginal) => {
  const original = await importOriginal<typeof import('#web/shared/lib/web-runtime')>();
  const { createWebTestRuntime } = await import('#web/shared/lib/web-runtime.fixture');
  const { QueryClient } =
    await vi.importActual<typeof import('@tanstack/react-query')>('@tanstack/react-query');
  const runtime = createWebTestRuntime({}, () => new QueryClient());
  return { ...original, useWebRuntime: () => runtime, useKromaClient: () => runtime.client };
});

const { useDiscoverRail, useDiscoverListPage, useGenres, useGenrePage } = await import(
  '#web/features/requests/use-discover-lists'
);

const ENTRY: DiscoverEntry = {
  kind: 'movie',
  tmdbId: 603,
  title: 'The Matrix',
  year: 1999,
  posterUrl: null,
  backdropUrl: null,
  overview: null,
  rating: null,
  inLibrary: false,
  localId: null,
  requestId: null,
  requestStatus: null,
  requestProgress: null,
};
const fetched = vi.fn();

function answer(url: string) {
  if (url.includes('/discover/genres')) return { genres: [{ id: 28, name: 'Action' }] };
  const totalPages = url.includes('/discover/genre/') ? 2 : 4;
  return { results: [ENTRY], page: 1, totalPages };
}

function wrapper({ children }: { children: ReactNode }) {
  const queries = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={queries}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  fetched.mockReset();
  vi.stubGlobal('fetch', async (input: RequestInfo | URL) => {
    const url = String(input instanceof Request ? input.url : input);
    fetched(url);
    return new Response(JSON.stringify(answer(url)), {
      headers: { 'content-type': 'application/json' },
    });
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const asked = () => fetched.mock.calls.map(([url]) => String(url));

describe('a discovery rail', () => {
  it('hands back page one of the list for a user who can request', async () => {
    const { result } = renderHook(() => useDiscoverRail('popular', true), { wrapper });

    await waitFor(() => expect(result.current.entries).toEqual([ENTRY]));

    expect(asked()[0]).toMatch(/\/api\/discover\/popular$/);
    expect(result.current.loading).toBe(false);
  });

  it('fetches nothing, and reports neither loading nor entries, when it is off', () => {
    const { result } = renderHook(() => useDiscoverRail('upcoming', false), { wrapper });

    expect(fetched).not.toHaveBeenCalled();
    expect(result.current).toEqual({ loading: false, entries: [] });
  });
});

describe('a see-all page', () => {
  it('asks for the kind and page it shows and keeps the page count', async () => {
    const { result } = renderHook(() => useDiscoverListPage('top-rated', 'tv', 2, true), {
      wrapper,
    });

    await waitFor(() => expect(result.current.entries).toEqual([ENTRY]));

    expect(asked()[0]).toMatch(/\/api\/discover\/top-rated\?type=tv&page=2$/);
    expect(result.current.totalPages).toBe(4);
  });

  it('reads a genre the same way', async () => {
    const { result } = renderHook(() => useGenrePage(28, 'movie', 1, true), { wrapper });

    await waitFor(() => expect(result.current.entries).toEqual([ENTRY]));

    expect(asked()[0]).toMatch(/\/api\/discover\/genre\/28\?type=movie$/);
    expect(result.current.totalPages).toBe(2);
  });
});

describe('the genres of a kind', () => {
  it('lists them once enabled', async () => {
    const { result } = renderHook(() => useGenres('movie', true), { wrapper });

    await waitFor(() => expect(result.current.genres).toEqual([{ id: 28, name: 'Action' }]));
  });

  it('asks for none while disabled', () => {
    renderHook(() => useGenres('movie', false), { wrapper });

    expect(fetched).not.toHaveBeenCalled();
  });
});
