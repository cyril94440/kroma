import { useT } from '@kromatv/module-sdk';
import { Box, Button, Field, ListRow, Row, Section, Text } from '@kromatv/ui/kit';
import { useEffect, useState } from 'react';
import type { LiveApi } from './live-api';
import type { ChannelView } from './schemas';

const SETTLE_MS = 300;

interface ChannelPanelProps {
  api: LiveApi;
  current: string | null;
  onSwitched: () => void;
}

export function ChannelPanel({ api, current, onSwitched }: Readonly<ChannelPanelProps>) {
  const t = useT();
  const [query, setQuery] = useState('');
  const [found, setFound] = useState<ChannelView[]>([]);
  const [favourites, setFavourites] = useState<ChannelView[]>([]);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    api.favourites().then(setFavourites, () => setFavourites([]));
  }, [api]);

  useEffect(() => {
    const q = query.trim();
    if (!q) return;
    const timer = setTimeout(() => {
      api.channels({ q }).then(
        (page) => setFound(page.channels),
        () => setFound([]),
      );
    }, SETTLE_MS);
    return () => clearTimeout(timer);
  }, [api, query]);

  const watch = async (channel: ChannelView) => {
    const outcome = await api
      .switchTo(channel.id)
      .catch((error: { code?: string }) => ({ status: error.code ?? 'error' }));
    setMessage(t(`live.switch.${outcome.status}`));
    if (outcome.status === 'switched' || outcome.status === 'already') {
      setQuery('');
      onSwitched();
    }
  };

  const rows = query.trim() ? found : favourites;
  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('live.channels.title')}</Section.Title>
      </Section.Header>
      <Field.Root
        label={t('live.channels.search')}
        hideLabel
        value={query}
        onValueChange={setQuery}
      >
        <Field.Input type="search" icon="search" placeholder={t('live.channels.search')} />
      </Field.Root>
      <Box mt={8}>
        <Text variant="meta" color="textDim">
          {query.trim() ? t('live.channels.results') : t('live.channels.favourites')}
        </Text>
        {rows.map((channel) => (
          <ListRow.Root key={channel.id}>
            <ListRow.Label>
              {channel.event?.competition
                ? `${channel.name} - ${channel.event.competition}`
                : channel.name}
            </ListRow.Label>
            <ListRow.Trailing>
              <Row gap={8}>
                {channel.now ? (
                  <Text variant="meta" color="textDim" lines={1}>
                    {channel.now.title}
                  </Text>
                ) : null}
                <Button
                  label={
                    channel.id === current ? t('live.channels.playing') : t('live.channels.watch')
                  }
                  variant="glass"
                  size="sm"
                  disabled={channel.id === current}
                  onPress={() => void watch(channel)}
                />
              </Row>
            </ListRow.Trailing>
          </ListRow.Root>
        ))}
        {rows.length === 0 ? (
          <Text variant="meta" color="textDim">
            {t('live.channels.empty')}
          </Text>
        ) : null}
        {message ? <Text variant="meta">{message}</Text> : null}
      </Box>
    </Section.Root>
  );
}
