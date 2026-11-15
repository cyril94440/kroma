import { useT } from '@kromatv/module-sdk';
import { Box, Button, Field, Row, Section, Text } from '@kromatv/ui/kit';
import { useEffect, useState } from 'react';
import type { LiveApi } from './live-api';
import type { ScanItem, ScanState } from './schemas';

const POLL_MS = 3000;

function framesOf(item: ScanItem): string[] {
  if (item.frames.length > 0) return item.frames;
  return item.frame ? [item.frame] : [];
}

function scanVerdict(item: ScanItem, t: (key: string) => string): string {
  if (item.error) return t('live.admin.scanFailed');
  if (item.black) return t('live.badge.black');
  return item.now ?? t('live.admin.scanLive');
}

export function AdminScan({ api }: Readonly<{ api: LiveApi }>) {
  const t = useT();
  const [query, setQuery] = useState('ligue1+');
  const [scan, setScan] = useState<ScanState | null>(null);

  useEffect(() => {
    let alive = true;
    const read = () =>
      api.scan().then(
        (next) => alive && setScan(next),
        () => undefined,
      );
    void read();
    const timer = setInterval(read, POLL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [api]);

  const start = (provider: 'primary' | 'backup') =>
    void api.startScan(query, provider).then(() => api.scan().then(setScan));

  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('live.admin.scan')}</Section.Title>
      </Section.Header>
      <Field.Root
        label={t('live.admin.scanQuery')}
        hideLabel
        value={query}
        onValueChange={setQuery}
      >
        <Field.Input placeholder={t('live.admin.scanQuery')} />
      </Field.Root>
      <Row gap={8} mt={8}>
        <Button
          label={t('live.admin.scanPrimary')}
          variant="glass"
          size="sm"
          disabled={scan?.running}
          onPress={() => start('primary')}
        />
        <Button
          label={t('live.admin.scanBackup')}
          variant="glass"
          size="sm"
          disabled={scan?.running}
          onPress={() => start('backup')}
        />
        {scan?.running ? (
          <Text variant="meta" color="textDim">
            {t('live.admin.scanRunning', { done: scan.items.length, total: scan.total })}
          </Text>
        ) : null}
      </Row>
      <Row gap={12} wrap mt={12}>
        {scan?.items.map((item) => (
          <Box key={item.id} style={{ width: 160 }}>
            {framesOf(item).map((path) => (
              <img
                key={path}
                src={api.fileUrl(path)}
                alt={item.name}
                width={160}
                style={{ borderRadius: 8, marginBottom: 4 }}
              />
            ))}
            <Text variant="meta" lines={1}>
              {item.number ?? '-'} · {item.name}
            </Text>
            <Text variant="meta" color="textDim" lines={1}>
              {scanVerdict(item, t)}
            </Text>
          </Box>
        ))}
      </Row>
    </Section.Root>
  );
}
