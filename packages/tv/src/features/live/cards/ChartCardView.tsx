import type { ChartCard } from '@kromatv/client/party';
import { useT } from '@kromatv/ui';
import { Box, Chart, Text } from '@kromatv/ui/kit';
import { useMemo } from 'react';

const HEIGHT = 160;

export function ChartCardView({ card }: Readonly<{ card: ChartCard }>) {
  const t = useT();
  const data = useMemo(() => card.points.map((p) => ({ at: p.label, value: p.value })), [card]);
  const first = card.points[0];
  const last = card.points.at(-1);
  const unit = card.unit ? ` ${card.unit}` : '';
  const label = t('live.card.chart', {
    title: card.title ?? '',
    from: `${first?.value ?? ''}${unit}`,
    to: `${last?.value ?? ''}${unit}`,
  });
  return (
    <Box gap={8} p={20} radius="lg" bg="surface2">
      {card.title ? <Text variant="labelTv">{card.title}</Text> : null}
      <Chart.Root data={data} x="at" height={HEIGHT} label={label} format={(v) => `${v}${unit}`}>
        <Chart.Grid />
        {card.style === 'bar' ? (
          <Chart.Bar series="value" label={card.title ?? ''} />
        ) : (
          <Chart.Line series="value" label={card.title ?? ''} curve="linear" />
        )}
        <Chart.Axis edge="left" />
        <Chart.Axis edge="bottom" />
      </Chart.Root>
    </Box>
  );
}
