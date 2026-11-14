import { useT } from '@kromatv/module-sdk';
import { Badge, Button, Row, Section, Text } from '@kromatv/ui/kit';
import type { HermesApi } from './hermes-api';
import type { AssistantStatus } from './schemas';

interface StatusPanelProps {
  api: HermesApi;
  status: AssistantStatus;
  onChanged: () => void;
}

export function StatusPanel({ api, status, onChanged }: Readonly<StatusPanelProps>) {
  const t = useT();
  const running = status.enabled && !status.paused;
  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('hermes.status.title')}</Section.Title>
      </Section.Header>
      <Row gap={8} wrap>
        <Badge>{running ? t('hermes.status.active') : t('hermes.status.paused')}</Badge>
        <Badge>{status.busy ? t('hermes.status.busy') : t('hermes.status.idle')}</Badge>
        {status.configured ? null : <Badge>{t('hermes.status.notConfigured')}</Badge>}
      </Row>
      <Text variant="meta" color="textDim" mt={6}>
        {t('hermes.status.model', { provider: status.provider, model: status.model })}
      </Text>
      <Text variant="meta" color="textDim">
        {t('hermes.status.calls', { count: status.callsLastHour, limit: status.hourlyLimit })}
      </Text>
      <Row gap={8} mt={8}>
        <Button
          label={status.paused ? t('hermes.status.resume') : t('hermes.status.pause')}
          variant="glass"
          size="sm"
          onPress={() => void api.pause(!status.paused).then(onChanged)}
        />
      </Row>
    </Section.Root>
  );
}
