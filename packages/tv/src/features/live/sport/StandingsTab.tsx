import { COMPETITIONS } from '@kromatv/core';
import { useStandings } from '@kromatv/core/react';
import type { LiveClient } from '@kromatv/core/react';
import { useT } from '@kromatv/ui';
import { Box, Chip, FocusRegion, Text } from '@kromatv/ui/kit';
import { useState } from 'react';

const ROWS = 12;

export function StandingsTab({ client }: Readonly<{ client: Pick<LiveClient, 'sport'> }>) {
  const t = useT();
  const [competition, setCompetition] = useState<string>(COMPETITIONS[0]);
  const { state, table } = useStandings(client, competition, true);
  return (
    <Box gap={14}>
      <FocusRegion style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
        {COMPETITIONS.map((name, at) => (
          <Chip
            key={name}
            label={name}
            active={name === competition}
            autoFocus={at === 0}
            onPress={() => setCompetition(name)}
          />
        ))}
      </FocusRegion>
      {table ? (
        <Box gap={6}>
          <Text variant="labelTv" color="textDim">
            {t('live.standings.provisional', { count: table.matchs_comptes })}
          </Text>
          {table.lignes.slice(0, ROWS).map((row) => (
            <Box key={row.club} row justify="space-between" bg={row.outcome ? 'accentSoft' : undefined}>
              <Text variant="bodyTv" flex>{`${row.rang}  ${row.club}`}</Text>
              <Text variant="bodyTv">{`${row.pts}  ${row.diff > 0 ? '+' : ''}${row.diff}  ${row.match}`}</Text>
            </Box>
          ))}
        </Box>
      ) : (
        <Text variant="bodyTv" color="textDim">
          {t(state === 'unavailable' ? 'live.standings.unavailable' : 'live.standings.loading')}
        </Text>
      )}
    </Box>
  );
}
