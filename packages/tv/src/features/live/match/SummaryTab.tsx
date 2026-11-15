import type { MatchView } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Box, Text } from '@kromatv/ui/kit';
import { feedLine } from '#tv/features/live/match/matchText';

const FEED_ROWS = 8;

export function SummaryTab({ view }: Readonly<{ view: MatchView }>) {
  const t = useT();
  return (
    <Box gap={14}>
      {view.best.length > 0 ? (
        <Box gap={6}>
          <Text variant="overlineTv" color="accentText">
            {t('live.bestPlayers')}
          </Text>
          {view.best.map((entry) => (
            <Box key={entry.player.id} row justify="space-between">
              <Text variant="bodyTv">{entry.player.nom}</Text>
              <Text variant="labelTv">{entry.rating.toFixed(1)}</Text>
            </Box>
          ))}
        </Box>
      ) : null}
      <Text variant="overlineTv" color="accentText">
        {t('live.matchFeed')}
      </Text>
      {view.feed.slice(0, FEED_ROWS).map((item) => (
        <Box key={item.id} row gap={14}>
          <Text variant="labelTv" color="accentText">
            {item.minute}
          </Text>
          <Text variant="bodyTv" flex color={item.side === 'dom' ? 'text' : 'info'}>
            {feedLine(item, t)}
          </Text>
        </Box>
      ))}
    </Box>
  );
}
