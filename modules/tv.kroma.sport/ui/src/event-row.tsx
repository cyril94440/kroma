import { useT } from '@kromatv/module-sdk';
import { Badge, Box, Button, Row, Text } from '@kromatv/ui/kit';
import type { SportEvent } from './schemas';
import { startsAt } from './when';

interface EventRowProps {
  event: SportEvent;
  now: number;
  ownScore: string | null;
  onWatch: (channelId: string) => void;
}

function certaintyKey(event: SportEvent): string {
  const channel = event.chaine;
  if (!channel) return 'sport.channel.unknown';
  if (channel.source === 'image' && channel.certitude === 'vu') return 'sport.channel.seen';
  if (channel.source === 'image' || channel.source === 'web') return 'sport.channel.toConfirm';
  return channel.etrangere ? 'sport.channel.foreignGuide' : 'sport.channel.guide';
}

function timeLabel(event: SportEvent, now: number): string {
  if (event.statut === 'pre') return startsAt(event.debut, now);
  return event.masquer_score ? '' : event.temps;
}

function scoreOf(event: SportEvent, index: number, ownScore: string | null): string {
  if (event.statut === 'pre') return '';
  if (!event.masquer_score) return event.lignes[index]?.score ?? '';
  return ownScore?.split('-')[index] ?? '';
}

function Action({
  event,
  onWatch,
}: Readonly<{ event: SportEvent; onWatch: (channelId: string) => void }>) {
  const t = useT();
  const target = event.chaine?.ref;
  if (event.a_lecran) return <Badge tone="warning">{t('sport.onScreen')}</Badge>;
  if (!target) return null;
  return (
    <Button label={t('sport.watch')} size="sm" variant="glass" onPress={() => onWatch(target)} />
  );
}

export function EventRow({ event, now, ownScore, onWatch }: Readonly<EventRowProps>) {
  const t = useT();
  const channel = event.chaine;
  return (
    <Box mb={12} p={12} gap={4} bg="tint/6" radius={12} data-testid={`sport-event-${event.id}`}>
      <Row between>
        <Text variant="meta" color="textDim">
          {event.compet}
        </Text>
        <Text variant="meta" color="accentText">
          {timeLabel(event, now)}
        </Text>
      </Row>
      {event.lignes.map((line, index) => (
        <Row key={line.nom} between>
          <Text variant="body">{line.nom}</Text>
          <Text variant="label">{scoreOf(event, index, ownScore)}</Text>
        </Row>
      ))}
      <Row between wrap gap={8}>
        <Box>
          <Text variant="body">{channel?.nom ?? ''}</Text>
          <Text variant="meta" color={channel?.certitude === 'vu' ? 'success' : 'textDim'}>
            {t(certaintyKey(event))}
          </Text>
        </Box>
        <Action event={event} onWatch={onWatch} />
      </Row>
    </Box>
  );
}
