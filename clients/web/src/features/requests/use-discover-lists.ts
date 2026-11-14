import type {
  DiscoverEntry,
  DiscoverGenre,
  DiscoverList,
  TmdbKind,
} from '@kromatv/client/discovery';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useWebRuntime } from '#web/shared/lib/web-runtime';

export interface DiscoverRailState {
  loading: boolean;
  entries: DiscoverEntry[];
}

/** Page one of a list, movies and shows together, for a rail on the
 * browse-first empty state. Fetched once when `enabled`. */
export function useDiscoverRail(list: DiscoverList, enabled: boolean): DiscoverRailState {
  const { queries } = useWebRuntime();

  const { data, isFetching } = useQuery({
    ...queries.discoverQueries.list(list, 'all', 1),
    enabled,
    select: (r) => r.results,
  });
  return { loading: enabled && isFetching && data === undefined, entries: data ?? [] };
}

export interface DiscoverPageState {
  loading: boolean;
  entries: DiscoverEntry[];
  totalPages: number;
}

/** One page of a list for a single kind. `keepPreviousData` retains the prior
 * page (and its total) while the next loads. */
export function useDiscoverListPage(
  list: DiscoverList,
  type: TmdbKind,
  page: number,
  enabled: boolean,
): DiscoverPageState {
  const { queries } = useWebRuntime();

  const found = useQuery({
    ...queries.discoverQueries.list(list, type, page),
    enabled,
    placeholderData: keepPreviousData,
  });
  return pageState(found.data, enabled && found.isFetching);
}

/** One page of the titles filed under a genre. */
export function useGenrePage(
  genreId: number,
  type: TmdbKind,
  page: number,
  enabled: boolean,
): DiscoverPageState {
  const { queries } = useWebRuntime();

  const found = useQuery({
    ...queries.discoverQueries.genre(genreId, type, page),
    enabled,
    placeholderData: keepPreviousData,
  });
  return pageState(found.data, enabled && found.isFetching);
}

export interface GenresState {
  loading: boolean;
  genres: DiscoverGenre[];
}

/** The genres of one kind. They change with the language and almost never
 * otherwise, so they are held for the visit. */
export function useGenres(type: TmdbKind, enabled: boolean): GenresState {
  const { queries } = useWebRuntime();

  const { data, isFetching } = useQuery({
    ...queries.discoverQueries.genres(type),
    enabled,
    staleTime: Infinity,
    select: (r) => r.genres,
  });
  return { loading: enabled && isFetching && data === undefined, genres: data ?? [] };
}

function pageState(
  data: { results: DiscoverEntry[]; totalPages: number } | undefined,
  loading: boolean,
): DiscoverPageState {
  return { loading, entries: data?.results ?? [], totalPages: data?.totalPages ?? 1 };
}
