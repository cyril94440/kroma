import type { RequestKind } from '@kromatv/client/requests';
import { useT } from '@kromatv/ui';
import { Button } from '@kromatv/ui/kit';
import { useWatchNowPlay } from '#tv/features/catalog/discover/useWatchNowPlay';

/** The first episode to play for a show, or nothing for a movie. */
export function firstEpisodeOf(
  kind: RequestKind,
  seasons: readonly { season: number }[] | undefined,
): { season: number; episode: number } | undefined {
  if (kind !== 'show') return undefined;
  const season = seasons?.find((s) => s.season > 0) ?? seasons?.[0];
  return { season: season?.season ?? 1, episode: 1 };
}

export function WatchNowButton({
  kind,
  tmdbId,
  episode,
}: Readonly<{
  kind: RequestKind;
  tmdbId: number;
  episode: { season: number; episode: number } | undefined;
}>) {
  const t = useT();
  const { state, watch } = useWatchNowPlay({ kind, tmdbId });
  const working = state.phase === 'searching' || state.phase === 'ready';
  let label = t('discover.watchNow');
  if (working) label = t('discover.watchNowSearching');
  if (state.phase === 'requested') label = t('requests.newRequestExisting');
  return (
    <Button
      size="lg"
      autoFocus
      icon="player-play"
      loading={working}
      disabled={state.phase === 'requested'}
      label={label}
      onPress={() => watch(episode)}
    />
  );
}
