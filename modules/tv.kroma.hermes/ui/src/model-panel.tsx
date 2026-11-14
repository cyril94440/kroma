import { useT } from '@kromatv/module-sdk';
import { Button, Row, Section, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import type { HermesApi } from './hermes-api';

export function ModelPanel({ api }: Readonly<{ api: HermesApi }>) {
  const t = useT();
  const [message, setMessage] = useState<string | null>(null);

  const test = () =>
    void api.testModel().then(
      (r) =>
        setMessage(
          r.ok
            ? t('hermes.model.ok', { ms: r.ms ?? 0, answer: r.answer ?? '' })
            : t('hermes.model.failed', { error: r.error ?? '' }),
        ),
      () => setMessage(t('hermes.model.failed', { error: 'network' })),
    );

  const list = () =>
    void api.listModels().then(
      (r) =>
        setMessage(
          r.ok
            ? `${t('hermes.model.models', { count: r.models.length })}: ${r.models.slice(0, 12).join(', ')}`
            : t('hermes.model.failed', { error: r.error ?? '' }),
        ),
      () => setMessage(t('hermes.model.failed', { error: 'network' })),
    );

  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('hermes.model.title')}</Section.Title>
      </Section.Header>
      <Row gap={8}>
        <Button label={t('hermes.model.test')} variant="glass" size="sm" onPress={test} />
        <Button label={t('hermes.model.list')} variant="glass" size="sm" onPress={list} />
      </Row>
      {message ? (
        <Text variant="meta" color="textDim" mt={8}>
          {message}
        </Text>
      ) : null}
    </Section.Root>
  );
}
