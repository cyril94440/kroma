import { useT } from '@kromatv/module-sdk';
import { Row, Section, Text } from '@kromatv/ui/kit';
import type { Viewer } from './schemas';

export function ViewersPanel({ viewers }: Readonly<{ viewers: readonly Viewer[] }>) {
  const t = useT();
  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('live.viewers.title', { count: viewers.length })}</Section.Title>
      </Section.Header>
      {viewers.map((viewer) => (
        <Row key={viewer.userId} gap={8} mb={4}>
          <Text variant="label">{viewer.name}</Text>
        </Row>
      ))}
    </Section.Root>
  );
}
