import { useT } from '@kromatv/module-sdk';
import { Box, Button, Field, Row, Text } from '@kromatv/ui/kit';
import { useRef, useState } from 'react';
import type { SportApi } from './sport-api';

interface CalibrateFormProps {
  api: SportApi;
  getPositionMs: () => number | null;
}

export function CalibrateForm({ api, getPositionMs }: Readonly<CalibrateFormProps>) {
  const t = useT();
  const [clock, setClock] = useState('');
  const [message, setMessage] = useState<string | null>(null);
  const frame = useRef<number | null>(null);
  const send = () => {
    api.calibrate(clock.trim(), frame.current ?? getPositionMs()).then(
      (done) => setMessage(t('sport.calibrate.done', { seconds: Math.round(done.delayS) })),
      (error: { code?: string }) =>
        setMessage(t(`sport.calibrate.error.${error.code ?? 'unreadable'}`)),
    );
    frame.current = null;
  };
  return (
    <Box data-testid="sport-calibrate">
      <Text variant="meta" color="textDim">
        {t('sport.calibrate.help')}
      </Text>
      <Row gap={8} align="center" mt={4}>
        <Box
          flex
          onFocus={() => {
            frame.current = getPositionMs();
          }}
        >
          <Field.Root
            label={t('sport.calibrate.label')}
            hideLabel
            value={clock}
            onValueChange={setClock}
          >
            <Field.Input placeholder="52:13" onSubmit={send} />
          </Field.Root>
        </Box>
        <Button label={t('sport.calibrate.send')} size="sm" variant="glass" onPress={send} />
      </Row>
      {message ? (
        <Text variant="meta" color="textDim">
          {message}
        </Text>
      ) : null}
    </Box>
  );
}
