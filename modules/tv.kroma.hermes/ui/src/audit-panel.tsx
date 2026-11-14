import { useFormat, useT } from '@kromatv/module-sdk';
import { Box, Button, Row, Section, Text } from '@kromatv/ui/kit';
import { useCallback, useEffect, useState } from 'react';
import type { HermesApi } from './hermes-api';
import type { AuditRow } from './schemas';

export function AuditPanel({ api }: Readonly<{ api: HermesApi }>) {
  const t = useT();
  const fmt = useFormat();
  const [genre, setGenre] = useState('');
  const [genres, setGenres] = useState<string[]>([]);
  const [rows, setRows] = useState<AuditRow[]>([]);

  const load = useCallback(() => {
    api.audit(genre).then(
      (page) => {
        setRows(page.rows);
        setGenres(page.genres);
      },
      () => undefined,
    );
  }, [api, genre]);

  useEffect(load, [load]);

  const older = () => {
    const last = rows.at(-1);
    if (!last) return;
    void api.audit(genre, last.id).then((page) => setRows((current) => [...current, ...page.rows]));
  };

  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('hermes.audit.title')}</Section.Title>
      </Section.Header>
      <Row wrap gap={6} mb={8}>
        {['', ...genres].map((g) => (
          <Button
            key={g || 'all'}
            label={g || t('hermes.audit.all')}
            variant={genre === g ? 'primary' : 'ghost'}
            size="sm"
            onPress={() => setGenre(g)}
          />
        ))}
      </Row>
      {rows.length === 0 ? (
        <Text variant="meta" color="textDim">
          {t('hermes.audit.empty')}
        </Text>
      ) : null}
      {rows.map((row) => (
        <Box key={row.id} mb={6}>
          <Row gap={8}>
            <Text variant="meta" color="textDim">
              {fmt.elapsed(row.at)}
            </Text>
            <Text variant="label">{`${row.genre} / ${row.event}`}</Text>
          </Row>
          <Text variant="meta" color="textMuted">
            {JSON.stringify(row.data)}
          </Text>
        </Box>
      ))}
      {rows.length > 0 ? (
        <Button label={t('hermes.audit.more')} variant="ghost" size="sm" onPress={older} />
      ) : null}
    </Section.Root>
  );
}
