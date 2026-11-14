import { useT } from '@kromatv/module-sdk';
import { Box, Row, Text } from '@kromatv/ui/kit';
import { progressOf } from './live-state';
import type { Slot } from './schemas';

interface NowNextProps {
  now: Slot | null;
  next: Slot | null;
  clock: number;
}

function hour(ms: number, locale?: string): string {
  return new Date(ms).toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' });
}

export function NowNext({ now, next, clock }: Readonly<NowNextProps>) {
  const t = useT();
  if (!now && !next)
    return (
      <Text variant="meta" color="textDim">
        {t('live.guide.none')}
      </Text>
    );
  return (
    <Box>
      {now ? (
        <Box>
          <Row gap={8}>
            <Text variant="label">{t('live.guide.now')}</Text>
            <Text>{now.title}</Text>
            <Text variant="meta" color="textDim">
              {hour(now.start)} - {hour(now.end)}
            </Text>
          </Row>
          <Box
            mt={4}
            style={{ height: 3, borderRadius: 2, backgroundColor: 'rgba(255,255,255,.12)' }}
          >
            <Box
              style={{
                height: 3,
                borderRadius: 2,
                backgroundColor: '#ef4444',
                width: `${Math.round(progressOf(clock, now) * 100)}%`,
              }}
            />
          </Box>
        </Box>
      ) : null}
      {next ? (
        <Row gap={8} mt={6}>
          <Text variant="label" color="textDim">
            {t('live.guide.next')}
          </Text>
          <Text color="textDim">{next.title}</Text>
          <Text variant="meta" color="textDim">
            {hour(next.start)}
          </Text>
        </Row>
      ) : null}
    </Box>
  );
}
