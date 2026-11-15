import type { MatchView } from '@kromatv/core';
import { Box, Text } from '@kromatv/ui/kit';
import { useT } from '#mobile/lib/i18n';

const MOMENTUM_HEIGHT = 60;

export function MatchStats({ view }: Readonly<{ view: MatchView }>) {
  const t = useT();
  return (
    <Box gap={10}>
      {view.stats.map((stat) => {
        const total = stat.dom + stat.ext || 1;
        const unit = stat.key === 'possession' ? '%' : '';
        return (
          <Box key={stat.key} gap={4}>
            <Box row justify="space-between">
              <Text variant="label">{`${stat.dom}${unit}`}</Text>
              <Text variant="meta" color="textDim">
                {t(`live.stat.${stat.key}`)}
              </Text>
              <Text variant="label">{`${stat.ext}${unit}`}</Text>
            </Box>
            <Box row h={6} radius="pill" bg="tint/6">
              <Box style={{ flex: stat.dom / total }} bg="accent" />
              <Box style={{ flex: stat.ext / total }} bg="info" />
            </Box>
          </Box>
        );
      })}
      <Text variant="meta" color="textDim">
        {t('live.momentum')}
      </Text>
      <Box row gap={2} h={MOMENTUM_HEIGHT} align="center">
        {view.momentum.map((slice) => (
          <Box key={slice.fromMinute} flex align="center" justify="center" h="100%">
            <Box w="100%" style={{ height: `${slice.dom * 50}%` }} bg="accent" />
            <Box w="100%" style={{ height: `${slice.ext * 50}%` }} bg="info" />
          </Box>
        ))}
      </Box>
    </Box>
  );
}
