import type { WatchNowState } from '@kromatv/core/react';
import { useT } from '@kromatv/ui';
import { Box, Button, Text } from '@kromatv/ui/kit';

/** The "Watch now" button of a title page and what it says about its progress.
 * Searching shows a spinner and words, never a percentage: nobody knows how far
 * along it is. */
export function WatchNowCta({
  state,
  onWatch,
}: Readonly<{ state: WatchNowState; onWatch: () => void }>) {
  const t = useT();
  switch (state.phase) {
    case 'searching':
    case 'ready':
      return <Button icon="player-play" label={t('discover.watchNowSearching')} loading />;
    case 'requested':
      return (
        <Text variant="label" color="white/70">
          {t('discover.watchNowRequested')}
        </Text>
      );
    case 'unavailable':
      return (
        <Text variant="label" color="white/70">
          {state.message}
        </Text>
      );
    case 'error':
      return (
        <Box gap={8}>
          <Button icon="player-play" label={t('discover.watchNow')} onPress={onWatch} />
          <Text variant="label" color="dangerHover">
            {t('discover.watchNowError')}
          </Text>
        </Box>
      );
    default:
      return <Button icon="player-play" label={t('discover.watchNow')} onPress={onWatch} />;
  }
}

/** Whether the title page should leave the request button out: the title was
 * just requested through watch now, or the server said why it cannot play. */
export function watchNowSettled(state: WatchNowState): boolean {
  return state.phase === 'requested' || state.phase === 'unavailable';
}
