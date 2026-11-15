import { useT } from '@kromatv/module-sdk';
import { Box, Row, Text } from '@kromatv/ui/kit';
import type { MomentumSlice, Stat } from './match-calc';

interface StatsViewProps {
  stats: Stat[];
  momentum: MomentumSlice[];
}

export function StatsView({ stats, momentum }: Readonly<StatsViewProps>) {
  const t = useT();
  return (
    <Box data-testid="sport-stats">
      {stats.map((stat) => {
        const total = stat.dom + stat.ext || 1;
        return (
          <Box key={stat.key} mb={10}>
            <Row between>
              <Text variant="label">
                {stat.dom}
                {stat.key === 'possession' ? '%' : ''}
              </Text>
              <Text variant="meta" color="textDim">
                {t(`sport.stat.${stat.key}`)}
              </Text>
              <Text variant="label">
                {stat.ext}
                {stat.key === 'possession' ? '%' : ''}
              </Text>
            </Row>
            <Row h={6} radius={3} bg="tint/6">
              <Box style={{ flex: stat.dom / total }} bg="accent" />
              <Box style={{ flex: stat.ext / total }} bg="info" />
            </Row>
          </Box>
        );
      })}
      <Text variant="meta" color="textDim">
        {t('sport.momentum')}
      </Text>
      <Row gap={2} h={60} align="center">
        {momentum.map((slice) => (
          <Box key={slice.fromMinute} flex align="center" justify="center" h="100%">
            <Box w="100%" style={{ height: `${slice.dom * 50}%` }} bg="accent" />
            <Box w="100%" style={{ height: `${slice.ext * 50}%` }} bg="info" />
          </Box>
        ))}
      </Row>
    </Box>
  );
}
