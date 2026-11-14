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
