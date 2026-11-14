// The browse-first empty state: the TMDB lists as home-style rails and the
// genres as chips, so the discovery page is a place to browse, not just a
// search box. Filtered by the active type chip.

import type { DiscoverEntry, DiscoverType, TmdbKind } from '@kromatv/client/discovery';
import { useT } from '@kromatv/ui';
import { Box, Chip, classes, Icon, Row, styles, Text, useStyles } from '@kromatv/ui/kit';
import type { IconName } from '@kromatv/ui/kit';
import { Link } from '@tanstack/react-router';
import type { ReactNode } from 'react';
import { DiscoverCard } from '#web/features/requests/discover-card';
import { BROWSE_LISTS, type BrowseList } from '#web/features/requests/discover-lists';
import { useDiscoverRail, useGenres } from '#web/features/requests/use-discover-lists';
import { PosterRail, SkeletonRow } from '#web/shared/ui';
import { RouteLink } from '#web/shared/ui/route-link';

const CAPTION_H = 52;

const KINDS: readonly { type: TmdbKind; kind: DiscoverEntry['kind'] }[] = [
  { type: 'movie', kind: 'movie' },
  { type: 'tv', kind: 'show' },
];

function RailHeading({
  title,
  icon,
  action,
}: Readonly<{ title: string; icon: IconName; action?: ReactNode }>) {
  const s = useStyles(sDefinition);

  return (
    <Row between gap={12} mt={36} mb={16}>
      <h2 className={classes(s.heading)}>
        <Row gap={8}>
          <Icon name={icon} size={20} thickness={2} color="accent" />
          <Text variant="h2">{title}</Text>
        </Row>
      </h2>
      {action}
    </Row>
  );
}

function ListRail({
  meta,
  type,
  entries,
}: Readonly<{ meta: BrowseList; type: TmdbKind; entries: DiscoverEntry[] }>) {
  const s = useStyles(sDefinition);

  const t = useT();
  if (entries.length === 0) return null;
  return (
    <section>
      <RailHeading
        title={t(meta.titleKeys[type])}
        icon={meta.icon}
        action={
          <Link
            to="/browse/$list/$type"
            params={{ list: meta.list, type }}
            className={classes(s.inlineLink)}
          >
            <Text variant="meta" color="textDim">
              {t('discover.seeAll')}
            </Text>
            <Icon name="chevron-right" size={15} thickness={2.4} color="textDim" />
          </Link>
        }
      />
      <PosterRail
        data={entries}
        extraHeight={CAPTION_H}
        renderItem={(e) => <DiscoverCard entry={e} />}
      />
    </section>
  );
}

function ListRails({ meta, type }: Readonly<{ meta: BrowseList; type: DiscoverType }>) {
  const t = useT();
  const { entries, loading } = useDiscoverRail(meta.list, true);

  if (loading) {
    return (
      <Box>
        <RailHeading title={t(meta.titleKeys.movie)} icon={meta.icon} />
        <SkeletonRow />
      </Box>
    );
  }
  return (
    <>
      {KINDS.filter((k) => type === 'all' || type === k.type).map((k) => (
        <ListRail
          key={k.type}
          meta={meta}
          type={k.type}
          entries={entries.filter((e) => e.kind === k.kind)}
        />
      ))}
    </>
  );
}

function GenreChips({ type }: Readonly<{ type: TmdbKind }>) {
  const s = useStyles(sDefinition);

  const t = useT();
  const { genres } = useGenres(type, true);
  if (genres.length === 0) return null;
  return (
    <section>
      <RailHeading
        title={t(type === 'movie' ? 'discover.genresMovies' : 'discover.genresShows')}
        icon="category"
      />
      <div className={classes(s.chips)}>
        {genres.map((g) => (
          <Chip key={g.id} variant="surface" label={g.name} asChild>
            <RouteLink to="/genre/$type/$genreId" params={{ type, genreId: String(g.id) }} />
          </Chip>
        ))}
      </div>
    </section>
  );
}

export function DiscoverBrowse({ type }: Readonly<{ type: DiscoverType }>) {
  const s = useStyles(sDefinition);

  return (
    <div className={classes(s.fadeIn)}>
      {KINDS.filter((k) => type === 'all' || type === k.type).map((k) => (
        <GenreChips key={k.type} type={k.type} />
      ))}
      {BROWSE_LISTS.map((meta) => (
        <ListRails key={meta.list} meta={meta} type={type} />
      ))}
    </div>
  );
}

const sDefinition = styles({
  heading: { m: 0 },
  chips: { display: 'flex', flexWrap: 'wrap', gap: 8 },
  inlineLink: {
    display: 'inline-flex',
    alignItems: 'center',
    gap: 4,
    flexShrink: 0,
    textDecorationLine: 'none',
    borderRadius: 'var(--radius-sm)',
  },
  fadeIn: {
    animationKeyframes: 'fade-in',
    animationDuration: '300ms',
    animationTimingFunction: 'var(--ease-out)',
  },
});
