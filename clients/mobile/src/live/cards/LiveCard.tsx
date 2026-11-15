import type { ChartCard, MatchCard, TableCard } from '@kromatv/client/party';
import { chartBars, parseCard, rowTone } from '@kromatv/core';
import { Badge, Box, Button, Text } from '@kromatv/ui/kit';
import { useT } from '#mobile/lib/i18n';

const BAR_HEIGHT = 90;

function TableView({ card }: Readonly<{ card: TableCard }>) {
  return (
    <Box gap={6} p={14} radius="lg" bg="surface2">
      {card.title ? <Text variant="label">{card.title}</Text> : null}
      <Box row gap={8}>
        {card.columns.map((column) => (
          <Box key={column} flex>
            <Text variant="meta" color="textMuted">
              {column}
            </Text>
          </Box>
        ))}
      </Box>
      {card.rows.map((row, at) => (
        <Box key={row.join('|')} row gap={8} bg={rowTone(card, at) ? 'accentSoft' : undefined}>
          {row.map((cell, column) => (
            <Box key={card.columns[column]} flex>
              <Text variant="body">{String(cell)}</Text>
            </Box>
          ))}
        </Box>
      ))}
      {card.note ? (
        <Text variant="meta" color="textDim">
          {card.note}
        </Text>
      ) : null}
    </Box>
  );
}

function ChartView({ card }: Readonly<{ card: ChartCard }>) {
  const t = useT();
  const bars = chartBars(card.points);
  const unit = card.unit ? ` ${card.unit}` : '';
  const label = t('live.card.chart', {
    title: card.title ?? '',
    from: `${card.points[0]?.value ?? ''}${unit}`,
    to: `${card.points.at(-1)?.value ?? ''}${unit}`,
  });
  return (
    <Box gap={6} p={14} radius="lg" bg="surface2" accessibilityLabel={label} accessible>
      {card.title ? <Text variant="label">{card.title}</Text> : null}
      <Box row gap={3} h={BAR_HEIGHT} align="flex-end">
        {bars.map((bar) => (
          <Box
            key={bar.label}
            flex
            bg="accent"
            style={{ height: `${Math.max(bar.ratio, 0.04) * 100}%` }}
          />
        ))}
      </Box>
      <Box row justify="space-between">
        <Text variant="meta" color="textDim">
          {card.points[0]?.label}
        </Text>
        <Text variant="meta" color="textDim">
          {card.points.at(-1)?.label}
        </Text>
      </Box>
    </Box>
  );
}

function MatchView({
  card,
  onWatch,
}: Readonly<{ card: MatchCard; onWatch: (channel: string) => void }>) {
  const t = useT();
  const channel = card.channel;
  return (
    <Box gap={8} p={14} radius="lg" bg="surface2">
      <Box row justify="space-between" align="center">
        <Text variant="meta" color="textMuted">
          {card.competition ?? ''}
        </Text>
        {card.status ? <Badge>{t(`live.card.${card.status}`)}</Badge> : null}
      </Box>
      <Box row justify="space-between" align="center">
        <Text variant="body">{card.home}</Text>
        <Text variant="title">{card.score ?? 'vs'}</Text>
        <Text variant="body">{card.away}</Text>
      </Box>
      {card.action && channel ? (
        <Button
          icon="device-tv"
          variant="scrim"
          label={card.action.label}
          onPress={() => onWatch(channel)}
        />
      ) : null}
    </Box>
  );
}

/** The card Hermès attached to a message, drawn from the closed catalogue; a
 *  card outside it draws nothing and the message text stays. */
export function LiveCard({
  card,
  onWatch,
}: Readonly<{ card: unknown; onWatch: (channel: string) => void }>) {
  const parsed = parseCard(card);
  if (!parsed) return null;
  if (parsed.kind === 'table') return <TableView card={parsed} />;
  if (parsed.kind === 'chart') return <ChartView card={parsed} />;
  return <MatchView card={parsed} onWatch={onWatch} />;
}
