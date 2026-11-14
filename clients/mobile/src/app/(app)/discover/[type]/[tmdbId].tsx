import { hasPermission } from '@kromatv/client/accounts';
import { type DiscoverDetail, TmdbKind, tmdbKindOf } from '@kromatv/client/discovery';
import { formatRuntime, offersWatchNow, sizedImageUrl } from '@kromatv/core';
import { type TitleWatchNow, useAskForTitle } from '@kromatv/core/react';
import { Box, Button, styles, Text, useStyles } from '@kromatv/ui/kit';
import { useQuery } from '@tanstack/react-query';
import { Redirect, useLocalSearchParams } from 'expo-router';
import { useWindowDimensions } from 'react-native';
import Animated, { useAnimatedScrollHandler, useSharedValue } from 'react-native-reanimated';
import { DiscoverEpisodes } from '#mobile/components/DiscoverEpisodes';
import { TrailerButton } from '#mobile/components/TrailerButton';
import { CastRail, DetailHero } from '#mobile/components/detail';
import {
  ErrorBanner,
  ErrorView,
  ExpandableText,
  Loading,
  SectionTitle,
} from '#mobile/components/ui';
import { useT } from '#mobile/lib/i18n';
import { useGutters } from '#mobile/lib/layout';
import { routeParam } from '#mobile/lib/nav';
import { useClient, useSession } from '#mobile/lib/session';
import { spacing, type } from '#mobile/lib/theme';
import { useWatchNowAction } from '#mobile/lib/watch-now';

const HERO_W = 1280;

function AskAction({ detail }: Readonly<{ detail: DiscoverDetail }>) {
  const t = useT();
  const client = useClient();
  const { status, asking, failed, ask } = useAskForTitle(client, detail);
  return (
    <>
      <Button
        variant="outline"
        icon="download"
        loading={asking}
        disabled={status !== null}
        label={status ? t('requests.newRequestExisting') : t('discover.request')}
        onPress={ask}
      />
      {failed ? <ErrorBanner message={t('discover.requestFailed')} /> : null}
    </>
  );
}

function WatchNowAction({
  detail,
  watch,
}: Readonly<{ detail: DiscoverDetail; watch: TitleWatchNow }>) {
  const t = useT();
  const { state } = watch;
  const episode =
    detail.kind === 'show'
      ? { season: detail.seasons.find((s) => s.season > 0)?.season ?? 1, episode: 1 }
      : undefined;
  const working = state.phase === 'searching' || state.phase === 'ready';
  return (
    <>
      <Button
        icon="player-play"
        loading={working}
        disabled={state.phase === 'requested'}
        label={working ? t('discover.watchNowSearching') : t('discover.watchNow')}
        onPress={() => watch.watch(episode)}
      />
      {state.phase === 'requested' ? <Text>{t('discover.watchNowRequested')}</Text> : null}
      {state.phase === 'unavailable' ? <ErrorBanner message={state.message} /> : null}
      {state.phase === 'error' ? <ErrorBanner message={t('discover.watchNowError')} /> : null}
    </>
  );
}

export default function DiscoverRoute() {
  const params = useLocalSearchParams<{ type?: string; tmdbId?: string }>();
  const kind = TmdbKind.safeParse(routeParam(params.type));
  const tmdbId = Number(routeParam(params.tmdbId));
  if (!(kind.success && Number.isFinite(tmdbId))) return <Redirect href="/" />;
  return <DiscoverScreen kind={kind.data} tmdbId={tmdbId} />;
}

function episodeWatch(offered: boolean, watch: TitleWatchNow) {
  if (!offered) return undefined;
  return {
    busy: watch.state.phase === 'searching',
    onWatch: (season: number, episode: number) => watch.watch({ season, episode }),
  };
}

function DiscoverScreen({ kind, tmdbId }: Readonly<{ kind: TmdbKind; tmdbId: number }>) {
  const s = useStyles(sDefinition);

  const t = useT();
  const client = useClient();
  const { user } = useSession();
  const { width } = useWindowDimensions();
  const gutters = useGutters();
  const scrollY = useSharedValue(0);
  const onScroll = useAnimatedScrollHandler((e) => {
    scrollY.value = e.contentOffset.y;
  });
  const fiche = useQuery(client.query.discovery.detail(kind, tmdbId));
  const watch = useWatchNowAction({ kind: kind === 'tv' ? 'show' : 'movie', tmdbId });

  if (fiche.isPending) return <Loading label={t('common.loading')} />;
  if (fiche.isError)
    return (
      <ErrorView
        message={t('requests.ledgerFailed')}
        retryLabel={t('error.retry')}
        onRetry={() => fiche.refetch()}
      />
    );

  const detail = fiche.data;
  if (detail.localId) {
    const owned = detail.kind === 'show' ? `/show/${detail.localId}` : `/item/${detail.localId}`;
    return <Redirect href={owned as never} />;
  }

  const offered = offersWatchNow(user, detail.watchNow);
  const span = detail.seasons.length
    ? t('content.seasonCount', { count: detail.seasons.length })
    : formatRuntime((detail.runtimeMin ?? 0) * 60_000);
  const kindLabel = t(detail.kind === 'show' ? 'discover.kindShow' : 'discover.kindMovie');

  return (
    <Animated.ScrollView
      style={s.screen}
      contentContainerStyle={{ paddingBottom: spacing.xl }}
      onScroll={onScroll}
      scrollEventThrottle={16}
    >
      <DetailHero
        scrollY={scrollY}
        art={sizedImageUrl(
          client.media.artwork.resolve(detail.backdropUrl ?? detail.posterUrl),
          Math.min(HERO_W, width),
        )}
        seed={String(detail.tmdbId)}
        title={detail.title}
        context={`${kindLabel} · ${t('discover.notInLibrary')}`}
        meta={<DiscoverMeta detail={detail} span={span} />}
      />

      <Box style={[s.body, gutters.style]}>
        {offered ? <WatchNowAction detail={detail} watch={watch} /> : null}
        {user && hasPermission(user, 'requests.create') ? <AskAction detail={detail} /> : null}
        <TrailerButton kind={tmdbKindOf(detail.kind)} tmdbId={detail.tmdbId} title={detail.title} />
        {detail.overview ? <ExpandableText>{detail.overview}</ExpandableText> : null}
      </Box>

      {detail.kind === 'show' ? (
        <DiscoverEpisodes
          tmdbId={detail.tmdbId}
          seasons={detail.seasons}
          watch={episodeWatch(offered, watch)}
        />
      ) : null}

      {detail.cast.length > 0 ? (
        <Box>
          <SectionTitle>{t('content.cast')}</SectionTitle>
          <CastRail cast={detail.cast} />
        </Box>
      ) : null}
    </Animated.ScrollView>
  );
}

function DiscoverMeta({ detail, span }: Readonly<{ detail: DiscoverDetail; span: string }>) {
  const s = useStyles(sDefinition);

  return (
    <>
      {detail.year ? <Text style={s.metaText}>{detail.year}</Text> : null}
      {span ? <Text style={s.metaText}>{span}</Text> : null}
      {detail.genres[0] ? <Text style={s.metaText}>{detail.genres[0]}</Text> : null}
      {detail.rating ? <Text style={s.rating}>★ {detail.rating.toFixed(1)}</Text> : null}
    </>
  );
}

const sDefinition = styles({
  screen: { flex: true, bg: 'bg' },
  body: { gap: spacing.md, pt: spacing.md },
  metaText: { ...type.caption, color: 'text', fontWeight: '600' },
  rating: { ...type.caption, color: 'accentText', fontWeight: '700' },
});
