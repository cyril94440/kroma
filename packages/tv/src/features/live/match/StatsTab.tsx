import type { MatchView } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Box, Text } from '@kromatv/ui/kit';

const MOMENTUM_HEIGHT = 80;

export function StatsTab({ view }: Readonly<{ view: MatchView }>) {
  const t = useT();
  return (
    <Box gap={14}>
      {view.stats.map((stat) => {
        const total = stat.dom + stat.ext || 1;
        const unit = stat.key === 'possession' ? '%' : '';
        return (
          <Box key={stat.key} gap={6}>
            <Box row justify="space-between">
              <Text variant="bodyTv">{`${stat.dom}${unit}`}</Text>
              <Text variant="labelTv" color="textMuted">
                {t(`live.stat.${stat.key}`)}
              </Text>
              <Text variant="bodyTv">{`${stat.ext}${unit}`}</Text>
            </Box>
            <Box row h={8} radius={4} bg="tint/6">
              <Box style={{ flex: stat.dom / total }} bg="accent" />
              <Box style={{ flex: stat.ext / total }} bg="info" />
            </Box>
          </Box>
        );
      })}
      <Text variant="overlineTv" color="accentText">
        {t('live.momentum')}
      </Text>
      <Box row gap={3} h={MOMENTUM_HEIGHT} align="center">
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
