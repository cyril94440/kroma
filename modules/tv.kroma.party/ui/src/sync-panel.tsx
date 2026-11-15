import { useT } from '@kromatv/module-sdk';
import { Badge, Button, Row, Section, Text } from '@kromatv/ui/kit';
import type { SyncState } from './schemas';
import type { SyncSession } from './use-sync-session';

export function SyncPanel({
  session,
  group,
}: Readonly<{ session: SyncSession; group: SyncState | null }>) {
  const t = useT();
  const others = (group?.members.length ?? 0) - (session.on ? 1 : 0);
  let status = t('party.sync.status.off');
  if (session.on) {
    status =
      others > 0
        ? t('party.sync.status.group', { count: group?.members.length ?? 0 })
        : t('party.sync.status.alone');
  }
  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('party.sync.title')}</Section.Title>
      </Section.Header>
      <Row gap={8}>
        <Button
          label={t(session.on ? 'party.sync.leave' : 'party.sync.join')}
          variant={session.on ? 'glass' : 'primary'}
          size="sm"
          onPress={session.on ? session.leave : session.join}
        />
        {session.on ? <Badge>{t('party.sync.badge')}</Badge> : null}
      </Row>
      <Text variant="meta" color="textDim" mt={6}>
        {status}
        {group?.by && session.on ? ` · ${t('party.sync.by', { name: group.by.name })}` : ''}
      </Text>
      {session.notice ? (
        <Text variant="meta" mt={4}>
          {t(session.notice.key, session.notice.vars)}
        </Text>
      ) : null}
    </Section.Root>
  );
}
