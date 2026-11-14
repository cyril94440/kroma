// A paginated grid of one TMDB list for movies OR shows, or of one genre,
// reached from the discover rails. TMDB-gated on `requests.create`.

import { hasPermission } from '@kromatv/client/accounts';
import type { DiscoverList, TmdbKind } from '@kromatv/client/discovery';
import { useT } from '@kromatv/ui';
import {
  classes,
  EmptyState,
  Icon,
  type IconName,
  PageHeader,
  styles,
  Text,
  useStyles,
} from '@kromatv/ui/kit';
import { Link } from '@tanstack/react-router';
import { type ReactNode, useState } from 'react';
import { DiscoverGrid, DiscoverPager } from '#web/features/requests/discover-grid';
import { browseListOf } from '#web/features/requests/discover-lists';
import {
  type DiscoverPageState,
  useDiscoverListPage,
  useGenrePage,
  useGenres,
} from '#web/features/requests/use-discover-lists';
import { useAuth } from '#web/shared/lib/auth';
import { PageFrame } from '#web/shared/ui';

const sDefinition = styles({
  backLink: {
    display: 'inline-flex',
    alignItems: 'center',
    alignSelf: 'flex-start',
    gap: 6,
    mb: 24,
    textDecorationLine: 'none',
  },
});

function PagedFrame({
  title,
  icon,
  state,
  page,
  onPage,
  canDiscover,
}: Readonly<{
  title: ReactNode;
  icon: IconName;
  state: DiscoverPageState;
  page: number;
  onPage: (page: number) => void;
  canDiscover: boolean;
}>) {
  const s = useStyles(sDefinition);

  const t = useT();
  const go = (next: number) => {
    onPage(Math.min(Math.max(1, next), state.totalPages));
    window.scrollTo({ top: 0 });
  };

  return (
    <PageFrame>
      <Link to="/search" search={{ q: '', type: 'all' }} className={classes(s.backLink)}>
        <Icon name="arrow-left" size={16} thickness={2.2} color="textDim" />
        <Text variant="meta" color="textDim">
          {t('discover.back')}
        </Text>
      </Link>

      <PageHeader.Root>
        <PageHeader.Title icon={icon}>{title}</PageHeader.Title>
      </PageHeader.Root>

      {!canDiscover ? (
        <EmptyState.Root icon="mood-empty">
          <EmptyState.Title>{t('discover.empty')}</EmptyState.Title>
        </EmptyState.Root>
      ) : (
        <>
          <DiscoverGrid state={state} />
          <DiscoverPager page={page} totalPages={state.totalPages} onGo={go} />
        </>
      )}
    </PageFrame>
  );
}

function useCanDiscover() {
  const { user } = useAuth();
  return !!user && hasPermission(user, 'requests.create');
}

export function DiscoverListPage({ list, type }: Readonly<{ list: DiscoverList; type: TmdbKind }>) {
  const t = useT();
  const canDiscover = useCanDiscover();
  const [page, setPage] = useState(1);
  const state = useDiscoverListPage(list, type, page, canDiscover);
  const meta = browseListOf(list);

  return (
    <PagedFrame
      title={meta ? t(meta.titleKeys[type]) : t('discover.title')}
      icon={meta?.icon ?? 'flame'}
      state={state}
      page={page}
      onPage={setPage}
      canDiscover={canDiscover}
    />
  );
}

export function GenrePage({ genreId, type }: Readonly<{ genreId: number; type: TmdbKind }>) {
  const t = useT();
  const canDiscover = useCanDiscover();
  const [page, setPage] = useState(1);
  const state = useGenrePage(genreId, type, page, canDiscover);
  const { genres } = useGenres(type, canDiscover);
  const name = genres.find((g) => g.id === genreId)?.name ?? t('discover.genre');

  return (
    <PagedFrame
      title={name}
      icon="category"
      state={state}
      page={page}
      onPage={setPage}
      canDiscover={canDiscover}
    />
  );
}
