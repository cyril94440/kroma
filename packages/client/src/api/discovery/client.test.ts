import { describe, expect, it } from 'vitest';
import { checkEndpoint, type Endpoint } from '../../endpoints.fixture';
import { recordingClient } from '../../kroma-client.fixture';

describe('the discovery endpoints', () => {
  it.each<Endpoint>([
    {
      name: 'search',
      call: (c) => c.discovery.search('dune', { type: 'movie', page: 2 }),
      method: 'GET',
      path: '/discover/search?q=dune&type=movie&page=2',
    },
    {
      name: 'search drops the all filter and page one',
      call: (c) => c.discovery.search('dune', { type: 'all', page: 1 }),
      method: 'GET',
      path: '/discover/search?q=dune',
    },
    {
      name: 'trending',
      call: (c) => c.discovery.list('trending'),
      method: 'GET',
      path: '/discover/trending',
    },
    {
      name: 'a list narrowed to one kind and page',
      call: (c) => c.discovery.list('top-rated', { type: 'tv', page: 3 }),
      method: 'GET',
      path: '/discover/top-rated?type=tv&page=3',
    },
    {
      name: 'genres',
      call: (c) => c.discovery.genres('movie'),
      method: 'GET',
      path: '/discover/genres?type=movie',
    },
    {
      name: 'a genre',
      call: (c) => c.discovery.genre(28, 'movie', 2),
      method: 'GET',
      path: '/discover/genre/28?type=movie&page=2',
    },
    {
      name: 'detail',
      call: (c) => c.discovery.detail('tv', 1399),
      method: 'GET',
      path: '/discover/tv/1399',
    },
    {
      name: 'watch now for a movie',
      call: (c) => c.discovery.watchNow('movie', 603),
      method: 'POST',
      path: '/discover/movie/603/watch-now',
      body: {},
    },
    {
      name: 'watch now for one episode',
      call: (c) => c.discovery.watchNow('tv', 1399, { season: 2, episode: 3 }),
      method: 'POST',
      path: '/discover/tv/1399/watch-now',
      body: { season: 2, episode: 3 },
    },
    {
      name: 'season',
      call: (c) => c.discovery.season(1399, 2),
      method: 'GET',
      path: '/discover/tv/1399/season/2',
    },
  ])('$name', checkEndpoint);
});

describe('the genre list', () => {
  it('reads the genres a server names', async () => {
    const { client } = recordingClient(() => ({ json: { genres: [{ id: 28, name: 'Action' }] } }));

    await expect(client.discovery.genres('movie')).resolves.toEqual({
      genres: [{ id: 28, name: 'Action' }],
    });
  });

  it('refuses a genre without a name', async () => {
    const { client } = recordingClient(() => ({ json: { genres: [{ id: 28 }] } }));

    await expect(client.discovery.genres('movie')).rejects.toThrow();
  });
});

describe('a season of a show', () => {
  it('reads an episode TMDB knows nothing about beyond its number', async () => {
    const bare = { episode: 2, name: null, overview: null, stillUrl: null, airDate: null };
    const { client } = recordingClient(() => ({ json: { episodes: [bare] } }));

    await expect(client.discovery.season(1399, 2)).resolves.toEqual({
      episodes: [{ episode: 2, title: null, overview: null, stillUrl: null, airDate: null }],
    });
  });

  it('reads the runtime the provider gives an episode', async () => {
    const timed = { episode: 2, name: 'Lord Snow', stillUrl: null, airDate: null, runtimeMin: 48 };
    const { client } = recordingClient(() => ({ json: { episodes: [timed] } }));

    const { episodes } = await client.discovery.season(1399, 2);

    expect(episodes[0]).toMatchObject({ episode: 2, runtimeMin: 48 });
  });

  it('reads the empty strings a server before the change sends as nothing at all', async () => {
    const older = { episode: 2, name: '', overview: '', stillUrl: null, airDate: null };
    const { client } = recordingClient(() => ({ json: { episodes: [older] } }));

    await expect(client.discovery.season(1399, 2)).resolves.toEqual({
      episodes: [{ episode: 2, title: null, overview: null, stillUrl: null, airDate: null }],
    });
  });
});

describe('watching a title now', () => {
  it.each([
    { status: 'ready', itemId: 'm1', fileId: 'f1' },
    { status: 'ready', itemId: 'm1' },
    { status: 'requested', requestId: 'r1' },
    { status: 'requested', requestId: null },
    { status: 'unavailable', reason: 'noRelease', message: 'Nothing fits.', requestId: 'r1' },
  ])('reads %j', async (answer) => {
    const { client } = recordingClient(() => ({ json: answer }));

    await expect(client.discovery.watchNow('movie', 603)).resolves.toMatchObject(answer);
  });

  it('refuses an answer in a state it does not know', async () => {
    const { client } = recordingClient(() => ({ json: { status: 'playing' } }));

    await expect(client.discovery.watchNow('movie', 603)).rejects.toThrow();
  });

  it('refuses a refusal with no reason it knows', async () => {
    const { client } = recordingClient(() => ({
      json: { status: 'unavailable', reason: 'tomorrow', message: 'x' },
    }));

    await expect(client.discovery.watchNow('movie', 603)).rejects.toThrow();
  });
});

describe('the detail of a title', () => {
  const detail = {
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
    tagline: null,
    genres: [],
    runtimeMin: 136,
    seasons: [],
    cast: [],
    crew: [],
    similar: [],
    airStatus: null,
    nextAirDate: null,
  };

  it('offers watch now only when the server says so', async () => {
    const { client } = recordingClient(() => ({ json: { ...detail, watchNow: true } }));

    await expect(client.discovery.detail('movie', 603)).resolves.toMatchObject({ watchNow: true });
  });

  it('reads a server before the feature as not offering it', async () => {
    const { client } = recordingClient(() => ({ json: detail }));

    await expect(client.discovery.detail('movie', 603)).resolves.toMatchObject({ watchNow: false });
  });
});
