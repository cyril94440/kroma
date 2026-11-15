import type { MatchCard } from '@kromatv/client/party';
import { useT } from '@kromatv/ui';
import { Badge, Box, Button, Text } from '@kromatv/ui/kit';

export function MatchCardView({
  card,
  onWatch,
}: Readonly<{ card: MatchCard; onWatch: (channel: string) => void }>) {
  const t = useT();
  const channel = card.channel;
  return (
    <Box gap={10} p={20} radius="lg" bg="surface2">
      <Box row justify="space-between" align="center">
        {card.competition ? (
          <Text variant="labelTv" color="textMuted">
            {card.competition}
          </Text>
        ) : null}
        {card.status ? <Badge>{t(`live.card.${card.status}`)}</Badge> : null}
      </Box>
      <Box row justify="space-between" align="center">
        <Text variant="bodyTv">{card.home}</Text>
        <Text variant="headingTv">{card.score ?? 'vs'}</Text>
        <Text variant="bodyTv">{card.away}</Text>
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
