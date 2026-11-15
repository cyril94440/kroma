import { useT } from '@kromatv/module-sdk';
import { Badge, Row, Section, Text } from '@kromatv/ui/kit';
import type { Overview } from './schemas';

export function AdminCoverage({ overview }: Readonly<{ overview: Overview }>) {
  const t = useT();
  if (overview.coverage.length === 0) return null;
  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('live.admin.coverage.title')}</Section.Title>
      </Section.Header>
      {overview.coverage.map((row) => (
        <Row key={row.favourite} gap={8} wrap mb={4}>
          <Text variant="label">{row.favourite}</Text>
          <Text variant="meta" color="textDim">
            {t('live.admin.coverage.primary', { name: row.primary ?? '-' })}
          </Text>
          <Text variant="meta" color="textDim">
            {t('live.admin.coverage.backup', {
              names: row.backup.length > 0 ? row.backup.join(', ') : '-',
            })}
          </Text>
          <Badge>
            {t(row.gap ? `live.admin.coverage.gap.${row.gap}` : 'live.admin.coverage.ok')}
          </Badge>
        </Row>
      ))}
    </Section.Root>
  );
}
