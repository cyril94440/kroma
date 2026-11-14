import { useT } from '@kromatv/module-sdk';
import { Badge, Button, Row, Section, Text } from '@kromatv/ui/kit';
import type { PartyApi } from './party-api';
import type { PartyState } from './schemas';

interface PresencePanelProps {
  api: PartyApi;
  state: PartyState;
}

export function PresencePanel({ api, state }: Readonly<PresencePanelProps>) {
  const t = useT();
  const sync = state.sync;
  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('party.presence.title', { count: state.presence.length })}</Section.Title>
      </Section.Header>
      {state.presence.map((v) => {
        const stats = state.stats[v.userId];
        const inParty = sync?.members.some((m) => m.userId === v.userId);
        return (
          <Row key={v.userId} gap={8} mb={4}>
            <Text variant="label">{v.name}</Text>
            {inParty ? <Badge>{t('party.sync.badge')}</Badge> : null}
            {stats ? (
              <Text variant="meta" color="textDim">
                {t('party.presence.stats', {
                  buffer: stats.bufferS.toFixed(1),
                  stalls: stats.stalls,
                })}
              </Text>
            ) : null}
            {state.me.admin && inParty ? (
              <Button
                label={t('party.sync.kick')}
                variant="ghost"
                size="sm"
                onPress={() => void api.kick(v.userId)}
              />
            ) : null}
          </Row>
        );
      })}
      {sync ? (
        <Text variant="meta" color="textDim" mt={6}>
          {t('party.sync.target', { seconds: sync.target.toFixed(1) })}
          {sync.by ? ` · ${t('party.sync.by', { name: sync.by.name })}` : ''}
        </Text>
      ) : null}
      {state.me.admin && sync ? (
        <Row gap={8} mt={8}>
          <Button
            label={t('party.sync.goLive')}
            variant="glass"
            size="sm"
            onPress={() => void api.goLive()}
          />
          <Button
            label={t('party.sync.fineSync')}
            variant="glass"
            size="sm"
            onPress={() => void api.fineSync()}
          />
        </Row>
      ) : null}
    </Section.Root>
  );
}
