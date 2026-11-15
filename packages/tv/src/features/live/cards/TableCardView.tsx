import { rowTone, type RowTone } from '@kromatv/core';
import type { TableCard } from '@kromatv/client/party';
import { Box, Text } from '@kromatv/ui/kit';
import type { ColorValue } from '@kromatv/ui/kit';

const TONE_INK: Record<NonNullable<RowTone>, ColorValue> = {
  live: 'danger',
  win: 'success',
  draw: 'accentText',
  loss: 'danger',
};

export function TableCardView({ card }: Readonly<{ card: TableCard }>) {
  return (
    <Box gap={8} p={20} radius="lg" bg="surface2">
      {card.title ? <Text variant="labelTv">{card.title}</Text> : null}
      <Box row gap={16}>
        {card.columns.map((column) => (
          <Box key={column} flex>
            <Text variant="overlineTv" color="textMuted">
              {column}
            </Text>
          </Box>
        ))}
      </Box>
      {card.rows.map((row, at) => {
        const tone = rowTone(card, at);
        return (
          <Box key={row.join('|')} row gap={16} bg={tone ? 'accentSoft' : undefined}>
            {row.map((cell, column) => (
              <Box key={card.columns[column]} flex>
                <Text
                  variant="bodyTv"
                  color={tone && column === card.live?.points_col ? TONE_INK[tone] : 'text'}
                >
                  {String(cell)}
                </Text>
              </Box>
            ))}
          </Box>
        );
      })}
      {card.note ? (
        <Text variant="labelTv" color="textDim">
          {card.note}
        </Text>
      ) : null}
    </Box>
  );
}
