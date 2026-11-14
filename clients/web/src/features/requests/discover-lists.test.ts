import { DiscoverList } from '@kromatv/client/discovery';
import { describe, expect, it } from 'vitest';
import { BROWSE_LISTS, browseListOf } from '#web/features/requests/discover-lists';

describe('the browsable lists', () => {
  it('cover every list the server names, once each', () => {
    expect(BROWSE_LISTS.map((l) => l.list).toSorted()).toEqual([...DiscoverList.options].toSorted());
  });

  it('give a movie title and a show title to each list', () => {
    for (const meta of BROWSE_LISTS) {
      expect(meta.titleKeys.movie).not.toEqual(meta.titleKeys.tv);
    }
  });

  it('find a list by its path name and nothing else', () => {
    expect(browseListOf('top-rated')?.list).toBe('top-rated');
    expect(browseListOf('nonsense')).toBeUndefined();
  });
});
