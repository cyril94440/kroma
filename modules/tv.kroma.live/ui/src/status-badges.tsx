import { useT } from '@kromatv/module-sdk';
import { Badge, Row, Text } from '@kromatv/ui/kit';
import { badges } from './live-state';
import type { Status } from './schemas';

export function StatusBadges({ status }: Readonly<{ status: Status }>) {
  const t = useT();
  return (
    <Row gap={6} wrap>
      <Badge>{t(`live.state.${status.state}`)}</Badge>
      {badges(status).map((name) => (
        <Badge key={name}>{t(`live.badge.${name}`)}</Badge>
      ))}
      {status.sourceResolution ? <Badge>{status.sourceResolution}</Badge> : null}
      {status.health.sourceKbps > 0 ? (
        <Text variant="meta" color="textDim">
          {t('live.health.kbps', { kbps: Math.round(status.health.sourceKbps) })}
        </Text>
      ) : null}
      {status.standbyInSeconds !== null ? (
        <Text variant="meta" color="textDim">
          {t('live.standby.in', { seconds: status.standbyInSeconds })}
        </Text>
      ) : null}
    </Row>
  );
}
