// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { firstEpisodeOf } from './WatchNowButton';

describe('firstEpisodeOf', () => {
  it('names nothing for a movie', () => {
    expect(firstEpisodeOf('movie', undefined)).toBeUndefined();
  });

  it('starts a show at the first episode of its first real season', () => {
    expect(firstEpisodeOf('show', [{ season: 0 }, { season: 1 }, { season: 2 }])).toEqual({
      season: 1,
      episode: 1,
    });
  });

  it('falls back to the first season it lists, or to season one when it lists none', () => {
    expect(firstEpisodeOf('show', [{ season: 0 }])).toEqual({ season: 0, episode: 1 });
    expect(firstEpisodeOf('show', [])).toEqual({ season: 1, episode: 1 });
    expect(firstEpisodeOf('show', undefined)).toEqual({ season: 1, episode: 1 });
  });
});
