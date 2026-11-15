import { useT } from '@kromatv/module-sdk';
import { Box, Row, Text } from '@kromatv/ui/kit';
import type { FeedItem, MatchView } from './match-calc';
import type { MatchPayload } from './schemas';

function line(item: FeedItem, t: ReturnType<typeof useT>): string {
  switch (item.kind) {
    case 'but':
      return [
        t(item.ownGoal ? 'sport.feed.ownGoal' : 'sport.feed.goal'),
        item.player,
        item.assist ? `(${item.assist})` : null,
      ]
        .filter(Boolean)
        .join(' ');
    case 'remplacement':
      return `${t('sport.feed.substitution')} ${item.cameOn ?? '?'} / ${item.wentOff ?? '?'}`;
    case 'jaune':
      return `${t('sport.feed.yellow')} ${item.player ?? ''}`;
    default:
      return `${t('sport.feed.red')} ${item.player ?? ''}`;
  }
}

interface MatchSummaryProps {
  match: MatchPayload;
  view: MatchView;
  limit?: number;
}

export function MatchSummary({ match, view, limit }: Readonly<MatchSummaryProps>) {
  const t = useT();
  const items = limit ? view.feed.slice(0, limit) : view.feed;
  return (
    <Box data-testid="sport-summary">
      {view.best.length > 0 ? (
        <Box mb={12}>
          <Text variant="meta" color="textDim">
            {t('sport.bestPlayers')}
          </Text>
          {view.best.map((entry) => (
            <Row key={entry.player.id} between>
              <Text variant="body">{entry.player.nom}</Text>
              <Text variant="label">{entry.rating.toFixed(1)}</Text>
            </Row>
          ))}
        </Box>
      ) : null}
      {items.map((item) => (
        <Row key={item.id} gap={8} mb={4}>
          <Text variant="meta" color="textDim">
            {item.minute}
          </Text>
          <Text variant="body" color={item.side === 'dom' ? 'accentText' : 'info'}>
            {line(item, t)}
          </Text>
        </Row>
      ))}
      <Text variant="meta" color="textDim">
        {match.chaine}
      </Text>
    </Box>
  );
}
