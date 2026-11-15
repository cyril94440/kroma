import { clampScore } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Box, IconButton, Text } from '@kromatv/ui/kit';

export function ScoreStepper({
  team,
  value,
  autoFocus,
  onChange,
}: Readonly<{
  team: string;
  value: number;
  autoFocus?: boolean;
  onChange: (next: number) => void;
}>) {
  const t = useT();
  return (
    <Box row align="center" justify="space-between" gap={16}>
      <Text variant="bodyTv" flex>
        {team}
      </Text>
      <IconButton
        icon="minus"
        label={t('live.prono.less', { team })}
        variant="glass"
        autoFocus={autoFocus}
        onPress={() => onChange(clampScore(value - 1))}
      />
      <Text variant="headingTv">{value}</Text>
      <IconButton
        icon="plus"
        label={t('live.prono.more', { team })}
        variant="glass"
        onPress={() => onChange(clampScore(value + 1))}
      />
    </Box>
  );
}
