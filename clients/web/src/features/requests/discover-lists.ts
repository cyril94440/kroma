import type { DiscoverList, TmdbKind } from '@kromatv/client/discovery';
import type { IconName } from '@kromatv/ui/kit';

export interface BrowseList {
  list: DiscoverList;
  icon: IconName;
  titleKeys: Record<TmdbKind, DiscoverListTitleKey>;
}

export type DiscoverListTitleKey =
  | 'discover.trendingMovies'
  | 'discover.trendingShows'
  | 'discover.popularMovies'
  | 'discover.popularShows'
  | 'discover.upcomingMovies'
  | 'discover.upcomingShows'
  | 'discover.topRatedMovies'
  | 'discover.topRatedShows';

export const BROWSE_LISTS: readonly BrowseList[] = [
  {
    list: 'trending',
    icon: 'flame',
    titleKeys: { movie: 'discover.trendingMovies', tv: 'discover.trendingShows' },
  },
  {
    list: 'popular',
    icon: 'trending-up',
    titleKeys: { movie: 'discover.popularMovies', tv: 'discover.popularShows' },
  },
  {
    list: 'upcoming',
    icon: 'calendar-event',
    titleKeys: { movie: 'discover.upcomingMovies', tv: 'discover.upcomingShows' },
  },
  {
    list: 'top-rated',
    icon: 'star',
    titleKeys: { movie: 'discover.topRatedMovies', tv: 'discover.topRatedShows' },
  },
];

export function browseListOf(list: string): BrowseList | undefined {
  return BROWSE_LISTS.find((l) => l.list === list);
}
