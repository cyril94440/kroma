import { useT } from '@kromatv/module-sdk';
import { Box, Button, ListRow, Row, Section, Text } from '@kromatv/ui/kit';
import { useEffect, useState } from 'react';
import { Entry } from './entry';
import type { PartyApi } from './party-api';
import type { Candidate } from './schemas';

const SETTLE_MS = 300;

interface SwitchPanelProps {
  api: PartyApi;
  admin: boolean;
}

export function SwitchPanel({ api, admin }: Readonly<SwitchPanelProps>) {
  const t = useT();
  const [query, setQuery] = useState('');
  const [found, setFound] = useState<Candidate[]>([]);
  const [outcome, setOutcome] = useState<string | null>(null);

  useEffect(() => {
    const q = query.trim();
    if (!q) return;
    const timer = setTimeout(() => {
      api
        .channels(q)
        .then((r) => setFound(r.channels))
        .catch(() => setFound([]));
    }, SETTLE_MS);
    return () => clearTimeout(timer);
  }, [api, query]);

  const ask = async (channel: Candidate, force: boolean) => {
    const result = await api.requestSwitch(channel.id, channel.name, force).catch(() => null);
    setOutcome(result ? (result.reason ?? result.outcome) : 'error');
    if (result) setQuery('');
  };

  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('party.switch.title')}</Section.Title>
      </Section.Header>
      <Entry
        search
        label={t('party.switch.search')}
        value={query}
        onValueChange={setQuery}
        placeholder={t('party.switch.search')}
      />
      <Box mt={8}>
        {(query.trim() ? found : []).map((c) => (
          <ListRow.Root key={c.id}>
            <ListRow.Label>{c.name}</ListRow.Label>
            <ListRow.Trailing>
              <Row gap={6}>
                <Button
                  label={t('party.switch.ask')}
                  variant="glass"
                  size="sm"
                  onPress={() => void ask(c, false)}
                />
                {admin ? (
                  <Button
                    label={t('party.switch.force')}
                    variant="ghost"
                    size="sm"
                    onPress={() => void ask(c, true)}
                  />
                ) : null}
              </Row>
            </ListRow.Trailing>
          </ListRow.Root>
        ))}
      </Box>
      {outcome ? (
        <Text variant="meta" color="textDim" mt={6}>
          {t(`party.switch.outcome.${outcome}`)}
        </Text>
      ) : null}
    </Section.Root>
  );
}
