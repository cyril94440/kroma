import { useT } from '@kromatv/module-sdk';
import { Button, Field, Row, Section, Text } from '@kromatv/ui/kit';
import { useCallback, useEffect, useState } from 'react';
import type { HermesApi } from './hermes-api';

export function RulesPanel({ api }: Readonly<{ api: HermesApi }>) {
  const t = useT();
  const [text, setText] = useState('');
  const [saved, setSaved] = useState(false);

  const load = useCallback(() => {
    api.rules().then(
      (r) => setText(r.text),
      () => undefined,
    );
  }, [api]);

  useEffect(load, [load]);

  const save = () =>
    void api.saveRules(text).then(
      () => setSaved(true),
      () => setSaved(false),
    );
  const reset = () =>
    void api.resetRules().then(() => {
      setSaved(false);
      load();
    });

  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('hermes.rules.title')}</Section.Title>
      </Section.Header>
      <Text variant="meta" color="textDim" mb={8}>
        {t('hermes.rules.hint')}
      </Text>
      <Field.Root
        label={t('hermes.rules.title')}
        hideLabel
        value={text}
        onValueChange={(next) => {
          setText(next);
          setSaved(false);
        }}
      >
        <Field.Textarea rows={10} maxRows={24} />
      </Field.Root>
      <Row gap={8} mt={8}>
        <Button label={t('hermes.rules.save')} variant="primary" size="sm" onPress={save} />
        <Button label={t('hermes.rules.reset')} variant="ghost" size="sm" onPress={reset} />
        {saved ? (
          <Text variant="meta" color="textDim">
            {t('hermes.rules.saved')}
          </Text>
        ) : null}
      </Row>
    </Section.Root>
  );
}
