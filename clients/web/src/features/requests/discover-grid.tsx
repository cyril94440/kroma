import { useT } from '@kromatv/ui';
import { Box, Button, EmptyState, Row, styles, Text, useStyles } from '@kromatv/ui/kit';
import { DiscoverCard } from '#web/features/requests/discover-card';
import type { DiscoverPageState } from '#web/features/requests/use-discover-lists';
import { SkeletonRow } from '#web/shared/ui';
import { TileGrid } from '#web/shared/ui/tile-grid';

const sDefinition = styles({ count: { fontVariant: ['tabular-nums'] } });

export function DiscoverGrid({ state }: Readonly<{ state: DiscoverPageState }>) {
  const t = useT();
  if (state.loading && state.entries.length === 0) {
    return (
      <Box mt={32}>
        <SkeletonRow count={12} />
      </Box>
    );
  }
  if (state.entries.length === 0) {
    return (
      <EmptyState.Root icon="mood-empty">
        <EmptyState.Title>{t('discover.noResults')}</EmptyState.Title>
      </EmptyState.Root>
    );
  }
  return (
    <Box mt={32}>
      <TileGrid>
        {(width) =>
          state.entries.map((entry) => (
            <DiscoverCard key={`${entry.kind}-${entry.tmdbId}`} entry={entry} width={width} />
          ))
        }
      </TileGrid>
    </Box>
  );
}

export function DiscoverPager({
  page,
  totalPages,
  onGo,
}: Readonly<{ page: number; totalPages: number; onGo: (n: number) => void }>) {
  const s = useStyles(sDefinition);

  const t = useT();
  if (totalPages <= 1) return null;
  return (
    <Row justify="center" gap={16} mt={40}>
      <Button
        variant="glass"
        size="sm"
        icon="chevron-left"
        label={t('discover.prev')}
        onPress={() => onGo(page - 1)}
        disabled={page <= 1}
      />
      <Text variant="meta" color="textDim" style={s.count}>
        {t('discover.pageOf', { page: String(page), total: String(totalPages) })}
      </Text>
      <Button
        variant="glass"
        size="sm"
        iconRight="chevron-right"
        label={t('discover.next')}
        onPress={() => onGo(page + 1)}
        disabled={page >= totalPages}
      />
    </Row>
  );
}
