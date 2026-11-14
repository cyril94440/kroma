import type { Query, RequestContext } from '../../core/client';
import {
  DiscoverDetail,
  DiscoverGenres,
  type DiscoverList,
  DiscoverResponse,
  DiscoverSeasonEpisodes,
  type DiscoverType,
  type TmdbKind,
} from './schemas';

/** What a discovery listing asks for. `all` is the absence of a type filter and
 * page one is the absence of a page, which is how the server reads them. */
export interface DiscoverPage {
  type?: DiscoverType;
  page?: number;
}

const NARROWED: Readonly<Record<DiscoverType, string | undefined>> = {
  movie: 'movie',
  tv: 'tv',
  all: undefined,
};

const beyondFirst = (page = 1) => (page > 1 ? page : undefined);

export function discoverQuery({ type = 'all', page }: DiscoverPage = {}): Query {
  return { type: NARROWED[type], page: beyondFirst(page) };
}

/** TMDB discovery for the request flow: search titles the library may not have,
 * the browsable lists and genres for the empty state, and a title detail with
 * its season list. */
export default function discoveryApi(ctx: RequestContext) {
  return {
    search: (query: string, opts?: DiscoverPage) =>
      ctx.get('/discover/search', DiscoverResponse, {
        query: { q: query, ...discoverQuery(opts) },
      }),

    list: (list: DiscoverList, opts?: DiscoverPage) =>
      ctx.get('/discover/:list', DiscoverResponse, {
        params: { list },
        query: discoverQuery(opts),
        concurrency: 'share',
      }),

    /** The genres of one kind; `all` has none, so the caller names movie or tv. */
    genres: (type: TmdbKind) => ctx.get('/discover/genres', DiscoverGenres, { query: { type } }),

    genre: (genreId: number, type: TmdbKind, page?: number) =>
      ctx.get('/discover/genre/:genreId', DiscoverResponse, {
        params: { genreId },
        query: { type, page: beyondFirst(page) },
        concurrency: 'share',
      }),

    /** One title's request-flow detail. `kind` follows the route vocabulary
     * (`movie` | `tv`); the response speaks the catalog's (`movie` | `show`). */
    detail: (kind: TmdbKind, tmdbId: number) =>
      ctx.get('/discover/:kind/:tmdbId', DiscoverDetail, { params: { kind, tmdbId } }),

    season: (tmdbId: number, season: number) =>
      ctx.get('/discover/tv/:tmdbId/season/:season', DiscoverSeasonEpisodes, {
        params: { tmdbId, season },
      }),
  };
}

declare module '../../core/client' {
  interface Domains {
    discovery: ReturnType<typeof discoveryApi>;
  }
}
