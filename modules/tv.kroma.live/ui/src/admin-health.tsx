import { useT } from '@kromatv/module-sdk';
import { Badge, Button, ListRow, Row, Section, Text } from '@kromatv/ui/kit';
import type { LiveApi } from './live-api';
import type { History, Metrics } from './schemas';

interface AdminHealthProps {
  api: LiveApi;
  history: History;
  metrics: Metrics;
  onChanged: () => void;
}

export function AdminHealth({ api, history, metrics, onChanged }: Readonly<AdminHealthProps>) {
  const t = useT();
  const last = history.points.at(-1);
  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('live.admin.health')}</Section.Title>
      </Section.Header>
      <Row gap={8} wrap>
        <Badge>{t('live.admin.kbps', { kbps: last?.kbps ?? 0 })}</Badge>
        <Badge>{t('live.admin.zaps', { count: metrics.zaps })}</Badge>
        {metrics.p50Ms !== null ? <Badge>{t('live.admin.zapP50', { ms: metrics.p50Ms })}</Badge> : null}
        {metrics.p95Ms !== null ? <Badge>{t('live.admin.zapP95', { ms: metrics.p95Ms })}</Badge> : null}
        <Badge>{t('live.admin.restarts', { count: metrics.restartsTotal })}</Badge>
      </Row>
      <Row gap={8} mt={8}>
        <Button label={t('live.admin.pause')} variant="glass" size="sm" onPress={() => void api.pause().then(onChanged)} />
        <Button label={t('live.admin.resume')} variant="glass" size="sm" onPress={() => void api.resume().then(onChanged)} />
      </Row>
      {history.providerEvents.map((event) => (
        <ListRow.Root key={event.id}>
          <ListRow.Label>{t(`live.event.${event.kind}`)}</ListRow.Label>
          <ListRow.Trailing>
            <Text variant="meta" color="textDim">
              {event.channel ?? ''} {new Date(event.at).toLocaleTimeString()}
            </Text>
          </ListRow.Trailing>
        </ListRow.Root>
      ))}
    </Section.Root>
  );
}
