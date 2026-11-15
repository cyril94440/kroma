import type { FeedItem, MatchView } from '@kromatv/core';
import { Box, Text } from '@kromatv/ui/kit';
import { useT } from '#mobile/lib/i18n';

const FEED_ROWS = 10;

function line(item: FeedItem, t: ReturnType<typeof useT>): string {
  switch (item.kind) {
    case 'but':
      return [
        t(item.ownGoal ? 'live.feed.ownGoal' : 'live.feed.but'),
        item.player,
        item.assist ? `(${item.assist})` : null,
      ]
        .filter(Boolean)
        .join(' ');
    case 'remplacement':
      return `${t('live.feed.remplacement')} ${item.cameOn ?? '?'} / ${item.wentOff ?? '?'}`;
    case 'jaune':
      return `${t('live.feed.jaune')} ${item.player ?? ''}`.trim();
    default:
      return `${t('live.feed.rouge')} ${item.player ?? ''}`.trim();
  }
}

export function MatchSummary({ view }: Readonly<{ view: MatchView }>) {
  const t = useT();
  return (
    <Box gap={10}>
      {view.best.length > 0 ? (
        <Box gap={4}>
          <Text variant="meta" color="textDim">
            {t('live.bestPlayers')}
          </Text>
          {view.best.map((entry) => (
            <Box key={entry.player.id} row justify="space-between">
              <Text variant="body">{entry.player.nom}</Text>
              <Text variant="label">{entry.rating.toFixed(1)}</Text>
            </Box>
          ))}
        </Box>
      ) : null}
      <Text variant="meta" color="textDim">
        {t('live.matchFeed')}
      </Text>
      {view.feed.slice(0, FEED_ROWS).map((item) => (
        <Box key={item.id} row gap={8}>
          <Text variant="meta" color="textDim">
            {item.minute}
          </Text>
          <Text variant="body" flex color={item.side === 'dom' ? 'text' : 'info'}>
            {line(item, t)}
          </Text>
        </Box>
      ))}
    </Box>
  );
}
